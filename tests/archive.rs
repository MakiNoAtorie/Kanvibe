//! Seam 1 for Archiving Cards, Remove Column and the Archive view (#11).
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{App, Message, Selection};
use kanvibe::archive;
use kanvibe::domain::Id;
use kanvibe::keys::{self, Context};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// To do: A, B, C; Doing: empty; Done: D.
fn app_with_cards() -> App {
    let mut app = App::default();
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    let (todo, done) = (column_id(&app, 0), column_id(&app, 2));
    for (column, title) in [(todo, "A"), (todo, "B"), (todo, "C"), (done, "D")] {
        send(
            &mut app,
            [
                Message::OpenCapture(column),
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
                Message::Escape,
            ],
        );
    }
    app.selection = None;
    app
}

fn column_id(app: &App, index: usize) -> Id {
    app.data.boards[0].columns[index].id
}

fn card(app: &App, title: &str) -> Id {
    let board = &app.data.boards[0];
    let active = board.columns.iter().flat_map(|c| &c.cards);
    let archived = board.archived_cards.iter().map(|a| &a.card);
    active
        .chain(archived)
        .find(|c| c.title == title)
        .unwrap()
        .id
}

fn titles(app: &App, column: usize) -> Vec<String> {
    let cards = &app.data.boards[0].columns[column].cards;
    cards.iter().map(|c| c.title.clone()).collect()
}

fn archived(app: &App) -> Vec<String> {
    let board = &app.data.boards[0];
    let rows = archive::rows(board);
    rows.into_iter().map(|r| r.title).collect()
}

/// The Card whose permanent-delete question is open.
fn delete_ask(app: &App) -> Option<Id> {
    match app.confirm.as_ref()?.action {
        Message::DeleteCard(card) => Some(card),
        _ => None,
    }
}

fn click(app: &mut App, title: &str) {
    let id = card(app, title);
    let column = app.data.boards[0]
        .columns
        .iter()
        .find(|c| c.cards.iter().any(|x| x.id == id))
        .unwrap()
        .id;
    send(
        app,
        [Message::Select(Selection {
            column,
            card: Some(id),
        })],
    );
}

fn press(key: Named) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        modified_key: Key::Named(key),
        key: Key::Named(key),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

#[test]
fn del_archives_the_selected_card() {
    let message = keys::map(&press(Named::Delete), &Context::default());
    assert_eq!(message, Some(Message::ArchiveSelected));

    let mut app = app_with_cards();
    click(&mut app, "B");
    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(titles(&app, 0), ["A", "C"]);
    assert_eq!(archived(&app), ["B"]);
    let a = &app.data.boards[0].archived_cards[0];
    assert_eq!(a.column_id, column_id(&app, 0));
    assert!(a.archived_at.parse::<jiff::Timestamp>().is_ok());
}

#[test]
fn del_does_nothing_without_a_selected_card_and_is_ignored_in_a_text_field() {
    let mut app = app_with_cards();
    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);

    let typing = Context {
        focused: Some(keys::Input::Edit),
        ..Context::default()
    };
    assert_eq!(keys::map(&press(Named::Delete), &typing), None);
}

#[test]
fn the_selection_goes_to_the_card_below_else_above_else_the_column() {
    let mut app = app_with_cards();
    let todo = column_id(&app, 0);
    let on = |app: &App, title: &str| {
        Some(Selection {
            column: todo,
            card: Some(card(app, title)),
        })
    };

    click(&mut app, "A");
    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(app.selection, on(&app, "B"), "below");

    click(&mut app, "C");
    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(app.selection, on(&app, "B"), "above, when nothing is below");

    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(
        app.selection,
        Some(Selection {
            column: todo,
            card: None
        }),
        "the Column, when it was the only Card"
    );
}

#[test]
fn archiving_from_the_panel_closes_it_and_is_one_undo_step() {
    let mut app = app_with_cards();
    click(&mut app, "B");
    let b = card(&app, "B");
    assert_eq!(app.panel, Some(b));
    send(&mut app, [Message::ArchiveCard(b)]);
    assert_eq!(app.panel, None);
    assert_eq!(titles(&app, 0), ["A", "C"]);

    send(&mut app, [Message::Undo]);
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    assert_eq!(archived(&app), Vec::<String>::new());
}

#[test]
fn archiving_updates_the_cards_timestamp() {
    let mut app = app_with_cards();
    let b = card(&app, "B");
    app.data.boards[0].columns[0].cards[1].updated = "2020-01-01T00:00:00Z".into();
    send(&mut app, [Message::ArchiveCard(b)]);
    let a = &app.data.boards[0].archived_cards[0];
    assert_ne!(a.card.updated, "2020-01-01T00:00:00Z");
}

#[test]
fn remove_column_archives_its_cards_as_one_undo_step() {
    let mut app = app_with_cards();
    let todo = column_id(&app, 0);
    assert_eq!(
        archive::remove_column_text(&app.data.boards[0].columns[0]),
        "Remove Column (archives 3 Cards)"
    );
    assert_eq!(
        archive::remove_column_text(&app.data.boards[0].columns[1]),
        "Remove Column (archives 0 Cards)"
    );

    send(&mut app, [Message::RemoveColumn(todo)]);
    assert_eq!(app.data.boards[0].columns.len(), 2);
    assert_eq!(archived(&app).len(), 3);
    let all_from_todo = app.data.boards[0]
        .archived_cards
        .iter()
        .all(|a| a.column_id == todo);
    assert!(all_from_todo);

    send(&mut app, [Message::Undo]);
    assert_eq!(app.data.boards[0].columns.len(), 3);
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    assert!(app.data.boards[0].archived_cards.is_empty());
}

#[test]
fn the_menu_closes_and_the_selection_leaves_a_removed_column() {
    let mut app = app_with_cards();
    let todo = column_id(&app, 0);
    click(&mut app, "A");
    send(
        &mut app,
        [Message::ToggleColumnMenu(todo), Message::RemoveColumn(todo)],
    );
    assert_eq!(app.column_menu, None);
    assert_eq!(app.selection, None);
    assert_eq!(app.panel, None);
}

#[test]
fn the_archive_view_lists_newest_first_with_the_original_column() {
    let mut app = app_with_cards();
    let todo = column_id(&app, 0);
    let (a, d) = (card(&app, "A"), card(&app, "D"));
    send(&mut app, [Message::ArchiveCard(a), Message::ArchiveCard(d)]);
    let rows = archive::rows(&app.data.boards[0]);
    let rows: Vec<_> = rows.iter().map(|r| (&*r.title, &*r.column)).collect();
    assert_eq!(rows, [("D", "Done"), ("A", "To do")]);

    send(&mut app, [Message::RemoveColumn(todo)]);
    let rows = archive::rows(&app.data.boards[0]);
    let rows: Vec<_> = rows.iter().map(|r| (&*r.title, &*r.column)).collect();
    assert_eq!(
        rows,
        [
            ("C", "removed Column"),
            ("B", "removed Column"),
            ("D", "Done"),
            ("A", "removed Column"),
        ]
    );
}

#[test]
fn the_archive_view_opens_and_esc_or_back_leaves_it() {
    let mut app = app_with_cards();
    let board = app.current_board;
    click(&mut app, "A");
    send(&mut app, [Message::OpenArchive]);
    assert_eq!(app.archive, board);
    assert_eq!((app.panel, app.selection), (None, None));

    send(&mut app, [Message::Escape]);
    assert_eq!(app.archive, None);

    send(&mut app, [Message::OpenArchive, Message::CloseArchive]);
    assert_eq!(app.archive, None);
}

#[test]
fn esc_closes_the_archive_view_before_the_panel_and_selection() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    app.archive = app.current_board; // reachable by undo, a click on nothing, etc.
    send(&mut app, [Message::Escape]);
    assert_eq!(app.archive, None);
    assert!(app.panel.is_some() && app.selection.is_some());
    send(&mut app, [Message::Escape]);
    assert_eq!(app.panel, None);
}

#[test]
fn keys_that_act_on_the_board_do_nothing_in_the_archive_view() {
    let mut app = app_with_cards();
    send(&mut app, [Message::OpenArchive]);
    for key in [Named::Delete, Named::ArrowDown, Named::Enter] {
        send(&mut app, [Message::KeyWithFocus(press(key), None)]);
    }
    assert_eq!((app.selection, app.panel), (None, None));
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
}

#[test]
fn restore_puts_the_card_at_the_bottom_of_its_original_column() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a), Message::Restore(a)]);
    assert_eq!(titles(&app, 0), ["B", "C", "A"]);
    assert!(app.data.boards[0].archived_cards.is_empty());

    send(&mut app, [Message::Undo]);
    assert_eq!(archived(&app), ["A"]);
}

#[test]
fn restore_into_a_removed_column_goes_to_the_first_column() {
    let mut app = app_with_cards();
    let done = column_id(&app, 2);
    let d = card(&app, "D");
    send(&mut app, [Message::RemoveColumn(done), Message::Restore(d)]);
    assert_eq!(titles(&app, 0), ["A", "B", "C", "D"]);
}

#[test]
fn restore_with_no_columns_creates_to_do_in_the_same_undo_step() {
    let mut app = app_with_cards();
    let ids: Vec<_> = (0..3).map(|i| column_id(&app, i)).collect();
    for id in ids {
        send(&mut app, [Message::RemoveColumn(id)]);
    }
    assert!(app.data.boards[0].columns.is_empty());
    let a = card(&app, "A");
    send(&mut app, [Message::Restore(a)]);
    let columns = &app.data.boards[0].columns;
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name, "To do");
    assert_eq!(titles(&app, 0), ["A"]);

    send(&mut app, [Message::Undo]);
    assert!(app.data.boards[0].columns.is_empty());
    assert_eq!(archived(&app).len(), 4);
}

#[test]
fn restoring_over_a_column_limit_is_allowed_and_flags_the_column() {
    let mut app = app_with_cards();
    let todo = column_id(&app, 0);
    let a = card(&app, "A");
    send(
        &mut app,
        [
            Message::ArchiveCard(a),
            Message::OpenLimit(todo),
            Message::LimitChanged("2".into()),
            Message::SubmitLimit,
            Message::Restore(a),
        ],
    );
    let column = &app.data.boards[0].columns[0];
    assert_eq!(column.cards.len(), 3);
    assert!(column.over_limit());
}

#[test]
fn restoring_updates_the_cards_timestamp() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a)]);
    app.data.boards[0].archived_cards[0].card.updated = "2020-01-01T00:00:00Z".into();
    send(&mut app, [Message::Restore(a)]);
    assert_ne!(app.data.card(a).unwrap().updated, "2020-01-01T00:00:00Z");
}

#[test]
fn permanent_delete_asks_first_and_esc_cancels() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a), Message::AskDelete(a)]);
    assert_eq!(delete_ask(&app), Some(a));
    assert_eq!(archived(&app), ["A"], "nothing deleted yet");

    send(&mut app, [Message::Escape]);
    assert_eq!(delete_ask(&app), None);
    assert_eq!(archived(&app), ["A"]);
}

#[test]
fn the_confirmation_names_the_card() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a), Message::AskDelete(a)]);
    let board = &app.data.boards[0];
    assert_eq!(
        archive::delete_question(board, a).as_deref(),
        Some("Permanently delete \"A\"?")
    );
}

#[test]
fn confirmed_permanent_delete_removes_the_card_and_is_undoable() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(
        &mut app,
        [
            Message::ArchiveCard(a),
            Message::AskDelete(a),
            Message::Confirm,
        ],
    );
    assert_eq!(delete_ask(&app), None);
    assert!(app.data.boards[0].archived_cards.is_empty());
    assert_eq!(app.data.card(a), None);

    send(&mut app, [Message::Undo]);
    assert_eq!(archived(&app), ["A"]);
}

#[test]
fn undo_dismisses_a_confirmation_whose_card_is_gone() {
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a), Message::AskDelete(a)]);
    send(&mut app, [Message::KeyWithFocus(ctrl_z(), None)]);
    assert_eq!(delete_ask(&app), Some(a), "keys other than Esc are ignored");

    send(&mut app, [Message::Undo]); // the card is back on the Board
    assert_eq!(delete_ask(&app), None);
}

fn ctrl_z() -> keyboard::Event {
    keyboard::Event::KeyPressed {
        modified_key: Key::Character("z".into()),
        key: Key::Character("z".into()),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyZ),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::CTRL,
        text: None,
        repeat: false,
    }
}

#[test]
fn archived_cards_survive_a_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_cards();
    let a = card(&app, "A");
    send(&mut app, [Message::ArchiveCard(a)]);
    let settings = Default::default();
    kanvibe::storage::save(dir.path(), &app.data, &settings).unwrap();
    let (data, _) = kanvibe::storage::load(dir.path()).unwrap().unwrap();
    assert_eq!(data, app.data);
    assert_eq!(data.boards[0].archived_cards[0].card.title, "A");
}
