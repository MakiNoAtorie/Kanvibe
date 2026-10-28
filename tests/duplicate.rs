//! Duplicate a Card (v2 item 4): the copy, its selection and its one undo step.
use kanvibe::app::{App, Message, Selection};
use kanvibe::domain::Id;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// To do: A, B, C. B has a description, a Label and two Items (one ticked).
/// Nothing selected, no undo history.
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
    let (board, todo) = (app.data.boards[0].id, app.data.boards[0].columns[0].id);
    for title in ["A", "B", "C"] {
        app.data.add_card(todo, title).unwrap();
    }
    let b = app.data.boards[0].columns[0].cards[1].id;
    let label = app.data.add_label(board, "urgent", "red").unwrap();
    app.data.toggle_label(b, label).unwrap();
    let first = app.data.add_item(b, None).unwrap();
    app.data.add_item(b, None).unwrap();
    app.data.toggle_item(first).unwrap();
    let card = &mut app.data.boards[0].columns[0].cards[1];
    card.description = "Some *notes*".into();
    card.checklist[0].text = "one".into();
    card.checklist[1].text = "two".into();
    card.created = "2020-01-01T00:00:00Z".into();
    card.updated = "2020-01-01T00:00:00Z".into();
    app.selection = None;
    app.history = Default::default();
    app
}

fn select(app: &mut App, index: Option<usize>) {
    let column = &app.data.boards[0].columns[0];
    app.selection = Some(Selection {
        column: column.id,
        card: index.map(|i| column.cards[i].id),
    });
}

fn titles(app: &App) -> Vec<&str> {
    let cards = &app.data.boards[0].columns[0].cards;
    cards.iter().map(|c| c.title.as_str()).collect()
}

fn ids(app: &App) -> Vec<Id> {
    let cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards
        .flat_map(|c| std::iter::once(c.id).chain(c.checklist.iter().map(|i| i.id)))
        .collect()
}

#[test]
fn duplicate_inserts_an_unticked_copy_after_the_original_and_selects_it() {
    let mut app = app_with_cards();
    select(&mut app, Some(1)); // B
    let before = ids(&app);

    send(&mut app, [Message::DuplicateCard]);

    assert_eq!(titles(&app), ["A", "B", "B", "C"]);
    let cards = &app.data.boards[0].columns[0].cards;
    let (original, copy) = (&cards[1], &cards[2]);
    assert_eq!(copy.description, "Some *notes*");
    assert_eq!(copy.labels, original.labels);
    assert_eq!(copy.labels.len(), 1);
    let texts: Vec<_> = copy.checklist.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["one", "two"]);
    assert!(copy.checklist.iter().all(|i| !i.done));
    assert!(original.checklist[0].done, "the original keeps its ticks");
    assert!(!before.contains(&copy.id));
    assert!(copy.checklist.iter().all(|i| !before.contains(&i.id)));
    assert_ne!(copy.created, "2020-01-01T00:00:00Z");
    assert_ne!(copy.updated, "2020-01-01T00:00:00Z");
    assert_eq!(app.selection.and_then(|s| s.card), Some(copy.id));
}

#[test]
fn the_copy_keeps_the_due_date_and_files() {
    let mut app = app_with_cards();
    let b = &mut app.data.boards[0].columns[0].cards[1];
    b.due = Some("2026-10-28".into());
    b.files = vec!["C:\\notes.txt".into(), "C:\\plan.pdf".into()];
    select(&mut app, Some(1));

    send(&mut app, [Message::DuplicateCard]);

    let copy = &app.data.boards[0].columns[0].cards[2];
    assert_eq!(copy.due.as_deref(), Some("2026-10-28"));
    assert_eq!(copy.files, ["C:\\notes.txt", "C:\\plan.pdf"]);
}

#[test]
fn duplicating_the_open_card_keeps_the_copy_open() {
    let mut app = app_with_cards();
    select(&mut app, Some(1));
    app.open = true;

    send(&mut app, [Message::DuplicateCard]);

    assert_eq!(
        app.open_card(),
        Some(app.data.boards[0].columns[0].cards[2].id)
    );
}

#[test]
fn one_undo_removes_the_copy() {
    let mut app = app_with_cards();
    select(&mut app, Some(1));
    let before = app.data.boards.clone();

    send(&mut app, [Message::DuplicateCard, Message::Undo]);

    assert_eq!(app.data.boards, before);
    let card = app.selection.and_then(|s| s.card);
    assert!(card.is_none_or(|c| app.data.card(c).is_some()));
}

#[test]
fn duplicate_with_only_a_column_selected_does_nothing() {
    let mut app = app_with_cards();
    select(&mut app, None);
    let before = app.data.boards.clone();

    send(&mut app, [Message::DuplicateCard, Message::Undo]);

    assert_eq!(app.data.boards, before, "no copy and no undo step");
}
