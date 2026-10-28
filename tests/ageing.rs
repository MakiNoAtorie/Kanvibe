//! Card ageing: a Card untouched for over 14 days is stale; moving it touches it.
use iced::Point;
use jiff::Timestamp;
use kanvibe::app::{App, Message};
use kanvibe::domain::{Card, Id};
use kanvibe::drag::{Item, Target};

const OLD: &str = "2020-01-01T00:00:00Z";

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn card(updated: &str) -> Card {
    Card {
        id: 1,
        title: "A".into(),
        created: String::new(),
        updated: updated.into(),
        checklist: Vec::new(),
        labels: Vec::new(),
        description: String::new(),
        files: Vec::new(),
        due: None,
    }
}

#[test]
fn a_card_is_stale_only_when_its_last_change_is_more_than_14_days_ago() {
    let now: Timestamp = "2026-03-20T12:00:00Z".parse().unwrap();
    assert!(!card("2026-03-07T12:00:00Z").is_stale(now)); // 13 days
    assert!(!card("2026-03-06T12:00:00Z").is_stale(now)); // exactly 14 days
    assert!(card("2026-03-06T11:59:59Z").is_stale(now)); // 14 days and a second
    assert!(card("2026-03-05T12:00:00Z").is_stale(now)); // 15 days
    assert!(!card("").is_stale(now));
    assert!(!card("not a date").is_stale(now));
}

/// One Board: To do: A, B; Doing empty. Timestamps `OLD`, history reset.
fn app_with_old_cards() -> App {
    let mut app = App::default();
    let work = app.data.create_board("Work").unwrap();
    let first = app.data.boards[0].columns[0].id;
    app.data.add_card(first, "A");
    app.data.add_card(first, "B");
    app.current_board = Some(work);
    let cards = app.data.boards[0].columns.iter_mut();
    for card in cards.flat_map(|c| &mut c.cards) {
        card.updated = OLD.into();
    }
    app.history = Default::default();
    app
}

fn id(app: &App, title: &str) -> Id {
    let mut cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.find(|c| c.title == title).unwrap().id
}

fn updated(app: &App, title: &str) -> String {
    app.data.card(id(app, title)).unwrap().updated.clone()
}

fn drop_on(app: &mut App, title: &str, target: Target) {
    let card = id(app, title);
    send(
        app,
        [
            Message::DragPress(Item::Card(card)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(100.0, 120.0)),
            Message::DragHover(target),
            Message::DragEnd,
        ],
    );
}

#[test]
fn dropping_a_card_where_it_was_neither_touches_it_nor_records_a_step() {
    let mut app = app_with_old_cards();
    let column = app.data.boards[0].columns[0].id;
    // A's own slot and the one right after it.
    for index in [0, 1] {
        drop_on(&mut app, "A", Target::Card { column, index });
        assert_eq!(updated(&app, "A"), OLD);
    }
    assert_eq!(app.history.undo(&mut app.data.boards), None);
}

#[test]
fn moving_a_card_within_its_column_or_to_another_touches_only_that_card() {
    let mut app = app_with_old_cards();
    let todo = app.data.boards[0].columns[0].id;
    let doing = app.data.boards[0].columns[1].id;
    drop_on(
        &mut app,
        "A",
        Target::Card {
            column: todo,
            index: 2,
        },
    );
    assert_ne!(updated(&app, "A"), OLD);
    assert_eq!(updated(&app, "B"), OLD);

    drop_on(&mut app, "B", Target::Column(doing));
    assert_ne!(updated(&app, "B"), OLD);
}
