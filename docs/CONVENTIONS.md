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
