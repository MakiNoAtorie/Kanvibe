//! Column and Board drag to reorder, through the drag Messages sent to `update`.
use iced::Point;
use kanvibe::app::{App, Message};
use kanvibe::domain::Id;
use kanvibe::drag::{Item, Target};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Boards A, B, C, D created in order; B is then archived. D is current.
fn app_with_boards() -> App {
    let mut app = App::default();
    for name in ["A", "B", "C", "D"] {
        send(
            &mut app,
            [
                Message::NewBoard,
                Message::BoardNameChanged(name.into()),
                Message::CreateBoard,
            ],
        );
    }
    let b = board(&app, "B");
    send(&mut app, [Message::ArchiveBoard(b)]);
    app
}

fn board(app: &App, name: &str) -> Id {
    app.data.boards.iter().find(|b| b.name == name).unwrap().id
}

fn archived(app: &App, name: &str) -> bool {
    app.data
        .boards
        .iter()
        .find(|b| b.name == name)
        .unwrap()
        .archived
}

fn board_names(app: &App) -> Vec<&str> {
    app.data.boards.iter().map(|b| b.name.as_str()).collect()
}

/// The current Board's Column names (To do, Doing, Done by default).
fn column_names(app: &App) -> Vec<&str> {
    let board = app.data.active_board(app.current_board.unwrap()).unwrap();
    board.columns.iter().map(|c| c.name.as_str()).collect()
}

fn column(app: &App, name: &str) -> Id {
    let board = app.data.active_board(app.current_board.unwrap()).unwrap();
    board.columns.iter().find(|c| c.name == name).unwrap().id
}

/// Presses `item`, moves past the threshold, hovers `target` (if any) and releases.
fn drag(app: &mut App, item: Item, target: Option<Target>) {
    send(
        app,
        [
            Message::DragPress(item),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(100.0, 120.0)),
        ],
    );
    send(app, target.map(Message::DragHover));
    send(app, [Message::DragEnd]);
}

#[test]
fn dragging_a_column_header_into_a_gap_reorders_it_as_one_undo_step() {
    let mut app = app_with_boards();
    let todo = column(&app, "To do");

    drag(&mut app, Item::Column(todo), Some(Target::ColumnGap(3)));

    assert_eq!(column_names(&app), ["Doing", "Done", "To do"]);
    assert_eq!(app.drag, None);
    send(&mut app, [Message::Undo]);
    assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
}

#[test]
fn dropping_a_column_in_a_gap_before_it_moves_it_up() {
    let mut app = app_with_boards();
    let done = column(&app, "Done");

    drag(&mut app, Item::Column(done), Some(Target::ColumnGap(0)));

    assert_eq!(column_names(&app), ["Done", "To do", "Doing"]);
}

#[test]
fn dropping_a_column_in_place_records_nothing() {
    let mut app = app_with_boards();
    let doing = column(&app, "Doing");

    // The gaps right before and right after it.
    for gap in [1, 2] {
        drag(&mut app, Item::Column(doing), Some(Target::ColumnGap(gap)));
        assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
    }

    send(&mut app, [Message::Undo]);
    assert!(!archived(&app, "B")); // the last step was archiving B, not a reorder
}

#[test]
fn dragging_a_board_into_a_gap_counts_active_boards_only() {
    let mut app = app_with_boards();
    let d = board(&app, "D");

    // Active Boards are A, C, D: gap 0 is before A.
    drag(&mut app, Item::Board(d), Some(Target::BoardGap(0)));

    assert_eq!(board_names(&app), ["D", "A", "B", "C"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(board_names(&app), ["A", "B", "C", "D"]);
}

#[test]
fn dropping_a_board_in_the_gap_after_the_last_active_board_moves_it_last() {
    let mut app = app_with_boards();
    let a = board(&app, "A");

    drag(&mut app, Item::Board(a), Some(Target::BoardGap(3)));

    let active: Vec<_> = board_names(&app)
        .into_iter()
        .filter(|n| *n != "B")
        .collect();
    assert_eq!(active, ["C", "D", "A"]);
    assert!(archived(&app, "B"));
}

#[test]
fn dropping_a_board_in_place_records_nothing() {
    let mut app = app_with_boards();
    let c = board(&app, "C");

    for gap in [1, 2] {
        drag(&mut app, Item::Board(c), Some(Target::BoardGap(gap)));
        assert_eq!(board_names(&app), ["A", "B", "C", "D"]);
    }

    send(&mut app, [Message::Undo]);
    assert!(!archived(&app, "B")); // the last step was archiving B, not a reorder
}

#[test]
fn cancel_and_release_outside_leave_everything_where_it_was() {
    let mut app = app_with_boards();
    let todo = column(&app, "To do");
    let a = board(&app, "A");

    drag(&mut app, Item::Column(todo), None);
    drag(&mut app, Item::Board(a), None);
    send(
        &mut app,
        [
            Message::DragPress(Item::Column(todo)),
            Message::DragMoved(Point::new(1.0, 1.0)),
            Message::DragMoved(Point::new(1.0, 50.0)),
            Message::DragHover(Target::ColumnGap(3)),
            Message::DragCancel,
            Message::DragEnd,
        ],
    );

    assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
    assert_eq!(board_names(&app), ["A", "B", "C", "D"]);
}

#[test]
fn a_board_dropped_on_a_column_gap_or_a_column_on_a_board_gap_does_nothing() {
    let mut app = app_with_boards();
    let todo = column(&app, "To do");
    let a = board(&app, "A");

    drag(&mut app, Item::Board(a), Some(Target::ColumnGap(2)));
    drag(&mut app, Item::Column(todo), Some(Target::BoardGap(0)));

    assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
    assert_eq!(board_names(&app), ["A", "B", "C", "D"]);
}

#[test]
fn clicking_a_board_row_without_dragging_opens_the_board() {
    let mut app = app_with_boards();
    let a = board(&app, "A");

    send(
        &mut app,
        [Message::DragPress(Item::Board(a)), Message::DragEnd],
    );

    assert_eq!(app.current_board, Some(a));
    assert_eq!(board_names(&app), ["A", "B", "C", "D"]);
}
