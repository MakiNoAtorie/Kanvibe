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
- At most one pending edit: `App.editing: Option<PendingEdit { field, start, text, typed_at }>` for renames, or `App.capture`. `pub fn App::commit_pending()` commits whichever is open, records one step (none if unchanged), closes the field. Every arm that changes domain data or UI context (selection, Board switch, drag start, open Card close/switch, opening a field) calls it first.
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
- A focused text field suppresses arrows, Ctrl+arrows, N; Ctrl+Z/Y and Esc still map. Every new text box needs a widget Id and a `keys::Input` variant wired into the `KeyWithFocus` Id table. `keys::Context` has `focused` and `dragging`; dialogs are gated in `App::allowed` via `App::dialog_open()`. Exception: Ctrl+B/I/K are `text_editor` key bindings (`description::key_binding`), not `keys::map`, because the editor captures them.
- New shortcuts: new `Message` variant + arm in `keys::map`. Use `Message::Undo`/`Message::Redo`. `Message` derives `PartialEq`.
- `App.selection: Option<Selection { column, card }>`; `Message::Select(Selection)`. Anything changing the selection returns `reveal_selection()`. `validate_ui` runs `validate_selection()` after every update; don't call it per arm. `outlined(base_style, selected)` style helper. `Data::move_card(card, column, index)` (index clamped).
- Widget Ids: `BOARD_SCROLL`, `column_widget(id)`, `cards_widget(id)`, `card_widget(id)`; `reveal(scrollable, child, horizontal)`.
- Test helpers (`tests/update.rs`): `key(Named, ctrl)`, `app_with_cards()` (To do: A,B,C; Doing empty; Done: D), `selected(app, col, Option<card>)`, `card_id`.

## Columns (#10)
- `Data::add_column(board) -> Option<Id>`, `Data::set_limit(column, &str) -> Option<()>` (blank clears; invalid returns `None`). `Column.limit: Option<u32>` (`skip_serializing_if`). `Column::over_limit()`, `Column::count_label()`.
- App: `column_menu: Option<Id>`, `limit_input: Option<LimitInput { column, text }>`; Messages `AddColumn`, `ToggleColumnMenu`, `OpenLimit`, `LimitChanged`, `SubmitLimit`; `LIMIT_INPUT`. The limit prompt is not a pending edit; opening the menu/prompt calls `commit_pending()` first. The ⋯ menu is inline buttons under the header (#11 adds "Remove Column").
- `editable_name` returns a clipped, tooltipped, `Fill`-width container; use it for any named header.
- Feature-area tests may get their own file (`tests/columns.rs`) with local helpers to avoid conflicts in `tests/update.rs`.
- iced: no `push_maybe`; use `column![..].extend(option)` and `.chain([...])`.

## Saving (#21)
- `App::save_if_changed() -> Option<SaveJob>` runs after every `update` and never writes; it returns `None` while a pending edit has unwritten keystrokes or a write is in flight. It compares `(&Data, &Settings)` in place and clones `Data` only once a save will happen (a clone is ~4 ms at 5k Cards; never clone on a per-message path). `SaveJob::run(self) -> Message::Saved(Result<(), String>, storage::Rotation)` is the one place a save happens: it rotates the backups if due, then writes. `Screen::update` runs jobs on a `std::thread`.
- App fields: `writing: Option<Arc<(Data, Settings)>>` (shared with the `SaveJob`; the `Saved` arm takes it back with `Arc::try_unwrap`), `failed` (held from retry until a change or `RetrySave`), `not_saved` (bar text, built by `fn not_saved(path, io::Error)`), `closing: Closing::{No, Waiting, Asking, Exiting}`.
- Messages `Saved`, `RetrySave` (10 s timer + Retry button), `SaveCopy`, `CopySaved(Option<Result<(), String>>)` (`None` = dialog cancelled), `QuitWithoutSaving`. Save-result arms (`Saved`, `RetrySave`, `CopySaved`) must not call `commit_pending`; arms that change domain data or UI context call it first.
- Settings for a save come from private `App::settings()`; theme, sidebar, window go there.
- Overlays: `stack![content, opaque(center(panel).style(dim))]`, as in `view_close_dialog`.
- Storage tests: `send` runs `while let Some(job) = app.save_if_changed() { app.update(job.run()) }`; `send_while_writing` holds a job; `block_saves(dir)` (a folder named `kanvibe.json.tmp`) forces a failure.

## Backup rotation (#20)
- `storage::rotate(dir, &mut rotation, data_changed)` runs in `SaveJob::run` right before `storage::save` (writer thread, never while another write is in flight); the job carries `App.rotation` in and `Message::Saved(.., rotation)` hands the new state back, so `App.rotation` changes only at `App::open`, startup Restore/Start empty, or `Saved`. `App.rotation: storage::Rotation::{Due, Done, Off}`; `data_changed` = `saved.0 != data` (settings-only changes don't count), computed only while `Due`.
- `.bak1` moves to `.bak2` only when it parses; an unreadable `.bak1` is just replaced, so a broken backup never pushes a readable one out.
- `Rotation::Off` disables rotation and the 24 h rule for the session; set it when a file didn't load cleanly (startup Restore/Start empty; #22 load repair sets it in `App::open`). A failed rotation stays `Due`.
- `.bak1` is written with `write_all` + `sync_all`, not `fs::copy` (which keeps the source mtime on Windows).
- Storage test helpers: `saved_sample(dir)`, `create_board(app, name)`, `backups(dir)`, `read(dir, name)`, `age(dir, name, hours)`. Compare file contents as strings, not bytes.

## Card fields and validation (#8; the open Card is in the v2 section)
- `App::update` is a thin wrapper: private `apply` (the body) then `validate_ui()`. Extend `validate_ui` for any "after every update" rule (Board gone, filter Labels gone, ...); it also calls `validate_selection`.
- `Data::name`/`Data::rename` also take a Card id (title); `rename` never touches `updated`. `commit_pending` calls `Data::touch(card)` only when the committed text differs from `start`.
- Domain: `Card.created`/`Card.updated` (`#[serde(default)]` RFC 3339 strings), `domain::now()`, `Data::card(id)`, `Data::card_mut(id)` (the one mutable active-Card lookup), `Data::touch(id)`.
- `EditTitle` (F2), `Field::CardTitle`; `editable_name` is `pub(crate)`.
- Tests setting timestamps directly reset `app.history = Default::default()`; use `OLD = "2020-01-01T00:00:00Z"`. `tests/open_card.rs` helpers: `app_with_cards()`, `card(app, title)`, `updated(app, title)`, `click(app, title)`.

## Boards and sidebar (#12, `src/sidebar.rs`)
- Domain: `Board.archived`; `Data::{active_board, archive_board, restore_board, delete_board, fallback(current, order)}`. Restore moves the Board to the end; delete only works on archived Boards.
- `validate_ui(&mut self, board_before, order)`: runs the fallback, then `left_board()` when the current Board changed. Any new per-Board UI state (Filter, ...) must be cleared in `left_board()`.
- Shared confirmation: `App.confirm = Some(Confirm { prompt, action })`; Delete sends `Message::Confirm` (runs `action` through `update`); Cancel/Esc close. `app::dialog(panel)` (`pub(crate)`) is the dimmed centred overlay for any dialog.
- `sidebar::board_row(board, current)` is the row to wrap as a drop target (#16/#17). `Settings.sidebar_collapsed`; `Settings` literals in tests need `..Settings::default()`.
- `tests/boards.rs` helpers: `app_with_boards()` (A, B, C; C current), `names`, `archived`, `asking_to_delete_a()`, `send_saving`.

## Archive (#11, `src/archive.rs`)
- `Board.archived_cards: Vec<Archived { card, column_id, archived_at }>`, oldest first; `archive::rows(&Board)` newest first with Column name or "removed Column". `Data::{archive_card, remove_column, restore_card, delete_archived}`.
- `App.archive: Option<Id>` (Board whose Archive view is open); `validate_archive` in `validate_ui`. Permanent delete uses `App.confirm` with action `Message::DeleteCard(Id)`.
- A Card leaving the Board (archive, Move to Board) sets the selection to its neighbour and closes the open Card (`App::card_leaves`).
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
- `App.skin`, `App::theme()`, `App::highlighter_theme()`; `Settings.{skin, window: Option<Geometry>, maximized}`.
- Themes live in `src/skin.rs`: `Skin::ALL` (Light, Dark, Aurora, Paper, Tinted, Midnight), saved as `"theme": Skin::key`. A style fn asks `Skin::of(theme)` (by the iced theme's name); the overrides there (`page`, `column`, `header`, `card`, `selected`, `sidebar`, `pill_radius`) return `None` for Light and Dark so the plain styles in app.rs apply. `column_style(column_id)` takes the Column (expanded, strip and export alike) so Tinted can tint it. The sidebar footer's theme list sends `Message::SetTheme(Skin)`. A new theme: add the variant to every match and `ALL`.
- Window geometry never triggers saves: compare saved state only with private `same(a, b)` in app.rs, never `==`.
- Every shortcut needs a row in `chrome::SHORTCUTS` as well as `keys::map`. `chrome::tip(widget, "Label (Key)")` for tooltips.

## Description (#9, `src/description.rs`)
- `Field::Description(card)`; `App.editor: text_editor::Content` mirrors `PendingEdit.text`; blank allowed. `Data::{description, set_description}`.
- Messages `EditDescription`, `DescriptionAction`, `Format(Markup)`, `OpenLink`, `ClickOutside` (whole-view mouse_area; commits only a description edit).
- `App.rendered` caches parsed markdown; `refresh_rendered()` runs after `validate_ui`. `open_link` is the only `webbrowser` call; `opens_in_browser` allows http/https/mailto.

## Reorder drags (#17) and Move to Board (#16)
- `Item::{Card, Column, Board}`; `Target::{Card, Column, ColumnGap(usize), BoardGap(usize), Board(Id)}`. Gap index = slot before item `i` (`len` = after last); `BoardGap` counts active Boards only. `Data::move_column(id, gap)`, `Data::move_board(id, gap)`.
- A target kind registers only while its matching Item is dragged (Card slots/Board rows for Cards, gaps for Columns/Boards). Match arms on `Target` end in `_ =>`.
- Under the threshold: `Item::Board` opens the Board, `Item::Column` does nothing. Labels that capture presses use `name_widget(field, name, size, Option<Item>)` to start a drag.
- `sidebar::board_row` is a `drag::source`; `sidebar::view` wraps rows in `drop_row` (Card drags, non-current Boards) inside `gap()`.
- `Data::move_to_board(card, board)` (Label match by trimmed lowercase name; target colour wins; missing Label appended; "To do" created when no Columns). `src/move_board.rs`: `App.move_picker`, `OpenMove` (M / open Card button), `CloseMove`, `MoveToBoard(card, board)`; `validate_move` in `validate_ui`. Cross-Board history: `record(boards, Some(source), Some(target))`.
- `App::selection_after_leaving(card)` (archive.rs) picks the neighbour for any action that makes a selected Card leave; `card_leaves` uses it.

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

## Image export (#25, `src/image.rs`)
- `image::render(&Board, &Filter, &Theme) -> Result<Vec<u8>, String>` blocks; call only on a worker thread. `image::LIMIT = 8192`. `App.saving_image`; Messages `SaveImage(Id)`, `ImageSaved(..)` → `App::update_image`.
- `app::card_face(Option<&Board>, &Card)` is the shared unstyled Card face; new face content goes there. `Filter::count_label(column, shown)` builds "n of m".
- Layout: 12 px padding, 280 px Columns, 12 px gaps: width = 292·n + 12.
- `iced_test::simulator(app.view()).find("text")` checks view text in tests (starts a GPU device; use sparingly). Clippy: `as_chunks::<4>().0` for RGBA, not `chunks_exact(4)`.

## After the whole-spec review
- Lookups from a Card/Column go through `Data::{board_of_card, column_of_card, board_of_column, board_of_archived, column, holding}`; don't add lookups on `App`.
- `App::record_step(board)` records a step showing one Board; call `history.record` directly only for a cross-Board move. `commit_pending()` and `History::record` return `bool` (step recorded).
- A Card leaving the Board goes through `App::card_leaves(card, |data| …)` (selection + `open = false`); the caller records the step.
- `App::dialog_open()` lists overlays that pass only Esc; new dialogs go there. `App::field_open()` lists stateful text boxes (Esc and `LeaveField`); new ones go there.
- View helpers: `app::{clipped, clipped_text, board_name_input, dialog_header}`, `domain::cards(n)`, `transfer::stem(name)`.
- Settings file shape lives in private `SettingsFile` in storage.rs (`"theme"`, `"window": {x, y, w, h, maximized}`); new settings go there and in both `From` impls.
- `App::same` ignores window geometry except while closing. A leftover `kanvibe.json.tmp` is removed only after a successful `storage::open`.

## Esc order (current)
- cancel drag → shortcuts overlay → notice → close dialog → confirm → label picker → move picker → Search (when no text field is open) → text field / limit / Board menu → Board menu / Labels filter list → Archive view → open Card (clears `open` even with no Card selected) → filter → selection.

## Undo history (`src/history.rs`)
- `App.history: History` exposes `sync`, `record`, `undo`, `redo`. `update` calls `self.history.sync(&self.data.boards)` first (adopts current Boards as baseline, records nothing).
- Every `update` arm that changes domain data calls `self.history.record(&self.data.boards, show_undo, show_redo)` after. `record` skips unchanged state, so calling it after a no-op is safe. Board-level actions pass `None, None`; Card actions pass the Card's Board for both; a cross-Board move passes source then target.
- `History::undo/redo(&mut boards) -> Option<Option<Id>>` restore in place, keeping every current Board that equals its snapshot and cloning only the changed ones; `next_id` and UI state are never restored.

## Storage (`src/storage.rs`)
- `storage::{data_dir(args), open(dir), load(dir), save(dir, &Data, &Settings)}`; see Startup and Load repair for return types.
- File format lives in private `FileOut`/`FileIn` structs. Domain types carry serde derives; `Data` does not. New optional fields get `#[serde(default)]`.
- `Settings` (`#[serde(default)]`) holds saved app settings (today `last_board`); theme, sidebar and window go there too.
- `App.data_dir: Option<PathBuf>` (`None` saves nothing, so `tests/update.rs` is unaffected); `App.saved: (Data, Settings)` mirrors disk.
- `App::open(dir)` loads and picks the current Board. `App::save_if_changed()` runs after every `update` from the iced update closure in `run()`, never inside `update`. Lock and problem screens go before `App::open`.
- Storage tests: `send` helper in `tests/storage.rs` calls `update` then `save_if_changed`; folder from `data_dir()` with `tempfile` + `--data-dir`. Held-file retry tests: `#[cfg(windows)]` with `OpenOptionsExt::share_mode(0)`.

## Tests
- Integration tests in `tests/`, public API only. Seam 1: `tests/update.rs`. Storage seam: `tests/storage.rs`.
- `tests/update.rs` helpers: `send(&mut app, [msgs])`, `app_with_board()` (one current Board "Work"), `column_id`, `card_titles`, `capture(app, column, &[titles])`.

## Checks
- Local CI is `ci.bat` (fmt --check, clippy -D warnings, tests); no hosted CI. Run it before every push and merge.
- `cargo test --offline -q` and `cargo clippy --offline --all-targets -- -D warnings`; `cargo fmt` before committing. Use `--no-fail-fast` when hunting, or cargo stops at the first failing test binary.
- Perf harness: `cargo test --release --test perf -- --ignored --nocapture` (`tests/perf.rs`, ignored tests, release build). Run it before and after any change to update, save, view or history and put the numbers in the commit body. Bars used so far: under 1 ms per message and under 5 ms per view at 5k Cards is fine. "View + warm layout" is the per-frame cost to watch; the simulator row is a cold layout (startup or Board switch). Delete `target\release` afterwards (disk).

## After the review passes (debug, UX, UI, performance)
- `validate_ui` also drops `column_menu` / `limit_input` whose Column isn't on the current Board (undo/redo can remove Columns); any new per-Column UI state needs the same check. Anything that hides the Columns view (the Archive view) also clears `board_menu` and `filter_menu`: hidden state still takes an Esc.
- Every arm that changes `current_board` (CreateBoard included) calls `commit_pending()` before the switch. `Data::fallback(None, …)` returns the first active Board: `current_board` is `None` only when no Board is active. `ArchiveBoard` expands the sidebar's Archived section; `RestoreBoard` opens the restored Board.
- The Archive-view key gate passes Esc, Undo, Redo and F1; add any new global shortcut there too. `StartEdit` closes both the Board menu and the Column menu.
- `WindowState` ignores maximised answers while minimised (0×0 size or parked position); treat a minimised report as carrying no information. `WindowState::size()` is the last reported size; `App::board_width()` (window less sidebar) drives width-dependent layout: below `NARROW` (640) the filter bar gets its own line.
- `App::open_selected()` opens the selected Card in place and returns it; F2, E and L go through it. `App::open_card()` is the open Card's ID (the selected Card while `App.open` is set).
- `Filter::matches(card, labels)` takes the Board's Labels: the text also matches Label names. Capture box paste: `CapturePasted(String)` → `clipboard::read()` → `CaptureLines(Option<String>)`, one Card per non-blank line as one step. `commit_pending` stores a whitespace-only description as `""`; `description.is_empty()` is the one "no description" rule.
- `LimitInput { column, text, refused }`: `refused` shows the reason line until the text changes. `Column::limit_tip()` is the count's tooltip; `app::count_badge(label, over)` draws the count in the Board view and the image export alike. `chrome::tip` takes `impl text::IntoFragment`.
- `board_name_input(name)` returns a Row (text box + Create, disabled while blank); set its width on the row. `Screen::title()` builds the window title. Label tests that need an unattached Label use `new_free_label`.
- Surfaces: light theme is `chrome::LIGHT` (custom palette, page `#f7f7f5`); widgets read tiers, never hex: page = `background.base`, page = `base`, Column = `weaker`, Card and raised buttons = `weakest` (white in light), hairline border = `weak`; both themes are custom (`chrome::LIGHT`, `chrome::DARK`, built by `chrome::layers`). Columns fill the Board height; "+ Add Card" sits inside the Cards' scrollable after the last Card. Muted text buttons use `app::faded(alpha)`.
- Styles: `app::quiet` is the secondary button style (no `button::secondary`; primary only for a view's or dialog's one main action). `app::HEADING` (semibold) for Board and Column names and open-Card section headings, not Card titles; the image export uses it too. Scrollables use `app::scrollbar(width)` (embedded, 4 px spacing) so they never cover content. Tooltips and `clipped` use `bordered_box`. Close/remove glyph is "✕" everywhere; Checklist progress is "✓ n/m". `text::secondary` is for muted context, not headings or 12 px text in light theme.
- Dialogs with a `space::horizontal()` (right-aligned buttons, `dialog_header`) need a fixed `width`, not `max_width`, or they stretch to the window. Controls that appear and disappear in a row keep their slot (transparent and inert when off) so neighbours don't shift. Drop indicators stay always laid out.

## v2 (`docs/spec-v2.md`)
- Open Card: `App.open: bool`; the open Card is the selected Card while `open` is set (`App::open_card()`), and only an explicit open shows it: a click only selects (a click inside the open Card keeps it open), and arrows or a click elsewhere close it, so navigating shows faces. `validate_ui` clears `open` whenever no Card is selected. It is also cleared by Esc, `CloseCard`, a Board switch (`left_board`), the Card leaving (`card_leaves`), `OpenCapture`, and a Selection cleared to `None` (`validate_ui`). `App::view_open_card(card, title)` is its body: title, a rule, Labels, description, Checklist, Due, Files, a rule, then `column![actions, times]`; new per-Card UI goes in that order, each as `app::section(app::section_heading(..), body)` with `app::small_button` for its "+ Add …" and actions. Enter opens (`OpenCard`), the X (`CloseCard`) or Esc closes. `app::local_time` formats timestamps. ADR 0004.
- Face: `card_face` builds a `meta` list (due badge, Label chips, Checklist progress, files marker) at `META_SIZE`, drawn by `metadata_row`; badges are `app::pill(label, Tone)` (`count_badge` and the due badge); the description's first line comes from `description::face_preview`. Sizes, radii and gaps are the constants beside `COLUMN_WIDTH` (`BOARD_TITLE_SIZE`, `COLUMN_NAME_SIZE`, `CARD_TITLE_SIZE`, `META_SIZE`, `COLUMN_RADIUS`, `CARD_RADIUS`, `COLUMN_GAP`, `CARD_GAP`, `STRIP_WIDTH`); no new literals for those roles. Column selection is `header_tint`; `outlined` is for Cards only. Theme tests check `extended_palette().is_dark` or `Skin::of`.
- Ageing: `domain::STALE_DAYS` (14) and `Card::is_stale(now)` are the one rule; `app::fade_text(style, alpha)` fades the face text (`STALE_ALPHA`; a lifted Card 0.4) on top of `card_style`, never inside it. Read the clock once per view and pass `now` and `today = due::today(now)` down (`view_column(col, now, today)`, `card_face(.., today)`, `image::view`). `Data::move_card` touches the moved Card unless its position is unchanged; neighbours are never touched.
- Due date (`src/due.rs`): `Card.due: Option<String>` ISO date. `due::parse` (three typed formats, `None` for invalid or blank), `due::status(due, today)`, `due::badge_text`, `due::badge(card, today)`, `Data::set_due`. `Field::Due(Id)` is a pending edit that never writes live text to data (stored form differs from typed form): `DueChanged(Id, String)` starts the edit if needed (dropping `StartEdit`'s focus task, which would move the cursor to the end) and forwards to `EditChanged`; Enter commits; a bad date reverts and records nothing. Pattern for any always-visible box that is a pending edit.
- Files (`src/files.rs`): `Card.files: Vec<String>` absolute paths. `Data::add_file` (no duplicates) and `remove_file` touch the Card themselves. Messages `AddFile(Id)` -> rfd Open dialog task -> `FileChosen(Id, Option<PathBuf>)`; `RemoveFile(Id, usize)`; `OpenFile(String)` via `chrome::open_path`. `files::marker(card)` is the face marker; a missing file renders inert with " (not found)".
- Collapse: `Column.collapsed` (`skip_serializing_if = "std::ops::Not::not"`), `Data::toggle_collapse`, `Message::ToggleCollapse(Id)` from the menu ("Collapse") or a click on the strip. `App::view_strip(col, selected, dragging)` at the top of `view_column` wraps `app::strip_face(col, count)`, which the image export uses too; the whole strip is the Column's drag handle and its drop target (drop = append at `cards.len()`, skipping `filter::placement`). A collapsed Column has no visible Cards: Left/Right land on the Column, `validate_selection` drops a selected Card there. Anything that must show a Card or a box inside a Column calls `App::expand(column)` first (expands a collapsed Column as an undo step): the Search pick and `OpenCapture`. Enter in the Filter skips collapsed Columns. `storage::VERSION` is 2 (`pub(crate)`, shared by Board export); v1 files load unchanged.
- Duplicate: `Data::duplicate_card(card)` clones the struct, then fresh IDs, Items unticked, timestamps now; new per-Card fields come along unless they must not. `DuplicateCard` from Ctrl+D or the open Card's button; selects the copy.
- Search (`src/search.rs`): `App.search: Option<Search { text, highlight }>`; `search::hits(&Data, &str) -> Vec<Hit>` over all Boards (Archive hits have `column: None`), nothing under two characters. View priority is Search, then Archive view, then Board. `SearchPick` runs outside `apply` (nested `update`s) because `validate_ui`'s `left_board` would wipe the Selection after a Board switch. `allowed` gates keys while Search is open; new global shortcuts need adding there. Esc order: see "Esc order (current)". Reveal an archived Card with `app::reveal(archive::SCROLL.into(), archive::row_widget(card), false)`.
- Capture: titles go through `Data::add_captured` (`labels::label_tokens` strips `#name` words that match a Label case-insensitively; unmatched stay; a title with no match or only matches comes back untouched, Labels still attached); `add_card` stays unaware.
- Column keys: Ctrl+Left/Right with a Column selected and no Card moves the Column (`App::column_move`); it is no longer a no-op.
- Tests: feature files with local helpers are fine (`tests/duplicate.rs`, `files.rs`, `due.rs`, `collapse.rs`, `ageing.rs`, `search.rs`, `open_card.rs`). Card and Column literals in tests need `due`, `files`, `collapsed`. `Data::add_label` takes a palette key ("red"), not hex. `tests/keys.rs` `press()` maps letters to physical keys; add new letters there.

## Windows tooling gotchas
- In worktrees, Bash heredocs and multi-step git commands may be refused by the isolation check; use Write/Edit and simple git commands.
- PowerShell 5.1: `git commit -F -` with a here-string breaks; write the message to a file and use `-F <file>`.
- Disk: parallel worktrees share the main checkout's `target/` (a per-worktree target is ~3 GB), but the `kanvibe` output file name is identical across worktrees, so one worktree tests another's code. Give each worktree its own output with `--config "profile.dev.package.kanvibe.codegen-units=N"` (distinct N per worktree) on every `cargo test` / `cargo clippy`. Set the variable from PowerShell (`$env:CARGO_TARGET_DIR`); Bash commands that set it inline are refused by the worktree isolation check.
- Long worktree paths can hit MAX_PATH (LNK1104) on release builds; use a shorter `--target-dir`.
- Read all of `ci.bat`'s output (`$out = .\ci.bat 2>&1 | Out-String`, then filter). Piping it into `Select-Object -First N` closes the pipe early and kills the batch mid-run with exit 255.
