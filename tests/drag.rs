//! Card drag and drop, through the drag Messages sent to `update`.
use iced::{Point, keyboard};
use kanvibe::app::{App, Field, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::drag::{Item, Target};
use kanvibe::keys::{self, Context};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One Board: To do: A, B, C; Doing empty; Done: D.
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
    for (column, titles) in [(0, &["A", "B", "C"][..]), (2, &["D"])] {
        let column = column_id(&app, column);
        send(&mut app, [Message::OpenCapture(column)]);
        for title in titles {
            send(
                &mut app,
                [
                    Message::CaptureChanged((*title).into()),
                    Message::SubmitCapture,
                ],
            );
        }
    }
    send(&mut app, [Message::Escape]);
    app.selection = None;
    app
}

fn column_id(app: &App, index: usize) -> Id {
    app.data.boards[0].columns[index].id
}

fn card_id(app: &App, title: &str) -> Id {
    let mut cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.find(|c| c.title == title).expect("card").id
}

fn titles(app: &App, column: usize) -> Vec<&str> {
    let cards = &app.data.boards[0].columns[column].cards;
    cards.iter().map(|c| c.title.as_str()).collect()
}

/// The Card slot before Card `index` of Column `column`; `None` is the Column's body.
fn target(app: &App, column: usize, slot: Option<usize>) -> Target {
    let column = column_id(app, column);
    match slot {
        Some(index) => Target::Card { column, index },
        None => Target::Column(column),
    }
}

/// Presses on Card `title` and moves the cursor `distance` px down.
fn press_and_move(app: &mut App, title: &str, distance: f32) {
    let card = card_id(app, title);
    send(
        app,
        [
            Message::DragPress(Item::Card(card)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(100.0, 100.0 + distance)),
        ],
    );
}

/// Drags Card `title` over a [`target`] and releases.
fn drop_on(app: &mut App, title: &str, column: usize, slot: Option<usize>) {
    press_and_move(app, title, 20.0);
    let target = target(app, column, slot);
    send(app, [Message::DragHover(target), Message::DragEnd]);
}

#[test]
fn dropping_on_a_columns_body_moves_the_card_to_its_end_as_one_undo_step() {
    let mut app = app_with_cards();

    drop_on(&mut app, "A", 1, None);

    assert_eq!(titles(&app, 0), ["B", "C"]);
    assert_eq!(titles(&app, 1), ["A"]);
    assert_eq!(app.drag, None);
    send(&mut app, [Message::Undo]);
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    assert_eq!(titles(&app, 1), Vec::<&str>::new());
}

#[test]
fn dropping_on_a_card_slot_puts_the_card_before_that_card_and_in_place_records_nothing() {
    let mut app = app_with_cards();

    drop_on(&mut app, "A", 0, Some(2));
    assert_eq!(titles(&app, 0), ["B", "A", "C"]);

    // A's own slot and the one right after it both leave A where it is.
    for gap in [1, 2] {
        drop_on(&mut app, "A", 0, Some(gap));
        assert_eq!(titles(&app, 0), ["B", "A", "C"]);
    }

    send(&mut app, [Message::Undo]);
    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
}

#[test]
fn dropping_on_a_card_slot_in_another_column_inserts_there() {
    let mut app = app_with_cards();

    drop_on(&mut app, "B", 2, Some(0));

    assert_eq!(titles(&app, 0), ["A", "C"]);
    assert_eq!(titles(&app, 2), ["B", "D"]);
}

#[test]
fn a_card_slot_wins_over_the_column_body_around_it_whatever_the_enter_order() {
    let mut app = app_with_cards();

    press_and_move(&mut app, "A", 20.0);
    let (body, card_slot) = (target(&app, 2, None), target(&app, 2, Some(0)));
    send(
        &mut app,
        [
            Message::DragHover(card_slot),
            Message::DragHover(body),
            Message::DragEnd,
        ],
    );

    assert_eq!(titles(&app, 2), ["A", "D"]);
}

#[test]
fn leaving_a_card_slot_for_the_column_body_drops_at_the_end() {
    let mut app = app_with_cards();

    press_and_move(&mut app, "A", 20.0);
    let (body, card_slot) = (target(&app, 2, None), target(&app, 2, Some(0)));
    send(
        &mut app,
        [
            Message::DragHover(body),
            Message::DragHover(card_slot),
            Message::DragLeave(card_slot),
            Message::DragEnd,
        ],
    );

    assert_eq!(titles(&app, 2), ["D", "A"]);
}

#[test]
fn moving_less_than_5px_is_a_click_that_selects_the_card() {
    let mut app = app_with_cards();

    press_and_move(&mut app, "A", 4.0);
    let elsewhere = target(&app, 1, None);
    send(&mut app, [Message::DragHover(elsewhere), Message::DragEnd]);

    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    let card = Some(card_id(&app, "A"));
    assert_eq!(
        app.selection,
        Some(Selection {
            column: column_id(&app, 0),
            card
        })
    );
    assert_eq!(app.open_card(), card, "a click opens the Card");
    assert_eq!(app.drag, None);
}

#[test]
fn releasing_outside_any_target_cancels_and_the_card_stays() {
    let mut app = app_with_cards();

    press_and_move(&mut app, "A", 20.0);
    let away = target(&app, 1, None);
    send(
        &mut app,
        [
            Message::DragHover(away),
            Message::DragLeave(away),
            Message::DragEnd,
        ],
    );

    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    assert_eq!(titles(&app, 1), Vec::<&str>::new());
    assert_eq!(app.drag, None);
    assert_eq!(app.selection, None);
}

fn escape() -> keyboard::Event {
    use keyboard::{Key, key::Named};
    keyboard::Event::KeyPressed {
        key: Key::Named(Named::Escape),
        modified_key: Key::Named(Named::Escape),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

#[test]
fn esc_cancels_the_drag_and_keeps_the_selection() {
    let mut app = app_with_cards();
    let selection = Some(Selection {
        column: column_id(&app, 2),
        card: Some(card_id(&app, "D")),
    });
    app.selection = selection;

    press_and_move(&mut app, "A", 20.0);
    let doing = target(&app, 1, None);
    send(
        &mut app,
        [
            Message::DragHover(doing),
            Message::KeyWithFocus(escape(), None),
            Message::DragEnd,
        ],
    );

    assert_eq!(titles(&app, 0), ["A", "B", "C"]);
    assert_eq!(app.drag, None);
    assert_eq!(app.selection, selection);
}

#[test]
fn a_captured_esc_cancels_the_drag_before_any_other_layer() {
    let mut app = app_with_cards();
    app.selection = Some(Selection {
        column: column_id(&app, 1),
        card: None,
    });

    press_and_move(&mut app, "A", 20.0);
    send(&mut app, [Message::Escape]);

    assert_eq!(app.drag, None);
    assert!(app.selection.is_some());
}

#[test]
fn autoscroll_speeds_up_within_40px_of_either_edge_only() {
    use kanvibe::drag::edge_speed;
    // An area from 100 to 500.
    assert_eq!(edge_speed(300.0, 100.0, 400.0), 0.0);
    assert_eq!(edge_speed(141.0, 100.0, 400.0), 0.0);
    assert!(edge_speed(130.0, 100.0, 400.0) < 0.0);
    assert!(edge_speed(470.0, 100.0, 400.0) > 0.0);
    assert!(edge_speed(490.0, 100.0, 400.0) > edge_speed(470.0, 100.0, 400.0));
    // Outside the area (e.g. over the sidebar) nothing scrolls.
    assert_eq!(edge_speed(50.0, 100.0, 400.0), 0.0);
    assert_eq!(edge_speed(550.0, 100.0, 400.0), 0.0);
}

#[test]
fn while_dragging_esc_maps_to_cancel_and_other_keys_do_nothing() {
    let dragging = Context {
        dragging: true,
        ..Context::default()
    };
    assert_eq!(keys::map(&escape(), &dragging), Some(Message::DragCancel));
    let mut arrow = escape();
    if let keyboard::Event::KeyPressed { key, .. } = &mut arrow {
        *key = keyboard::Key::Named(keyboard::key::Named::ArrowDown);
    }
    assert_eq!(keys::map(&arrow, &dragging), None);
}

#[test]
fn pressing_on_a_card_commits_a_pending_rename() {
    let mut app = app_with_cards();
    let to_do = column_id(&app, 0);
    send(
        &mut app,
        [
            Message::StartEdit(Field::ColumnName(to_do)),
            Message::EditChanged("Backlog".into()),
        ],
    );

    press_and_move(&mut app, "A", 20.0);

    assert_eq!(app.editing, None);
    assert_eq!(app.data.boards[0].columns[0].name, "Backlog");
}
