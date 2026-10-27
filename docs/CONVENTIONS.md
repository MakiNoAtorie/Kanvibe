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
- Concurrent cargo builds sharing one `target` dir fail with LNK1104; use a separate `CARGO_TARGET_DIR` per build.
- Long worktree paths can hit MAX_PATH (LNK1104) on release builds; use a shorter `--target-dir`.
