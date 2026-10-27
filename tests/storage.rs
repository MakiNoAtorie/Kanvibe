//! Seam 2: storage against a temporary data folder, passed as `--data-dir` like the app gets it.
use std::fs;
use std::path::{Path, PathBuf};

use kanvibe::app::{App, Closing, Message};
use kanvibe::domain::Data;
use kanvibe::startup::{self, Problem, Screen};
use kanvibe::storage::{self, LoadError, Settings};

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

/// What the iced update function does: `update`, then start a save if anything changed. Here
/// the background writer's job runs at once and its result goes back through `update`.
fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
        while let Some(job) = app.save_if_changed() {
            let _ = app.update(job.run());
        }
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

#[test]
fn second_copy_cannot_take_the_lock_until_the_first_lets_go() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Kanvibe");

    let first = storage::lock(&dir).unwrap();
    assert!(first.is_some(), "creates the folder and takes the lock");
    assert!(storage::lock(&dir).unwrap().is_none());

    drop(first);
    assert!(storage::lock(&dir).unwrap().is_some());
    assert!(dir.join("kanvibe.lock").exists());
}

#[test]
fn newer_version_is_refused_and_the_file_left_alone() {
    let (_tmp, dir) = data_dir();
    let json = r#"{ "version": 2, "next_id": 1, "boards": [], "future": true }"#;
    fs::write(dir.join("kanvibe.json"), json).unwrap();

    assert!(matches!(storage::load(&dir), Err(LoadError::Newer)));
    assert_eq!(fs::read_to_string(dir.join("kanvibe.json")).unwrap(), json);
}

#[test]
fn content_errors_are_unreadable() {
    let (_tmp, dir) = data_dir();
    for json in [
        "{ not json",
        r#"{ "next_id": 1 }"#,
        r#"{ "version": 1, "next_id": 1, "boards": [ { "id": "one" } ] }"#,
    ] {
        fs::write(dir.join("kanvibe.json"), json).unwrap();
        let result = storage::load(&dir);
        assert!(
            matches!(result, Err(LoadError::Unreadable(_))),
            "{json}: {result:?}"
        );
    }
}

#[test]
fn missing_file_with_a_backup_is_unreadable_not_first_run() {
    let (_tmp, dir) = data_dir();
    fs::write(dir.join("kanvibe.json.bak2"), "anything").unwrap();

    assert!(matches!(storage::load(&dir), Err(LoadError::Unreadable(_))));
    assert!(!dir.join("kanvibe.json").exists());
}

/// A data file that can't be read at all (here: a folder in its place) is an I/O error,
/// reported only after retrying for about a second.
#[test]
fn unreadable_file_system_entry_is_an_io_error_after_retrying() {
    let (_tmp, dir) = data_dir();
    fs::create_dir(dir.join("kanvibe.json")).unwrap();
    let start = std::time::Instant::now();

    assert!(matches!(storage::load(&dir), Err(LoadError::Io(_))));
    assert!(start.elapsed() >= std::time::Duration::from_millis(900));
}

/// Saves `data` as the data file and copies it to `name`, the way backup rotation leaves it.
fn backup(dir: &Path, name: &str, data: &Data) {
    storage::save(dir, data, &Settings::default()).unwrap();
    fs::rename(dir.join("kanvibe.json"), dir.join(name)).unwrap();
}

/// Names of the `kanvibe.json.corrupt-<timestamp>` files and their contents.
fn corrupt_files(dir: &Path) -> Vec<(String, String)> {
    let mut found: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            name.starts_with("kanvibe.json.corrupt-")
        })
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap().to_owned();
            (name, fs::read_to_string(&p).unwrap())
        })
        .collect();
    found.sort();
    found
}

#[test]
fn restore_offers_the_newest_readable_backup_and_none_when_none_is_readable() {
    let (_tmp, dir) = data_dir();
    assert!(storage::backup(&dir).is_none());

    let older = sample();
    let mut newer = sample();
    newer.create_board("Newer");
    backup(&dir, "kanvibe.json.bak2", &older);
    backup(&dir, "kanvibe.json.bak1", &newer);
    assert_eq!(
        storage::backup(&dir).unwrap().0,
        dir.join("kanvibe.json.bak1")
    );

    fs::write(dir.join("kanvibe.json.bak1"), "{ broken").unwrap();
    assert_eq!(
        storage::backup(&dir).unwrap().0,
        dir.join("kanvibe.json.bak2")
    );

    fs::write(dir.join("kanvibe.json.bak2"), r#"{ "version": 9 }"#).unwrap();
    assert!(storage::backup(&dir).is_none());
}

#[test]
fn restore_moves_the_bad_file_aside_then_copies_the_newest_readable_backup_over_it() {
    let (_tmp, dir) = data_dir();
    let older = sample();
    let mut newer = sample();
    newer.create_board("Newer");
    backup(&dir, "kanvibe.json.bak2", &older);
    backup(&dir, "kanvibe.json.bak1", &newer);
    fs::write(dir.join("kanvibe.json"), "{ broken").unwrap();

    storage::restore(&dir).unwrap();

    assert_eq!(storage::load(&dir).unwrap().unwrap().0, newer);
    let corrupt = corrupt_files(&dir);
    assert_eq!(corrupt.len(), 1);
    assert_eq!(corrupt[0].1, "{ broken");
    let stamp = corrupt[0].0.strip_prefix("kanvibe.json.corrupt-").unwrap();
    let shape: String = stamp
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect();
    assert_eq!(shape, "99999999T999999Z", "UTC timestamp, no colons");
    assert!(dir.join("kanvibe.json.bak1").exists(), "backups are kept");
}

#[test]
fn restore_works_when_the_data_file_is_missing() {
    let (_tmp, dir) = data_dir();
    let data = sample();
    backup(&dir, "kanvibe.json.bak1", &data);

    storage::restore(&dir).unwrap();

    assert_eq!(storage::load(&dir).unwrap().unwrap().0, data);
    assert!(corrupt_files(&dir).is_empty());
}

#[test]
fn start_empty_moves_the_bad_file_aside_and_keeps_older_corrupt_files() {
    let (_tmp, dir) = data_dir();
    let old = "kanvibe.json.corrupt-20200101T000000Z";
    fs::write(dir.join(old), "old").unwrap();
    fs::write(dir.join("kanvibe.json"), "{ broken").unwrap();

    storage::start_empty(&dir).unwrap();

    assert_eq!(storage::load(&dir).unwrap().unwrap().0, Data::default());
    let corrupt = corrupt_files(&dir);
    assert_eq!(corrupt.len(), 2);
    assert_eq!(corrupt[0], (old.to_owned(), "old".to_owned()));
    assert_eq!(corrupt[1].1, "{ broken");
}

/// The startup problem screen `Screen::open` shows, or `None` when the app opens.
fn problem(screen: &Screen) -> Option<&Problem> {
    match screen {
        Screen::App(_) => None,
        Screen::Problem(problem) => Some(problem),
    }
}

fn opened(screen: Screen) -> App {
    match screen {
        Screen::App(app) => app,
        Screen::Problem(problem) => panic!("expected the app, got {problem:?}"),
    }
}

#[test]
fn each_load_class_reaches_its_screen() {
    let (_tmp, dir) = data_dir();
    assert!(
        opened(Screen::open(dir.clone())).data.boards.is_empty(),
        "first run"
    );

    fs::write(dir.join("kanvibe.json"), r#"{ "version": 2 }"#).unwrap();
    let screen = Screen::open(dir.clone());
    assert!(matches!(problem(&screen).unwrap().error, LoadError::Newer));

    fs::write(dir.join("kanvibe.json"), "{ broken").unwrap();
    let screen = Screen::open(dir.clone());
    let unreadable = problem(&screen).unwrap();
    assert!(matches!(unreadable.error, LoadError::Unreadable(_)));
    assert!(
        unreadable.backup.is_none(),
        "no Restore without a readable backup"
    );

    fs::remove_file(dir.join("kanvibe.json")).unwrap();
    backup(&dir, "kanvibe.json.bak1", &sample());
    let screen = Screen::open(dir.clone());
    let missing = problem(&screen).unwrap();
    assert!(matches!(missing.error, LoadError::Unreadable(_)));
    assert_eq!(
        missing.backup.as_ref().unwrap().0,
        dir.join("kanvibe.json.bak1")
    );

    fs::create_dir(dir.join("kanvibe.json")).unwrap();
    let mut screen = Screen::open(dir.clone());
    assert!(matches!(problem(&screen).unwrap().error, LoadError::Io(_)));

    fs::remove_dir(dir.join("kanvibe.json")).unwrap();
    storage::save(&dir, &sample(), &Settings::default()).unwrap();
    let _ = screen.update(startup::Message::Retry);
    assert_eq!(opened(screen).data, sample(), "Retry loads again");
}

#[test]
fn restore_and_start_empty_open_the_app() {
    let (_tmp, dir) = data_dir();
    backup(&dir, "kanvibe.json.bak1", &sample());
    fs::write(dir.join("kanvibe.json"), "{ broken").unwrap();

    let mut screen = Screen::open(dir.clone());
    let _ = screen.update(startup::Message::Restore);
    assert_eq!(opened(screen).data, sample());

    fs::write(dir.join("kanvibe.json"), "{ broken again").unwrap();
    let mut screen = Screen::open(dir.clone());
    let _ = screen.update(startup::Message::StartEmpty);
    assert_eq!(opened(screen).data, Data::default());
    assert_eq!(corrupt_files(&dir).len(), 2);
}

/// An indexer or antivirus briefly holding the data file must not end in an error screen.
#[cfg(windows)]
#[test]
fn load_retries_while_another_process_holds_the_file() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::Duration;

    let (_tmp, dir) = data_dir();
    let data = sample();
    storage::save(&dir, &data, &Settings::default()).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.join("kanvibe.json"))
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });

    let loaded = storage::load(&dir).unwrap().unwrap().0;

    release.join().unwrap();
    assert_eq!(loaded, data);
}

// --- Save failure (#21) ---

/// A folder where the save writes its temporary file makes every save fail at once with an OS
/// error. Made after `App::open`, which deletes a leftover `.tmp`.
fn block_saves(dir: &Path) {
    fs::create_dir(dir.join("kanvibe.json.tmp")).unwrap();
}

fn unblock_saves(dir: &Path) {
    fs::remove_dir(dir.join("kanvibe.json.tmp")).unwrap();
}

fn create_board(app: &mut App, name: &str) {
    send(
        app,
        [
            Message::NewBoard,
            Message::BoardNameChanged(name.into()),
            Message::CreateBoard,
        ],
    );
}

fn add_card(app: &mut App, title: &str) {
    let todo = app.data.boards[0].columns[0].id;
    send(
        app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged(title.into()),
            Message::SubmitCapture,
        ],
    );
}

#[test]
fn a_failed_save_keeps_the_change_shows_the_bar_and_retries_on_the_next_change() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    block_saves(&dir);

    create_board(&mut app, "Work");

    assert_eq!(app.data.boards[0].name, "Work", "kept in memory");
    assert!(!dir.join("kanvibe.json").exists());
    let bar = app.not_saved.clone().expect("the Not saved bar");
    let prefix = format!("Not saved: {}: ", dir.join("kanvibe.json").display());
    assert!(
        bar.starts_with(&prefix) && bar.len() > prefix.len(),
        "{bar}"
    );

    unblock_saves(&dir);
    assert!(
        app.save_if_changed().is_none(),
        "no retry until a change or the timer"
    );

    add_card(&mut app, "Call Ann");

    assert_eq!(app.not_saved, None, "the bar clears on success");
    assert_eq!(storage::load(&dir).unwrap().unwrap().0, app.data);
}

/// `update` without running the save it starts: the writer is still busy.
fn send_while_writing(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
        assert!(app.save_if_changed().is_none(), "one save in flight");
    }
}

#[test]
fn changes_during_a_write_merge_into_one_save_of_the_latest_state() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    create_board(&mut app, "Work");
    let todo = app.data.boards[0].columns[0].id;
    let _ = app.update(Message::OpenCapture(todo));
    let _ = app.update(Message::CaptureChanged("A".into()));
    let _ = app.update(Message::SubmitCapture);
    let first = app.save_if_changed().expect("a save starts");

    send_while_writing(
        &mut app,
        [
            Message::CaptureChanged("B".into()),
            Message::SubmitCapture,
            Message::CaptureChanged("C".into()),
            Message::SubmitCapture,
        ],
    );
    let _ = app.update(first.run());
    let second = app.save_if_changed().expect("the merged save");
    let _ = app.update(second.run());

    assert!(app.save_if_changed().is_none(), "nothing left to save");
    let saved = storage::load(&dir).unwrap().unwrap().0;
    let titles: Vec<_> = saved.boards[0].columns[0]
        .cards
        .iter()
        .map(|c| c.title.as_str())
        .collect();
    assert_eq!(titles, ["A", "B", "C"]);
}

#[test]
fn closing_waits_for_the_save_in_flight_then_exits() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    let _ = app.update(Message::NewBoard);
    let _ = app.update(Message::BoardNameChanged("Work".into()));
    let _ = app.update(Message::CreateBoard);
    let job = app.save_if_changed().expect("a save starts");

    send_while_writing(&mut app, [Message::CloseRequested]);
    assert_eq!(app.closing, Closing::Waiting);

    let _ = app.update(job.run());

    assert_eq!(app.closing, Closing::Exiting);
    assert_eq!(storage::load(&dir).unwrap().unwrap().0, app.data);
}

#[test]
fn closing_with_nothing_unsaved_exits_at_once() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir).unwrap();
    create_board(&mut app, "Work");

    send(&mut app, [Message::CloseRequested]);

    assert_eq!(app.closing, Closing::Exiting);
}

/// Closes while saves fail: the close commits the pending rename, its save fails, the dialog asks.
fn close_unsaved(dir: &Path) -> App {
    use kanvibe::app::Field;
    let mut app = App::open(dir.to_owned()).unwrap();
    create_board(&mut app, "Work");
    block_saves(dir);
    let board = Field::BoardName(app.data.boards[0].id);
    send(
        &mut app,
        [
            Message::StartEdit(board),
            Message::EditChanged("Home".into()),
            Message::CloseRequested,
        ],
    );
    assert_eq!(app.data.boards[0].name, "Home");
    assert_eq!(app.editing, None, "committed");
    assert_eq!(
        app.closing,
        Closing::Asking,
        "Retry / Save a copy as… / Quit"
    );
    app
}

#[test]
fn closing_while_unsaved_asks_and_retry_saves_then_exits() {
    let (_tmp, dir) = data_dir();
    let mut app = close_unsaved(&dir);

    send(&mut app, [Message::RetrySave]);
    assert_eq!(app.closing, Closing::Asking, "still failing");

    unblock_saves(&dir);
    send(&mut app, [Message::RetrySave]);

    assert_eq!(app.closing, Closing::Exiting);
    assert_eq!(
        storage::load(&dir).unwrap().unwrap().0.boards[0].name,
        "Home"
    );
}

#[test]
fn closing_while_unsaved_quits_without_saving() {
    let (_tmp, dir) = data_dir();
    let mut app = close_unsaved(&dir);

    send(&mut app, [Message::QuitWithoutSaving]);

    assert_eq!(app.closing, Closing::Exiting);
}

#[test]
fn closing_while_unsaved_exits_once_a_copy_is_saved() {
    let (_tmp, dir) = data_dir();
    let mut app = close_unsaved(&dir);

    send(&mut app, [Message::CopySaved(None)]);
    assert_eq!(app.closing, Closing::Asking, "Save dialog cancelled");

    send(
        &mut app,
        [Message::CopySaved(Some(Err("Not saved: X: full".into())))],
    );
    assert_eq!(app.closing, Closing::Asking);
    assert_eq!(app.not_saved.as_deref(), Some("Not saved: X: full"));

    send(&mut app, [Message::CopySaved(Some(Ok(())))]);
    assert_eq!(app.closing, Closing::Exiting);
}

#[test]
fn esc_dismisses_the_close_dialog_and_keeps_the_app_open() {
    let (_tmp, dir) = data_dir();
    let mut app = close_unsaved(&dir);

    send(&mut app, [Message::Escape]);

    assert_eq!(app.closing, Closing::No);
    assert!(app.not_saved.is_some(), "the bar stays");
}

#[test]
fn a_saved_copy_loads_like_the_data_file() {
    let (_tmp, dir) = data_dir();
    let copy = dir.join("elsewhere");
    fs::create_dir(&copy).unwrap();
    let data = sample();

    storage::save_copy(&copy.join("kanvibe.json"), &data, &Settings::default()).unwrap();

    assert_eq!(storage::load(&copy).unwrap().unwrap().0, data);
}

#[test]
fn a_failed_save_is_retried_by_the_timer_without_a_change() {
    let (_tmp, dir) = data_dir();
    let mut app = App::open(dir.clone()).unwrap();
    block_saves(&dir);
    create_board(&mut app, "Work");

    send(&mut app, [Message::RetrySave]);
    assert!(app.not_saved.is_some(), "still failing");

    unblock_saves(&dir);
    send(&mut app, [Message::RetrySave]);

    assert_eq!(app.not_saved, None);
    assert_eq!(storage::load(&dir).unwrap().unwrap().0, app.data);
}
