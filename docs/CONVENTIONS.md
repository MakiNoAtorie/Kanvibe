# Conventions

## Layout
- Library plus binary: `src/lib.rs` declares the modules, `src/main.rs` only calls `kanvibe::app::run()`.
- One file per module: `src/domain.rs`, `src/app.rs`; later `src/storage.rs`, `src/history.rs`, etc.
- Domain is plain data, no iced types. `Data { next_id, boards }`; only `Data::new_id()` advances the counter (increment then return, first ID is 1).
- Domain operations return `Option<Id>` (`None` = nothing created: blank input or unknown target) and apply the name/title rules themselves.

## App
- `App` fields are `pub`: `data`, `current_board`, `board_name` (name prompt), `capture: Option<Capture { column, text }>`.
- `update(&mut self, Message) -> Task<Message>` is the only place state changes. Messages carry domain `Id`s.
- Widget IDs are `&str` constants in `app.rs` (`BOARD_NAME_INPUT`, `CAPTURE_INPUT`), focused via `iced::widget::operation::focus(ID)` returned as a Task.
- View functions passed to `iced::application` must be named `fn`s; a closure fails with "ViewFn is not general enough".
- iced 0.14 API notes: see the orchestrator's scratchpad `notes/iced-0.14.md`. `time::every` needs the iced `tokio` feature.

## Text edits (#6)
- At most one pending edit: `App.editing: Option<PendingEdit { field, start, text, typed_at }>` for renames, or `App.capture`. `pub fn App::commit_pending()` commits whichever is open, records one step (none if unchanged), closes the field. Every arm that changes domain data or UI context (selection, Board switch, drag start, panel close/switch, opening a field) calls it first.
- `Field` enum (`BoardName(Id)`, `ColumnName(Id)`): new fields add a variant, arms in `Field::id`/`commit_pending`, and a domain getter/setter.
- Messages: `StartEdit(Field)` (commits, focuses `EDIT_INPUT`), `EditChanged(String)`, `CommitEdit`, `EditTick(Instant)`, `CloseRequested`.
- Live edits write into `data` per keystroke; blank text is stored as `start`, so data never holds an empty name. Only `commit_pending` records the step.
- Domain: `Data::name(id)` / `Data::rename(id, name) -> Option<()>` for Board or Column.
- Idle write: `save_if_changed` returns early while `editing.typed_at.is_some()`. Tests send `EditTick(Instant::now() + 1s)`; no sleeping.
- Esc discards the capture box, then `commit_pending()`. A capture box interrupted by any other commit point adds its text as a Card.
- `App::editable_name(field, name, size)` is the double-click-to-rename widget; reuse it.
- Test helper `type_into(app, field, &[texts])` leaves an edit pending.

## Startup (#19, `src/startup.rs`)
- `main.rs` calls `kanvibe::startup::run()`; it owns the lock, the rfd box and the iced program. Program state `Screen::{App(App), Problem(Problem)}`, message `startup::Message::{App(app::Message), Retry, Restore, StartEmpty, Quit}`.
- `Screen::update` calls `app.save_if_changed()` after `App::update`; `App` stays unaware of startup. Program uses `.exit_on_close_request(false)`; the Problem screen maps close requests to `Quit`.
- `storage::load` -> `Result<Option<(Data, Settings)>, LoadError>`; `LoadError::{Newer, Io(io::Error), Unreadable(String)}`. `version` is required and read first. `App::open` -> `Result<App, LoadError>`.
- `storage::{lock, backup, restore, start_empty, file_path}`. File names: `kanvibe.lock`, `kanvibe.json.bak1/.bak2` (const `BACKUPS`), `kanvibe.json.corrupt-YYYYMMDDTHHMMSSZ`.
- Dates: `jiff` (not `time`). `Timestamp::strftime`; local via `Timestamp::try_from(SystemTime)?.to_zoned(TimeZone::system())`.
- Storage test helpers: `backup(dir, name, &data)`, `corrupt_files(dir)`, `problem(&screen)`, `opened(screen)`. A folder named `kanvibe.json` forces an I/O error cross-platform.

## Selection and keys (#7, `src/keys.rs`)
- Key flow: subscription sends `Message::Key(event)` for uncaptured presses; `update` queries focus, then `Message::KeyWithFocus(event, Option<selector::Target>)` builds `keys::Context` and runs `keys::map`, whose Message goes back through `update`. Tests send `KeyWithFocus(event, None)` or call `keys::map`.
- A captured Esc is sent as `Message::Escape` (the "leave text field" layer). New Esc layers go in that arm; it clears selection only when nothing else is open.
- A focused text field suppresses arrows, Ctrl+arrows, N; Ctrl+Z/Y and Esc still map. Every new text box needs a widget Id and a `keys::Input` variant wired into the `KeyWithFocus` Id table. `keys::Context` holds `focused` only; panel/archive/drag/dialog get added by their tickets.
- New shortcuts: new `Message` variant + arm in `keys::map`. Use `Message::Undo`/`Message::Redo`. `Message` derives `PartialEq`.
- `App.selection: Option<Selection { column, card }>`; `Message::Select(Selection)`. Anything changing the selection returns `reveal_selection()`. `App::validate_selection()` after undo/redo, Board creation, Board switch, import. `outlined(base_style, selected)` style helper. `Data::move_card(card, column, index)` (index clamped).
- Widget Ids: `BOARD_SCROLL`, `column_widget(id)`, `cards_widget(id)`, `card_widget(id)`; `reveal(scrollable, child, horizontal)`.
- Test helpers (`tests/update.rs`): `key(Named, ctrl)`, `app_with_cards()` (To do: A,B,C; Doing empty; Done: D), `selected(app, col, Option<card>)`, `card_id`.

## Columns (#10)
- `Data::add_column(board) -> Option<Id>`, `Data::set_limit(column, &str) -> Option<()>` (blank clears; invalid returns `None`). `Column.limit: Option<u32>` (`skip_serializing_if`). `Column::over_limit()`, `Column::count_label()`.
- App: `column_menu: Option<Id>`, `limit_input: Option<LimitInput { column, text }>`; Messages `AddColumn`, `ToggleColumnMenu`, `OpenLimit`, `LimitChanged`, `SubmitLimit`; `LIMIT_INPUT`. The limit prompt is not a pending edit; opening the menu/prompt calls `commit_pending()` first. The ⋯ menu is inline buttons under the header (#11 adds "Remove Column").
- `editable_name` returns a clipped, tooltipped, `Fill`-width container; use it for any named header.
- Feature-area tests may get their own file (`tests/columns.rs`) with local helpers to avoid conflicts in `tests/update.rs`.
- iced: no `push_maybe`; use `column![..].extend(option)` and `.chain([...])`.

## Saving (#21)
- `App::save_if_changed() -> Option<SaveJob>` runs after every `update` and never writes; it returns `None` while a pending edit has unwritten keystrokes or a write is in flight. `SaveJob::run(self) -> Message::Saved(Result<(), String>)` is the one place a save happens (backup rotation hooks there or in `storage::save`). `Screen::update` runs jobs on a `std::thread`.
- App fields: `writing`, `failed` (held from retry until a change or `RetrySave`), `not_saved` (bar text, built by `fn not_saved(path, io::Error)`), `closing: Closing::{No, Waiting, Asking, Exiting}`.
- Messages `Saved`, `RetrySave` (10 s timer + Retry button), `SaveCopy`, `CopySaved(Option<Result<(), String>>)` (`None` = dialog cancelled), `QuitWithoutSaving`. Arms that don't change domain data must not call `commit_pending`.
- Settings for a save come from private `App::settings()`; theme, sidebar, window go there.
- Overlays: `stack![content, opaque(center(panel).style(dim))]`, as in `view_close_dialog`.
- Storage tests: `send` runs `while let Some(job) = app.save_if_changed() { app.update(job.run()) }`; `send_while_writing` holds a job; `block_saves(dir)` (a folder named `kanvibe.json.tmp`) forces a failure.

## Backup rotation (#20)
- `storage::rotate(dir, &mut rotation, data_changed)` runs in `save_if_changed` right before a `SaveJob` is created (UI thread, never while a write is in flight). `App.rotation: storage::Rotation::{Due, Done, Off}`; `data_changed` = `saved.0 != data` (settings-only changes don't count).
- `Rotation::Off` disables rotation and the 24 h rule for the session; set it when a file didn't load cleanly (startup Restore/Start empty; #22 load repair sets it in `App::open`). A failed rotation stays `Due`.
- `.bak1` is written with `write_all` + `sync_all`, not `fs::copy` (which keeps the source mtime on Windows).
- Storage test helpers: `saved_sample(dir)`, `create_board(app, name)`, `backups(dir)`, `read(dir, name)`, `age(dir, name, hours)`. Compare file contents as strings, not bytes.

## Card panel (#8, `src/panel.rs`)
- `App::update` is a thin wrapper: private `apply` (the body) then `validate_ui()`. Extend `validate_ui` for any "after every update" rule (Board gone, filter Labels gone, ...); it also calls `validate_selection`.
- `App.panel: Option<Id>` (the Card shown); it stays when the Selection becomes column-only. Esc order now: text field, panel, Selection. A Board switch closes the panel via `validate_ui`.
- `Data::name`/`Data::rename` also take a Card id (title); `rename` never touches `updated`. `commit_pending` calls `Data::touch(card)` only when the committed text differs from `start`. `Data::move_card` bumps `updated` on a Column change only.
- Domain: `Card.created`/`Card.updated` (`#[serde(default)]` RFC 3339 strings), `domain::now()`, `Data::card(id)`, `Data::touch(id)`.
- Messages `OpenPanel` (Enter), `ClosePanel` (X), `EditTitle` (F2). `Field::CardTitle`. `panel.rs` has `local_time(&str, &TimeZone)`, `WIDTH`, `App::view_panel`; `editable_name` is `pub(crate)`.
- `view` wraps the Board as `row![container(board).width(Fill), panel]` before the Not saved bar.
- Tests setting timestamps directly reset `app.history = Default::default()`; use `OLD = "2020-01-01T00:00:00Z"`. `tests/panel.rs` helpers: `app_with_cards()`, `card(app, title)`, `updated(app, title)`, `click(app, title)`.

## Boards and sidebar (#12, `src/sidebar.rs`)
- Domain: `Board.archived`; `Data::{active_board, archive_board, restore_board, delete_board, fallback(current, order)}`. Restore moves the Board to the end; delete only works on archived Boards.
- `validate_ui(&mut self, board_before, order)`: runs the fallback, then `left_board()` when the current Board changed. Any new per-Board UI state (Filter, ...) must be cleared in `left_board()`.
- Shared confirmation: `App.confirm = Some(Confirm { prompt, action })`; Delete sends `Message::Confirm` (runs `action` through `update`); Cancel/Esc close. `app::dialog(panel)` (`pub(crate)`) is the dimmed centred overlay for any dialog.
- `sidebar::board_row(board, current)` is the row to wrap as a drop target (#16/#17). `Settings.sidebar_collapsed`; `Settings` literals in tests need `..Settings::default()`.
- `tests/boards.rs` helpers: `app_with_boards()` (A, B, C; C current), `names`, `archived`, `asking_to_delete_a()`, `send_saving`.

## Archive (#11, `src/archive.rs`)
- `Board.archived_cards: Vec<Archived { card, column_id, archived_at }>`, oldest first; `archive::rows(&Board)` newest first with Column name or "removed Column". `Data::{archive_card, remove_column, restore_card, delete_archived}`.
- `App.archive: Option<Id>` (Board whose Archive view is open); `validate_archive` in `validate_ui`. Permanent delete uses `App.confirm` with action `Message::DeleteCard(Id)`.
- A Card leaving the Board (archive, Move to Board) sets the selection to its neighbour and `panel = None` explicitly.
- Column ⋯ menu is `column![Set limit…, Remove Column (...)]`; Board header row `[name, Archive (n), ...]` is where filter/⋯ go.

## Drag (#15, `src/drag.rs`)
- `Item::Card(Id)`, `Target::{Card { column, index }, Column(Id)}`, `Drag { item, origin, cursor, started, hovered }`. #16/#17 add `Item`/`Target` variants and an arm in `DragEnd`'s `match (drag.item, drag.target())`.
- Messages `DragPress(Item)`, `DragMoved(Point)`, `DragHover(Target)`, `DragLeave(Target)`, `DragEnd`, `DragCancel`, `DragTick`. Press commits the pending edit; release under 5px is a click (`Select`).
- `Target::Card { column, index }` = slot before the Card at `index`. Drop: `move_card`, `validate_selection`, `history.record`.
- `drag::target(area, t, dragging)` adds enter/move/exit only while dragging; `hovered` lists all targets, `target()` prefers a Card slot. `keys::Context.dragging`: Esc → `DragCancel`, other keys nothing.

## Checklists (#14, `src/checklist.rs`)
- `Card.checklist: Vec<Item { id, text, done }>`; `Card::progress()`. `Data::{item, card_of_item, add_item, toggle_item, move_item, remove_item}` never touch `updated`; `App::change_checklist(item, op)` touches and records.
- `AddItem`/`SubmitItem` open a blank Item as `Field::ChecklistItem` with no step; the commit records it; `commit_pending` removes a blank Item. `keys::Input::Item`; Ctrl+Up/Down → `MoveEditedItem`.

## Labels (#13, `src/labels.rs`)
- `PALETTE: [(key, Color); 10]`, `swatch(key)` (unknown key → first colour), `chips(board, card)`, `delete_prompt`. Label `Data` ops live in labels.rs (`add_label`, `toggle_label`, `delete_label`, `label_use`, `label_clash`, ...); they cover archived Cards too. `Data::new_id` is `pub(crate)`.
- Filter reuse: `App::picker_rows(board, checked, toggle, editing: Option<&LabelPicker>)`; `None` = checkboxes only.
- `App.label_picker` overlay; `validate_labels` in `validate_ui`. Attach/detach/delete touch Cards; rename/recolour don't.

## Chrome (#23, `src/chrome.rs`)
- `App.dark`, `App::theme()`, `App::highlighter_theme()`; `Settings.{dark, window: Option<Geometry>, maximized}`.
- Window geometry never triggers saves: compare saved state only with private `same(a, b)` in app.rs, never `==`.
- Every shortcut needs a row in `chrome::SHORTCUTS` as well as `keys::map`. `chrome::tip(widget, "Label (Key)")` for tooltips.

## Description (#9, `src/description.rs`)
- `Field::Description(card)`; `App.editor: text_editor::Content` mirrors `PendingEdit.text`; blank allowed. `Data::{description, set_description}`.
- Messages `EditDescription`, `DescriptionAction`, `Format(Markup)`, `OpenLink`, `ClickOutside` (whole-view mouse_area; commits only a description edit).
- `App.rendered` caches parsed markdown; `refresh_rendered()` runs after `validate_ui`. `open_link` is the only `webbrowser` call; `opens_in_browser` allows http/https/mailto.
- Card face: title row (+ description icon), Label chips, Checklist progress.

## Reorder drags (#17) and Move to Board (#16)
- `Item::{Card, Column, Board}`; `Target::{Card, Column, ColumnGap(usize), BoardGap(usize), Board(Id)}`. Gap index = slot before item `i` (`len` = after last); `BoardGap` counts active Boards only. `Data::move_column(id, gap)`, `Data::move_board(id, gap)`.
- A target kind registers only while its matching Item is dragged (Card slots/Board rows for Cards, gaps for Columns/Boards). Match arms on `Target` end in `_ =>`.
- Under the threshold: `Item::Board` opens the Board, `Item::Column` does nothing. Labels that capture presses use `name_widget(field, name, size, Option<Item>)` to start a drag.
- `sidebar::board_row` is a `drag::source`; `sidebar::view` wraps rows in `drop_row` (Card drags, non-current Boards) inside `gap()`.
- `Data::move_to_board(card, board)` (Label match by trimmed lowercase name; target colour wins; missing Label appended; "To do" created when no Columns). `src/move_board.rs`: `App.move_picker`, `OpenMove` (M / panel button), `CloseMove`, `MoveToBoard(card, board)`; `validate_move` in `validate_ui`. Cross-Board history: `record(boards, Some(source), Some(target))`.
- `App::selection_after_leaving(card)` (archive.rs) for any action that makes a selected Card leave; then clear `panel` if it showed that Card.

## Load repair (#22, `src/repair.rs`)
- `repair::repair(Data) -> (Data, bool)` is the one place for load repairs (pure; bool = changed); import calls it too. New invariants go there. Lenient field readers are `pub(crate)` fns in repair.rs used via `deserialize_with` (e.g. `repair::limit`).
- `storage::open(dir) -> Result<Option<Loaded { data, settings, repaired }>, LoadError>`; `storage::load` drops `repaired`. `App::open` sets `Rotation::Off` when repaired. Settings are not repaired (`Data::fallback` handles `last_board`).
- Duplicate IDs: first holder keeps it, later ones get fresh IDs; references stay on the first holder. Blank-name defaults: Board "Untitled", Column "New column", Card "Untitled", Label "Label".
- Migration: pre-migration copy `kanvibe.v<old>.json` written at load with `create_new`; real migrations go in `storage::parse` before `FileIn`.
- Tests: `tests/repair.rs` helpers `write(json)`, `load_repaired(json)`, `opened(screen)`.

## Export / import (#24, `src/transfer.rs`)
- `transfer::{export(path, &Board), file_name(name)}`, `Data::import_board(Board) -> Id`, `App::update_transfer` handles `ExportBoard(Id)`, `Exported(..)`, `ImportBoard`, `Imported(Option<Result<Vec<u8>, String>>)` (`None` = dialog cancelled; raw bytes so tests drive parsing through `update`).
- `App.notice: Option<String>`: OK-only dimmed message (`view_notice`) for refusals/failures; while open only Esc maps.
- Import repairs on a throwaway `Data` before remapping IDs; names from outside go through `Data::free_name` (" (n)" rule).
- `tests/transfer.rs` helpers: `app_with_work()`, `shape(&Board)`, `ids`, `export`, `import(app, bytes)`, `refusal(bytes)`, `imported_name(existing, name)`.

## Filter (#18, `src/filter.rs`)
- Strict predicate: `filter::Filter { text, labels }` with `is_on()` and `matches(&Card)` (no iced types); image export uses `matches`. `App::shown(&Card)` adds the selected and just-created (`filter_kept`) Cards for the live view.
- `filter::placement(cards, slot, shown) -> usize` = drop gap under a filter; `Target::Card.index` stays an index among all Cards. `moved`/`card_move` take `shown: impl Fn(&Card) -> bool` (pass `|c| self.shown(c)`).
- App fields `filter`, `filter_kept`, `filter_menu`, cleared in `left_board()`; `App::clear_filter() -> bool`. Messages `FocusFilter`, `FilterChanged`, `SubmitFilter`, `ClearFilter`, `ToggleFilterMenu`, `FilterLabel(Id)`, `LeaveField` → `App::update_filter`.
- A captured Esc sends `Message::LeaveField`: it forwards to `Escape` when a field with state is open, else only leaves the focused box. New stateless text boxes need their own `keys::Input` variant and a look at `LeaveField`'s list. Unfocus via `operation::focus("no-widget")`.
- Board header row: `[name, filter_bar(), Archive (n), ⋯]`; `filter_lines(board)` sits under it.

## Esc order (current)
- cancel drag → shortcuts overlay → notice → close dialog → confirm → label picker → move picker → text field / limit / Board menu → Board menu / Labels filter list → Archive view → panel → filter → selection.

## Undo history (`src/history.rs`)
- `App.history: History` exposes `sync`, `record`, `undo`, `redo`. `update` calls `self.history.sync(&self.data.boards)` first (adopts current Boards as baseline, records nothing).
- Every `update` arm that changes domain data calls `self.history.record(&self.data.boards, show_undo, show_redo)` after. `record` skips unchanged state, so calling it after a no-op is safe. Board-level actions pass `None, None`; Card actions pass the Card's Board for both; a cross-Board move passes source then target.
- Undo/redo replace `data.boards` only; `next_id` and UI state are never restored.
- `Message::Key(keyboard::Event)` carries key presses; `undo_shortcut` in `app.rs` is to be folded into the key-mapping function (#7). Test helper `ctrl("z"|"y", shift)`.

## Storage (`src/storage.rs`)
- `storage::{data_dir(args), load(dir) -> io::Result<Option<(Data, Settings)>>, save(dir, &Data, &Settings)}`. `Ok(None)` = first run; content error = `ErrorKind::InvalidData`.
- File format lives in private `FileOut`/`FileIn` structs. Domain types carry serde derives; `Data` does not. New optional fields get `#[serde(default)]`.
- `Settings` (`#[serde(default)]`) holds saved app settings (today `last_board`); theme, sidebar and window go there too.
- `App.data_dir: Option<PathBuf>` (`None` saves nothing, so `tests/update.rs` is unaffected); `App.saved: (Data, Settings)` mirrors disk.
- `App::open(dir)` loads and picks the current Board. `App::save_if_changed()` runs after every `update` from the iced update closure in `run()`, never inside `update`. Lock and problem screens go before `App::open`.
- Storage tests: `send` helper in `tests/storage.rs` calls `update` then `save_if_changed`; folder from `data_dir()` with `tempfile` + `--data-dir`. Held-file retry tests: `#[cfg(windows)]` with `OpenOptionsExt::share_mode(0)`.

## Tests
- Integration tests in `tests/`, public API only. Seam 1: `tests/update.rs`. Storage seam: `tests/storage.rs`.
- `tests/update.rs` helpers: `send(&mut app, [msgs])`, `app_with_board()` (one current Board "Work"), `column_id`, `card_titles`, `capture(app, column, &[titles])`.

## Checks
- `cargo test --offline -q` and `cargo clippy --offline --all-targets -- -D warnings`; `cargo fmt` before committing.

## Windows tooling gotchas
- In worktrees, Bash heredocs and multi-step git commands may be refused by the isolation check; use Write/Edit and simple git commands.
- PowerShell 5.1: `git commit -F -` with a here-string breaks; write the message to a file and use `-F <file>`.
- Disk: build only in the worktree's own `target/` and delete it when done. Never share a `CARGO_TARGET_DIR` across worktrees: the `kanvibe` output file name is identical, so one worktree tests another's code.
- Long worktree paths can hit MAX_PATH (LNK1104) on release builds; use a shorter `--target-dir`.
