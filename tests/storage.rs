//! Seam 2: storage against a temporary data folder, passed as `--data-dir` like the app gets it.
use std::fs;
use std::path::PathBuf;

use kanvibe::app::{App, Message};
use kanvibe::domain::Data;
use kanvibe::storage::{self, Settings};

/// A fresh temporary folder and the data folder the app resolves from `--data-dir <it>`.
fn data_dir() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let args = ["kanvibe", "--data-dir", tmp.path().to_str().unwrap()].map(String::from);
    let dir = storage::data_dir(args).unwrap();
    assert_eq!(dir, tmp.path());
    (tmp, dir)
}

fn sample() -> Data {
    let mut data = Data::default();
    data.create_board("Work");
    let board = data.create_board("Home").unwrap();
    let doing = data.boards[1].columns[1].id;
    data.add_card(doing, "Paint fence");
    data.add_card(doing, "Fix tap");
    assert_eq!(data.boards[1].id, board);
    data
}

#[test]
fn round_trip_keeps_boards_columns_cards_and_next_id_pretty_printed() {
    let (_tmp, dir) = data_dir();
    let data = sample();
    let settings = Settings {
        last_board: Some(data.boards[1].id),
    };

    storage::save(&dir, &data, &settings).unwrap();
    let (loaded, loaded_settings) = storage::load(&dir).unwrap().unwrap();

    assert_eq!(loaded, data);
    assert_eq!(loaded.next_id, 10);
    assert_eq!(loaded_settings, settings);
    let text = fs::read_to_string(dir.join("kanvibe.json")).unwrap();
    assert!(text.starts_with("{\n  \"version\": 1,\n"), "{text}");
}

/// What the iced update function does: `update`, then save if anything changed.
fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
        app.save_if_changed();
    }
}

#[test]
fn app_reopens_the_last_open_board() {
    let (_tmp, dir) = data_dir();
    let data = sample();
    let home = data.boards[1].id;
    storage::save(
        &dir,
        &data,
        &Settings {
            last_board: Some(home),
        },
    )
    .unwrap();

    let app = App::open(dir).unwrap();

    assert_eq!(app.data, data);
    assert_eq!(app.current_board, Some(home));
}

#[test]
fn app_opens_the_first_board_when_the_last_board_is_unknown_or_unset() {
    let (_tmp, dir) = data_dir();
    let data = sample();
    for last_board in [Some(999), None] {
        storage::save(&dir, &data, &Settings { last_board }).unwrap();
        let app = App::open(dir.clone()).unwrap();
        assert_eq!(app.current_board, Some(data.boards[0].id));
    }
}

#[test]
fn every_committed_change_is_saved_and_reloaded_on_the_next_start() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    assert!(app.data.boards.is_empty());
    send(
        &mut app,
        [Message::NewBoard, Message::BoardNameChanged("Work".into())],
    );
    assert!(
        storage::load(&dir).unwrap().is_none(),
        "nothing committed yet"
    );

    send(&mut app, [Message::CreateBoard]);
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("Call Ann".into()),
            Message::SubmitCapture,
        ],
    );

    let reopened = App::open(dir).unwrap();
    assert_eq!(reopened.data, app.data);
    assert_eq!(
        reopened.data.boards[0].columns[0].cards[0].title,
        "Call Ann"
    );
    assert_eq!(reopened.current_board, app.current_board);
}

#[test]
fn missing_file_is_first_run() {
    let (_tmp, dir) = data_dir();
    assert!(storage::load(&dir).unwrap().is_none());
}

#[test]
fn file_with_utf8_bom_and_missing_optional_fields_loads() {
    let (_tmp, dir) = data_dir();
    let json = r#"{ "version": 1, "next_id": 7,
        "boards": [ { "id": 1, "name": "Work", "columns": [
            { "id": 2, "name": "To do", "cards": [ { "id": 5, "title": "Call Ann" } ] },
            { "id": 3, "name": "Done" } ] } ] }"#;
    fs::write(
        dir.join("kanvibe.json"),
        [b"\xEF\xBB\xBF", json.as_bytes()].concat(),
    )
    .unwrap();

    let (data, settings) = storage::load(&dir).unwrap().unwrap();

    assert_eq!(data.next_id, 7);
    assert_eq!(data.boards[0].columns[0].cards[0].title, "Call Ann");
    assert!(data.boards[0].columns[1].cards.is_empty());
    assert_eq!(settings, Settings::default());
}

#[test]
fn leftover_tmp_is_removed_at_startup_and_never_loaded() {
    let (_tmp, dir) = data_dir();
    let tmp_file = dir.join("kanvibe.json.tmp");
    fs::write(
        &tmp_file,
        r#"{ "next_id": 1, "boards": [ { "id": 1, "name": "Half" } ] }"#,
    )
    .unwrap();

    assert!(storage::load(&dir).unwrap().is_none());
    assert!(!tmp_file.exists());
}

#[test]
fn save_replaces_the_data_file_and_leaves_no_tmp() {
    let (_tmp, dir) = data_dir();
    storage::save(&dir, &Data::default(), &Settings::default()).unwrap();
    let data = sample();

    storage::save(&dir, &data, &Settings::default()).unwrap();

    assert_eq!(storage::load(&dir).unwrap().unwrap().0, data);
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["kanvibe.json"]);
}

/// An indexer or antivirus holding the data file open without sharing makes the rename fail
/// with a sharing violation until it lets go.
#[cfg(windows)]
#[test]
fn save_retries_the_rename_while_another_process_holds_the_file() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::Duration;

    let (_tmp, dir) = data_dir();
    storage::save(&dir, &Data::default(), &Settings::default()).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.join("kanvibe.json"))
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    let data = sample();

    storage::save(&dir, &data, &Settings::default()).unwrap();

    release.join().unwrap();
    assert_eq!(storage::load(&dir).unwrap().unwrap().0, data);
}

// --- Text edits (#6) ---

#[test]
fn a_pending_rename_reaches_disk_one_idle_second_after_the_last_keystroke_and_on_close() {
    use kanvibe::app::Field;
    use std::time::{Duration, Instant};

    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    let saved_name = || {
        storage::load(&dir).unwrap().unwrap().0.boards[0]
            .name
            .clone()
    };
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    let board = Field::BoardName(app.data.boards[0].id);

    send(
        &mut app,
        [
            Message::StartEdit(board),
            Message::EditChanged("Home".into()),
            Message::EditTick(Instant::now()),
        ],
    );
    assert_eq!(saved_name(), "Work", "not yet a second idle");

    send(
        &mut app,
        [Message::EditTick(Instant::now() + Duration::from_secs(1))],
    );
    assert_eq!(saved_name(), "Home");
    assert!(app.editing.is_some(), "still pending");

    send(
        &mut app,
        [
            Message::EditChanged("Homes".into()),
            Message::CloseRequested,
        ],
    );
    assert_eq!(saved_name(), "Homes");
    assert_eq!(app.editing, None);
}
