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
- iced 0.14 API notes: see the orchestrator's scratchpad `notes/iced-0.14.md`. `time::every` needs the iced `tokio` feature.

## Tests
- Integration tests in `tests/`, public API only. Seam 1: `tests/update.rs`. Storage seam: `tests/storage.rs`.
- `tests/update.rs` helpers: `send(&mut app, [msgs])`, `app_with_board()` (one current Board "Work"), `column_id`, `card_titles`, `capture(app, column, &[titles])`.

## Checks
- `cargo test --offline -q` and `cargo clippy --offline --all-targets -- -D warnings`; `cargo fmt` before committing.

## Windows tooling gotchas
- PowerShell 5.1: `git commit -F -` with a here-string breaks; write the message to a file and use `-F <file>`.
- Concurrent cargo builds sharing one `target` dir fail with LNK1104; use a separate `CARGO_TARGET_DIR` per build.
