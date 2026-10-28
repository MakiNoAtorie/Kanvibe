//! Files on a Card (v2 item 6): paths added through the Open dialog, removed, kept everywhere.
use std::path::PathBuf;

use kanvibe::app::{App, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::storage::{self, Settings};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Boards "Work" (To do: A, B) and "Home", "Work" current, with A open.
fn app_with_open_card() -> App {
    let mut app = App::default();
    for name in ["Home", "Work"] {
        send(
            &mut app,
            [
                Message::NewBoard,
                Message::BoardNameChanged(name.into()),
                Message::CreateBoard,
            ],
        );
    }
    let todo = app.data.boards[1].columns[0].id;
    for title in ["A", "B"] {
        send(
            &mut app,
            [
                Message::OpenCapture(todo),
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
                Message::Escape,
            ],
        );
    }
    let a = app.data.boards[1].columns[0].cards[0].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column: todo,
            card: Some(a),
        })],
    );
    assert_eq!(app.open_card(), Some(a));
    app
}

fn files(app: &App, card: Id) -> Vec<String> {
    let found = app.data.card(card).map(|c| c.files.clone());
    let archived = app.data.boards.iter().flat_map(|b| &b.archived_cards);
    found
        .or_else(|| {
            archived
                .map(|a| &a.card)
                .find(|c| c.id == card)
                .map(|c| c.files.clone())
        })
        .unwrap()
}

const REPORT: &str = r"C:\Docs\report.pdf";
const NOTES: &str = r"C:\Docs\notes.txt";

fn choose(app: &mut App, card: Id, path: &str) {
    send(app, [Message::FileChosen(card, Some(PathBuf::from(path)))]);
}

#[test]
fn a_chosen_file_is_added_once_and_is_one_undo_step() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    app.data.boards[1].columns[0].cards[0].updated = "2020-01-01T00:00:00Z".into();
    app.history = Default::default();

    choose(&mut app, a, REPORT);
    assert_eq!(files(&app, a), [REPORT]);
    assert_ne!(app.data.card(a).unwrap().updated, "2020-01-01T00:00:00Z");

    choose(&mut app, a, NOTES);
    choose(&mut app, a, REPORT);
    assert_eq!(files(&app, a), [REPORT, NOTES], "no duplicates");

    send(&mut app, [Message::Undo]);
    assert_eq!(files(&app, a), [REPORT], "the duplicate recorded no step");
    send(&mut app, [Message::Undo]);
    assert!(files(&app, a).is_empty());
}

#[test]
fn a_cancelled_dialog_adds_nothing() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    let before = app.data.clone();
    send(&mut app, [Message::FileChosen(a, None)]);
    assert_eq!(app.data, before);
}

#[test]
fn removing_a_file_is_one_undo_step() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    choose(&mut app, a, REPORT);
    choose(&mut app, a, NOTES);

    send(&mut app, [Message::RemoveFile(a, 0)]);
    assert_eq!(files(&app, a), [NOTES]);
    send(&mut app, [Message::RemoveFile(a, 5)]);
    assert_eq!(files(&app, a), [NOTES], "an unknown index does nothing");

    send(&mut app, [Message::Undo]);
    assert_eq!(files(&app, a), [REPORT, NOTES]);
}

#[test]
fn files_survive_move_to_board_and_archive_and_restore() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    choose(&mut app, a, REPORT);

    let home = app.data.boards[0].id;
    send(&mut app, [Message::MoveToBoard(a, home)]);
    assert_eq!(app.data.board_of_card(a), Some(home));
    assert_eq!(files(&app, a), [REPORT]);

    send(&mut app, [Message::ArchiveCard(a)]);
    assert!(app.data.card(a).is_none());
    assert_eq!(files(&app, a), [REPORT]);
    send(&mut app, [Message::Restore(a)]);
    assert!(app.data.card(a).is_some());
    assert_eq!(files(&app, a), [REPORT]);
}

#[test]
fn files_round_trip_through_storage_and_a_v1_card_has_none() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    choose(&mut app, a, REPORT);
    let tmp = tempfile::tempdir().unwrap();

    storage::save(tmp.path(), &app.data, &Settings::default()).unwrap();
    let text = std::fs::read_to_string(tmp.path().join("kanvibe.json")).unwrap();
    assert_eq!(text.matches("\"files\"").count(), 1, "omitted when empty");
    let (loaded, _) = storage::load(tmp.path()).unwrap().unwrap();
    assert_eq!(loaded, app.data);

    let v1 = r#"{ "version": 1, "next_id": 7, "boards": [ { "id": 1, "name": "Work",
        "columns": [ { "id": 2, "name": "To do", "cards": [ { "id": 5, "title": "Old" } ] } ] } ] }"#;
    std::fs::write(tmp.path().join("kanvibe.json"), v1).unwrap();
    let (loaded, _) = storage::load(tmp.path()).unwrap().unwrap();
    assert!(loaded.boards[0].columns[0].cards[0].files.is_empty());
}

#[test]
fn export_and_import_keep_files() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    choose(&mut app, a, REPORT);
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("work.kanvibe.json");
    kanvibe::transfer::export(&path, &app.data.boards[1]).unwrap();

    let bytes = std::fs::read(path).unwrap();
    send(&mut app, [Message::Imported(Some(Ok(bytes)))]);

    let copy = app.data.boards.last().unwrap();
    assert_eq!(copy.columns[0].cards[0].title, "A");
    assert_eq!(copy.columns[0].cards[0].files, [REPORT]);
}

#[test]
fn the_open_card_lists_files_and_marks_a_missing_one() {
    let mut app = app_with_open_card();
    let a = app.open_card().unwrap();
    let b = app.data.boards[1].columns[0].cards[1].id;
    let here = std::env::current_dir().unwrap();
    choose(&mut app, a, here.join("Cargo.toml").to_str().unwrap());
    choose(&mut app, a, here.join("gone.txt").to_str().unwrap());
    choose(&mut app, b, REPORT);

    let mut ui = iced_test::simulator(app.view());
    for shown in [
        "Files",
        "Cargo.toml",
        "gone.txt (not found)",
        "+ Add file…",
        "1 file",
    ] {
        assert!(ui.find(shown).is_ok(), "{shown}");
    }
}
