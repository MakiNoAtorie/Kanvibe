//! Export and import of a Board (#24): the export file naming (storage seam) and import
//! through `update` (seam 1).
use iced::Point;
use kanvibe::app::{App, Field, Message};
use kanvibe::domain::{Board, Card, Id};
use kanvibe::drag::Item;
use kanvibe::transfer::{self, file_name};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One current Board "Work": Labels Bug and Idea; To do holds A (Idea, Bug, one done Item,
/// a description, a limit of 3); Doing holds B; Done is empty. C was archived from Doing, and
/// D from a Column "Old" that was then removed.
fn app_with_work() -> App {
    let mut app = App::default();
    let data = &mut app.data;
    let work = data.create_board("Work").unwrap();
    let [todo, doing] = [0, 1].map(|i| data.boards[0].columns[i].id);
    let bug = data.add_label(work, "Bug", "red").unwrap();
    let idea = data.add_label(work, "Idea", "blue").unwrap();
    let a = data.add_card(todo, "A").unwrap();
    data.toggle_label(a, idea);
    data.toggle_label(a, bug);
    let item = data.add_item(a, None).unwrap();
    data.rename(item, "Step");
    data.toggle_item(item);
    data.set_description(a, "**Notes**");
    data.set_limit(todo, "3");
    data.add_card(doing, "B");
    let c = data.add_card(doing, "C").unwrap();
    data.archive_card(c);
    let old = data.add_column(work).unwrap();
    data.rename(old, "Old");
    data.add_card(old, "D");
    data.remove_column(old);
    for card in data.boards[0].columns.iter_mut().flat_map(|c| &mut c.cards) {
        card.created = OLD.into();
    }
    app.current_board = Some(work);
    app.history = Default::default();
    app
}

const OLD: &str = "2020-01-01T00:00:00Z";

/// Everything about a Board except its name and IDs, with references shown by name.
fn shape(b: &Board) -> Vec<String> {
    let label = |id: &Id| {
        b.labels
            .iter()
            .find(|l| l.id == *id)
            .map(|l| l.name.clone())
    };
    let card = |c: &Card| {
        let labels: Vec<_> = c.labels.iter().map(label).collect();
        let items: Vec<_> = c.checklist.iter().map(|i| (&i.text, i.done)).collect();
        let (title, created, updated) = (&c.title, &c.created, &c.updated);
        format!(
            "{title} {created} {updated} {labels:?} {items:?} {:?}",
            c.description
        )
    };
    let labels = b
        .labels
        .iter()
        .map(|l| format!("label {} {}", l.name, l.color));
    let mut out: Vec<String> = labels.collect();
    for c in &b.columns {
        out.push(format!("column {} {:?}", c.name, c.limit));
        out.extend(c.cards.iter().map(card));
    }
    out.extend(b.archived_cards.iter().map(|a| {
        let from = b
            .columns
            .iter()
            .find(|c| c.id == a.column_id)
            .map(|c| &c.name);
        format!(
            "archived from {from:?} at {}: {}",
            a.archived_at,
            card(&a.card)
        )
    }));
    out
}

/// Every ID the Board holds or refers to.
fn ids(b: &Board) -> Vec<Id> {
    let card = |c: &Card| {
        let items = c.checklist.iter().map(|i| i.id);
        let all: Vec<Id> = [c.id]
            .into_iter()
            .chain(c.labels.clone())
            .chain(items)
            .collect();
        all
    };
    let mut out = vec![b.id];
    out.extend(b.labels.iter().map(|l| l.id));
    for c in &b.columns {
        out.push(c.id);
        out.extend(c.cards.iter().flat_map(card));
    }
    for a in &b.archived_cards {
        out.push(a.column_id);
        out.extend(card(&a.card));
    }
    out
}

fn export(board: &Board) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.kanvibe.json");
    transfer::export(&path, board).unwrap();
    std::fs::read(path).unwrap()
}

fn import(app: &mut App, bytes: &[u8]) {
    send(app, [Message::Imported(Some(Ok(bytes.to_vec())))]);
}

#[test]
fn export_writes_the_board_with_its_format_and_version_and_no_settings() {
    let app = app_with_work();

    let bytes = export(&app.data.boards[0]);

    let file: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(file["format"], "kanvibe-board");
    assert_eq!(file["version"], 1);
    assert_eq!(file["board"]["name"], "Work");
    assert_eq!(file["board"]["labels"][0]["name"], "Bug");
    assert_eq!(file["board"]["archived_cards"][1]["card"]["title"], "D");
    let mut keys: Vec<_> = file.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["board", "format", "version"]);
}

#[test]
fn export_round_trips_through_import_into_an_equivalent_board_with_fresh_ids() {
    let mut app = app_with_work();
    let before = app.data.next_id;
    let bytes = export(&app.data.boards[0]);

    import(&mut app, &bytes);

    let [work, copy] = &app.data.boards[..] else {
        panic!("expected two Boards");
    };
    assert_eq!(shape(copy), shape(work));
    let a = format!("A {OLD} ");
    let a = shape(copy).into_iter().find(|l| l.starts_with(&a)).unwrap();
    assert!(a.contains(r#"[Some("Idea"), Some("Bug")] [("Step", true)] "**Notes**""#));
    let c = &shape(copy)[shape(copy).len() - 2];
    assert!(c.starts_with(r#"archived from Some("Doing")"#), "{c}");
    assert!(ids(copy).iter().all(|id| *id > before), "{:?}", ids(copy));
    assert!(!copy.archived);
}

#[test]
fn export_file_name_replaces_illegal_characters_and_trims_trailing_dots_and_spaces() {
    assert_eq!(file_name("Work"), "Work.kanvibe.json");
    assert_eq!(
        file_name("a<b>c:d\"e/f\\g|h?i*j\tk\u{7f}"),
        "a_b_c_d_e_f_g_h_i_j_k_.kanvibe.json"
    );
    assert_eq!(file_name("Plans. . "), "Plans.kanvibe.json");
}

#[test]
fn export_file_name_prefixes_reserved_device_names() {
    assert_eq!(file_name("CON"), "_CON.kanvibe.json");
    assert_eq!(file_name("nul"), "_nul.kanvibe.json");
    assert_eq!(file_name("COM1"), "_COM1.kanvibe.json");
    assert_eq!(file_name("lpt9"), "_lpt9.kanvibe.json");
    assert_eq!(file_name("COM10"), "COM10.kanvibe.json");
    assert_eq!(file_name("Console"), "Console.kanvibe.json");
    assert_eq!(file_name("Aux.notes"), "_Aux.notes.kanvibe.json"); // Windows ignores the extension
}

#[test]
fn export_file_name_falls_back_to_board_when_empty() {
    assert_eq!(file_name(""), "Board.kanvibe.json");
    assert_eq!(file_name(" ..."), "Board.kanvibe.json");
}

#[test]
fn export_file_name_is_cut_to_100_characters() {
    let long = "é".repeat(150);
    assert_eq!(
        file_name(&long),
        format!("{}.kanvibe.json", "é".repeat(100))
    );
    // Trailing spaces left by the cut are trimmed too.
    let spaced = format!("{} x", "a".repeat(99));
    assert_eq!(
        file_name(&spaced),
        format!("{}.kanvibe.json", "a".repeat(99))
    );
}

/// Imports `bytes` into a copy of `app_with_work` and returns the message shown, checking
/// that nothing was added.
fn refusal(bytes: &[u8]) -> Option<String> {
    let mut app = app_with_work();
    import(&mut app, bytes);
    assert_eq!(app.data.boards.len(), 1);
    app.notice
}

#[test]
fn importing_a_kanvibe_data_file_is_refused() {
    let app = app_with_work();
    let dir = tempfile::tempdir().unwrap();
    kanvibe::storage::save(dir.path(), &app.data, &Default::default()).unwrap();
    let bytes = std::fs::read(kanvibe::storage::file_path(dir.path())).unwrap();

    assert_eq!(
        refusal(&bytes).as_deref(),
        Some("This is a Kanvibe data file, not a Board export.")
    );
}

#[test]
fn importing_a_file_that_is_not_a_board_export_is_refused() {
    let not_export = Some("This file is not a Kanvibe Board export.");
    assert_eq!(refusal(br#"{"name": "Work"}"#).as_deref(), not_export);
    assert_eq!(
        refusal(br#"{"format": "trello", "version": 1, "board": {}}"#).as_deref(),
        not_export
    );
    assert_eq!(refusal(b"[1, 2]").as_deref(), not_export);
    assert_eq!(refusal(b"not json").as_deref(), not_export);
}

#[test]
fn importing_a_board_from_a_newer_version_is_refused() {
    let bytes = br#"{"format": "kanvibe-board", "version": 2, "board": {"id": 1, "name": "X"}}"#;

    assert_eq!(
        refusal(bytes).as_deref(),
        Some("This Board was exported by a newer version of Kanvibe. Update Kanvibe to import it.")
    );
}

#[test]
fn a_broken_board_export_is_refused_and_esc_closes_the_message() {
    let mut app = app_with_work();
    import(
        &mut app,
        br#"{"format": "kanvibe-board", "version": 1, "board": {"id": "x"}}"#,
    );
    let notice = app.notice.clone().unwrap();
    assert!(
        notice.starts_with("This Board export can't be read: "),
        "{notice}"
    );

    send(&mut app, [Message::Escape]);

    assert_eq!(app.notice, None);
}

/// The name a Board called `name` gets when imported next to Boards `existing`
/// (a leading `-` marks an archived one).
fn imported_name(existing: &[&str], name: &str) -> String {
    let mut app = App::default();
    for board in existing {
        let id = app
            .data
            .create_board(board.trim_start_matches('-'))
            .unwrap();
        if board.starts_with('-') {
            app.data.archive_board(id);
        }
    }
    let mut source = App::default();
    source.data.create_board("x");
    source.data.boards[0].name = name.into();
    import(&mut app, &export(&source.data.boards[0]));
    app.data.boards.last().unwrap().name.clone()
}

#[test]
fn an_imported_board_whose_name_is_free_keeps_it() {
    assert_eq!(imported_name(&["Home"], "Work"), "Work");
    assert_eq!(imported_name(&["Home"], "Work (2)"), "Work (2)");
}

#[test]
fn a_name_clash_gets_the_first_free_number_ignoring_case_spaces_and_archiving() {
    assert_eq!(imported_name(&["Work"], "Work"), "Work (2)");
    assert_eq!(imported_name(&["Work", "-WORK (2)"], " work "), "work (3)");
    assert_eq!(imported_name(&["Work", "Work (3)"], "Work"), "Work (2)");
}

#[test]
fn a_clashing_name_loses_its_own_number_before_numbering() {
    assert_eq!(imported_name(&["Work", "Work (2)"], "Work (2)"), "Work (3)");
    assert_eq!(imported_name(&["Work (7)"], "Work (7)"), "Work (2)");
}

#[test]
fn a_failed_export_or_unreadable_import_file_shows_why_and_a_cancelled_dialog_nothing() {
    let mut app = app_with_work();
    send(&mut app, [Message::Exported(None), Message::Imported(None)]);
    assert_eq!(app.notice, None);

    send(
        &mut app,
        [Message::Exported(Some(Err(
            "Couldn't export: denied".into()
        )))],
    );
    assert_eq!(app.notice.as_deref(), Some("Couldn't export: denied"));

    send(
        &mut app,
        [
            Message::Escape,
            Message::Imported(Some(Err("Can't open x".into()))),
        ],
    );
    assert_eq!(app.notice.as_deref(), Some("Can't open x"));
    assert_eq!(app.data.boards.len(), 1);
}

#[test]
fn import_applies_the_load_repairs() {
    let mut app = App::default();
    // Unknown colour, limit 0, a blank Column name, a dangling Label and a repeated Card ID.
    let bytes = br#"{"format": "kanvibe-board", "version": 1, "board": {"id": 1, "name": "X",
        "labels": [{"id": 2, "name": "Bug", "color": "mauve"}],
        "columns": [{"id": 3, "name": " ", "limit": 0, "cards": [
            {"id": 4, "title": "A", "labels": [2, 99]},
            {"id": 4, "title": "B"}]}]}}"#;

    import(&mut app, bytes);

    let board = &app.data.boards[0];
    assert_eq!(board.labels[0].color, "gray");
    let column = &board.columns[0];
    assert_eq!((column.name.as_str(), column.limit), ("New column", None));
    assert_eq!(column.cards[0].labels, [board.labels[0].id]);
    assert_ne!(column.cards[0].id, column.cards[1].id);
}

#[test]
fn import_opens_the_new_board_cancels_a_drag_and_is_one_undo_step() {
    let mut app = app_with_work();
    let work = app.data.boards[0].id;
    let bytes = export(&app.data.boards[0]);
    let a = app.data.boards[0].columns[0].cards[0].id;
    send(
        &mut app,
        [
            Message::StartEdit(Field::BoardName(work)),
            Message::EditChanged("Job".into()),
            Message::DragPress(Item::Card(a)),
            Message::DragMoved(Point::new(0.0, 0.0)),
            Message::DragMoved(Point::new(50.0, 50.0)),
        ],
    );
    assert!(app.drag.is_some());

    import(&mut app, &bytes);

    let names = |app: &App| {
        app.data
            .boards
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&app), ["Job", "Work"]);
    assert_eq!(app.current_board, Some(app.data.boards[1].id));
    assert!(app.drag.is_none());

    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["Job"]);
    assert_eq!(app.current_board, Some(work));

    send(&mut app, [Message::Redo]);
    assert_eq!(names(&app), ["Job", "Work"]);
}

#[test]
fn export_and_save_as_image_first_add_the_card_in_an_open_capture_box() {
    let mut app = app_with_work();
    let (work, todo) = (app.data.boards[0].id, app.data.boards[0].columns[0].id);
    for (title, message) in [
        ("Typed before export", Message::ExportBoard(work)),
        ("Typed before image", Message::SaveImage(work)),
    ] {
        send(
            &mut app,
            [
                Message::OpenCapture(todo),
                Message::CaptureChanged(title.into()),
                message,
            ],
        );
        assert!(app.capture.is_none());
        let cards = &app.data.boards[0].columns[0].cards;
        assert!(cards.iter().any(|c| c.title == title), "{title}");
    }
}
