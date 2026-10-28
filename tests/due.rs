//! Due date (spec v2 item 1): the pure parse/status/badge functions, the open Card's Due field
//! through `update`, and the due date surviving moves, archive, save and export.
use jiff::civil::date;
use kanvibe::app::{App, Message, Selection};
use kanvibe::domain::{Card, Id};
use kanvibe::due::{self, Status};
use kanvibe::storage::{self, Settings};
use kanvibe::transfer;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One current Board "Work"; To do holds A, selected and open, with no steps recorded.
fn app_with_open_card() -> (App, Id) {
    let mut app = App::default();
    let data = &mut app.data;
    let work = data.create_board("Work").unwrap();
    let column = data.boards[0].columns[0].id;
    let a = data.add_card(column, "A").unwrap();
    app.current_board = Some(work);
    let card = Some(a);
    send(&mut app, [Message::Select(Selection { column, card })]);
    app.history = Default::default();
    (app, a)
}

fn card(app: &App, id: Id) -> &Card {
    app.data.card(id).unwrap()
}

/// Types `text` into the open Card's Due field and presses Enter.
fn type_due(app: &mut App, id: Id, text: &str) {
    send(
        app,
        [Message::DueChanged(id, text.into()), Message::CommitEdit],
    );
}

#[test]
fn typed_dates_parse_to_iso_in_three_formats_and_nothing_else() {
    let table = [
        ("2026-10-28", Some("2026-10-28")),
        ("28/10/2026", Some("2026-10-28")),
        ("28-10-2026", Some("2026-10-28")),
        ("  05/01/2027 ", Some("2027-01-05")),
        ("31/02/2026", None),
        ("2026-02-31", None),
        ("28/10-2026", None),
        ("2026/10/28", None),
        ("5/1/2027", None),
        ("20261028", None),
        ("tomorrow", None),
        ("", None),
        ("   ", None),
    ];
    for (typed, iso) in table {
        assert_eq!(due::parse(typed).as_deref(), iso, "{typed:?}");
    }
}

#[test]
fn status_compares_with_the_given_today() {
    let today = date(2026, 10, 28);
    assert_eq!(due::status("2026-10-29", today), Some(Status::Future));
    assert_eq!(due::status("2027-01-01", today), Some(Status::Future));
    assert_eq!(due::status("2026-10-28", today), Some(Status::Today));
    assert_eq!(due::status("2026-10-27", today), Some(Status::Overdue));
    assert_eq!(due::status("2025-12-31", today), Some(Status::Overdue));
    assert_eq!(due::status("garbage", today), None);
}

#[test]
fn badge_text_is_day_and_month_with_the_year_only_when_not_this_year() {
    let today = date(2026, 10, 28);
    assert_eq!(
        due::badge_text("2026-10-28", today).as_deref(),
        Some("28 Oct")
    );
    assert_eq!(
        due::badge_text("2026-01-05", today).as_deref(),
        Some("5 Jan")
    );
    assert_eq!(
        due::badge_text("2027-01-05", today).as_deref(),
        Some("5 Jan 2027")
    );
    assert_eq!(
        due::badge_text("2025-12-31", today).as_deref(),
        Some("31 Dec 2025")
    );
    assert_eq!(due::badge_text("garbage", today), None);
}

#[test]
fn a_typed_date_commits_as_iso_on_enter_as_one_undo_step() {
    let (mut app, a) = app_with_open_card();
    let updated = card(&app, a).updated.clone();

    send(&mut app, [Message::DueChanged(a, "28/10/2026".into())]);
    assert_eq!(
        card(&app, a).due,
        None,
        "the typed text waits for the commit"
    );
    send(&mut app, [Message::CommitEdit]);

    assert_eq!(card(&app, a).due.as_deref(), Some("2026-10-28"));
    assert_eq!(app.editing, None);
    assert!(card(&app, a).updated >= updated);
    send(&mut app, [Message::Undo]);
    assert_eq!(card(&app, a).due, None);
    assert_eq!(card(&app, a).title, "A");
}

#[test]
fn a_bad_date_reverts_the_field_and_records_nothing() {
    let (mut app, a) = app_with_open_card();
    type_due(&mut app, a, "2026-10-28");
    let before = card(&app, a).clone();

    type_due(&mut app, a, "31/02/2026");

    assert_eq!(card(&app, a), &before);
    assert_eq!(app.editing, None, "the field shows the stored date again");
    send(&mut app, [Message::Undo]);
    assert_eq!(
        card(&app, a).due,
        None,
        "the one step undone was setting the date"
    );
}

#[test]
fn a_blank_field_clears_the_date_as_one_undo_step() {
    let (mut app, a) = app_with_open_card();
    type_due(&mut app, a, "28-10-2026");

    type_due(&mut app, a, "  ");

    assert_eq!(card(&app, a).due, None);
    send(&mut app, [Message::Undo]);
    assert_eq!(card(&app, a).due.as_deref(), Some("2026-10-28"));
}

#[test]
fn the_date_survives_move_to_board_and_archive_and_restore() {
    let (mut app, a) = app_with_open_card();
    type_due(&mut app, a, "2026-10-28");
    let home = app.data.create_board("Home").unwrap();

    send(&mut app, [Message::MoveToBoard(a, home)]);
    assert_eq!(app.data.board_of_card(a), Some(home));
    assert_eq!(card(&app, a).due.as_deref(), Some("2026-10-28"));

    send(&mut app, [Message::ArchiveCard(a)]);
    assert_eq!(app.data.card(a), None);
    send(&mut app, [Message::Restore(a)]);
    assert_eq!(card(&app, a).due.as_deref(), Some("2026-10-28"));
}

#[test]
fn the_date_round_trips_through_the_data_file_and_a_v1_file_has_none() {
    let (mut app, a) = app_with_open_card();
    type_due(&mut app, a, "2026-10-28");
    let dir = tempfile::tempdir().unwrap();

    storage::save(dir.path(), &app.data, &Settings::default()).unwrap();
    let (data, _) = storage::load(dir.path()).unwrap().unwrap();
    assert_eq!(data, app.data);
    assert_eq!(data.card(a).unwrap().due.as_deref(), Some("2026-10-28"));

    let v1 = r#"{ "version": 1, "next_id": 5, "boards": [ { "id": 1, "name": "Work",
        "columns": [ { "id": 2, "name": "To do", "cards": [ { "id": 3, "title": "A" } ] } ] } ] }"#;
    std::fs::write(dir.path().join("kanvibe.json"), v1).unwrap();
    let (data, _) = storage::load(dir.path()).unwrap().unwrap();
    assert_eq!(data.card(3).unwrap().due, None);
}

#[test]
fn a_card_without_a_date_writes_no_due_key() {
    let (app, a) = app_with_open_card();
    let json = serde_json::to_value(card(&app, a)).unwrap();
    assert!(json.get("due").is_none(), "{json}");
}

#[test]
fn export_and_import_keep_the_date() {
    let (mut app, a) = app_with_open_card();
    type_due(&mut app, a, "2026-10-28");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.kanvibe.json");
    transfer::export(&path, &app.data.boards[0]).unwrap();

    let bytes = std::fs::read(path).unwrap();
    send(&mut app, [Message::Imported(Some(Ok(bytes)))]);

    let copy = &app.data.boards[1];
    assert_eq!(copy.columns[0].cards[0].due.as_deref(), Some("2026-10-28"));
}
