//! Seam 1 for Column adding and Column Limits (#10).
use kanvibe::app::{App, Message};
use kanvibe::domain::Id;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn app_with_board() -> App {
    let mut app = App::default();
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    app
}

fn column_id(app: &App, index: usize) -> Id {
    app.data.boards[0].columns[index].id
}

fn undo(app: &mut App) {
    send(app, [Message::Undo]);
}

/// Opens the limit prompt on a Column and submits `input`.
fn set_limit(app: &mut App, column: Id, input: &str) {
    send(
        app,
        [
            Message::OpenLimit(column),
            Message::LimitChanged(input.into()),
            Message::SubmitLimit,
        ],
    );
}

fn label(app: &App, index: usize) -> String {
    app.data.boards[0].columns[index].count_label()
}

#[test]
fn add_column_appends_at_the_end_and_is_one_undo_step() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;

    send(&mut app, [Message::AddColumn(board)]);
    let names: Vec<_> = app.data.boards[0]
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["To do", "Doing", "Done", "New column"]);
    assert!(app.data.boards[0].columns[3].cards.is_empty());

    undo(&mut app);
    assert_eq!(app.data.boards[0].columns.len(), 3);
}

#[test]
fn a_board_with_no_columns_can_get_one() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    app.data.boards[0].columns.clear();

    send(&mut app, [Message::AddColumn(board)]);
    assert_eq!(app.data.boards[0].columns.len(), 1);
}

#[test]
fn add_column_to_an_unknown_board_does_nothing() {
    let mut app = app_with_board();
    send(&mut app, [Message::AddColumn(999)]);
    assert_eq!(app.data.boards[0].columns.len(), 3);
}

#[test]
fn limit_is_set_and_shown_as_count_over_limit() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    assert_eq!(label(&app, 0), "0");

    set_limit(&mut app, to_do, " 3 ");
    assert_eq!(app.data.boards[0].columns[0].limit, Some(3));
    assert_eq!(label(&app, 0), "0/3");
    assert_eq!(app.limit_input, None, "the prompt closes on success");

    undo(&mut app);
    assert_eq!(app.data.boards[0].columns[0].limit, None);
}

#[test]
fn empty_input_removes_the_limit() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    set_limit(&mut app, to_do, "3");

    set_limit(&mut app, to_do, "  ");
    assert_eq!(app.data.boards[0].columns[0].limit, None);
    assert_eq!(label(&app, 0), "0");
}

#[test]
fn non_positive_or_non_numeric_limits_are_refused() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    set_limit(&mut app, to_do, "2");

    for bad in ["0", "-1", "abc", "1.5", "99999999999999999999"] {
        set_limit(&mut app, to_do, bad);
        assert_eq!(app.data.boards[0].columns[0].limit, Some(2), "{bad:?}");
        assert!(app.limit_input.is_some(), "{bad:?} keeps the prompt open");
    }
    // The refused inputs recorded no step: one undo clears the limit set before them.
    send(&mut app, [Message::Escape]);
    undo(&mut app);
    assert_eq!(app.data.boards[0].columns[0].limit, None);
}

#[test]
fn a_column_over_its_limit_is_flagged_but_still_accepts_cards() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    set_limit(&mut app, to_do, "2");

    for title in ["One", "Two"] {
        send(
            &mut app,
            [
                Message::OpenCapture(to_do),
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
            ],
        );
    }
    assert!(!app.data.boards[0].columns[0].over_limit(), "at the limit");
    assert_eq!(label(&app, 0), "2/2");

    send(
        &mut app,
        [
            Message::OpenCapture(to_do),
            Message::CaptureChanged("Three".into()),
            Message::SubmitCapture,
            Message::Escape,
        ],
    );
    assert!(app.data.boards[0].columns[0].over_limit());
    assert_eq!(app.data.boards[0].columns[0].cards.len(), 3);
    assert_eq!(label(&app, 0), "3/2");
    assert!(!app.data.boards[0].columns[1].over_limit());
}

#[test]
fn the_header_menu_opens_and_esc_closes_it() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);

    send(&mut app, [Message::ToggleColumnMenu(to_do)]);
    assert_eq!(app.column_menu, Some(to_do));
    send(&mut app, [Message::OpenLimit(to_do)]);
    assert_eq!(app.column_menu, None);
    assert_eq!(app.limit_input.as_ref().map(|l| l.column), Some(to_do));
    send(&mut app, [Message::Escape]);
    assert_eq!(app.limit_input, None);
}

#[test]
fn undo_that_removes_a_column_closes_its_menu_and_limit_prompt() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    send(&mut app, [Message::AddColumn(board)]);
    let added = column_id(&app, 3);

    send(
        &mut app,
        [Message::OpenLimit(added), Message::LimitChanged("4".into())],
    );
    undo(&mut app);
    assert_eq!(app.data.boards[0].columns.len(), 3);
    assert_eq!(app.limit_input, None, "its Column is gone");

    send(&mut app, [Message::Redo, Message::ToggleColumnMenu(added)]);
    undo(&mut app);
    assert_eq!(app.column_menu, None, "its Column is gone");
}

#[test]
fn a_refused_limit_says_why_until_the_text_changes() {
    const WHY: &str = "Enter a whole number of 1 or more, or leave blank to remove";
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    send(&mut app, [Message::OpenLimit(to_do)]);
    assert!(iced_test::simulator(app.view()).find(WHY).is_err());
    send(
        &mut app,
        [Message::LimitChanged("0".into()), Message::SubmitLimit],
    );
    assert!(app.limit_input.as_ref().unwrap().refused);
    assert!(iced_test::simulator(app.view()).find(WHY).is_ok());
    send(&mut app, [Message::LimitChanged("3".into())]);
    assert!(!app.limit_input.as_ref().unwrap().refused);
}
