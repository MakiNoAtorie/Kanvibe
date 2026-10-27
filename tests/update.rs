//! Seam 1: build an App, send Messages through `update`, check the resulting state.
use iced::keyboard;
use kanvibe::app::{App, Capture, Message};
use kanvibe::domain::Id;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn column_names(app: &App) -> Vec<&str> {
    app.data.boards[0]
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect()
}

/// An App with one Board named "Work", already current.
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

fn card_titles(app: &App, index: usize) -> Vec<&str> {
    app.data.boards[0].columns[index]
        .cards
        .iter()
        .map(|c| c.title.as_str())
        .collect()
}

/// Opens the Capture box on a Column and types each title followed by Enter.
fn capture(app: &mut App, column: Id, titles: &[&str]) {
    send(app, [Message::OpenCapture(column)]);
    for title in titles {
        send(
            app,
            [
                Message::CaptureChanged((*title).into()),
                Message::SubmitCapture,
            ],
        );
    }
}

#[test]
fn creating_the_first_board_gives_three_columns_and_makes_it_current() {
    let mut app = App::default();

    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("  Work  ".into()),
            Message::CreateBoard,
        ],
    );

    assert_eq!(app.data.boards.len(), 1);
    assert_eq!(app.data.boards[0].name, "Work");
    assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
    assert_eq!(app.current_board, Some(app.data.boards[0].id));
    assert_eq!(app.board_name, None);
}

#[test]
fn a_blank_board_name_creates_nothing_and_keeps_the_prompt_open() {
    let mut app = App::default();

    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("   ".into()),
            Message::CreateBoard,
        ],
    );

    assert!(app.data.boards.is_empty());
    assert_eq!(app.current_board, None);
    assert!(app.board_name.is_some());
}

#[test]
fn esc_closes_the_board_name_prompt_without_creating_a_board() {
    let mut app = App::default();

    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::Escape,
        ],
    );

    assert!(app.data.boards.is_empty());
    assert_eq!(app.board_name, None);
}

#[test]
fn enter_in_the_capture_box_adds_cards_at_the_bottom_and_keeps_it_open() {
    let mut app = app_with_board();
    let doing = column_id(&app, 1);

    capture(&mut app, doing, &["Write spec", "Review spec"]);

    assert_eq!(card_titles(&app, 1), ["Write spec", "Review spec"]);
    assert!(card_titles(&app, 0).is_empty());
    assert_eq!(
        app.capture,
        Some(Capture {
            column: doing,
            text: String::new()
        })
    );
}

#[test]
fn esc_closes_the_capture_box_and_discards_its_text() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);

    send(
        &mut app,
        [
            Message::OpenCapture(to_do),
            Message::CaptureChanged("Half typed".into()),
            Message::Escape,
        ],
    );

    assert_eq!(app.capture, None);
    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn blank_titles_create_nothing_and_newlines_become_spaces() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);

    capture(
        &mut app,
        to_do,
        &["", "  \n ", "  Line one\nline two\r\nthree  "],
    );

    assert_eq!(card_titles(&app, 0), ["Line one line two three"]);
}

// --- Undo / redo (#5) ---

/// A Ctrl(+Shift)+letter key press, as the keyboard subscription delivers it.
fn ctrl(letter: &str, shift: bool) -> Message {
    use keyboard::key::{Code, Physical};
    let code = match letter {
        "z" => Code::KeyZ,
        "y" => Code::KeyY,
        _ => unreachable!(),
    };
    let mut modifiers = keyboard::Modifiers::CTRL;
    if shift {
        modifiers |= keyboard::Modifiers::SHIFT;
    }
    Message::Key(keyboard::Event::KeyPressed {
        key: keyboard::Key::Character(letter.into()),
        modified_key: keyboard::Key::Character(letter.into()),
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

#[test]
fn ctrl_z_undoes_the_last_change_and_ctrl_y_or_ctrl_shift_z_redo_it() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    capture(&mut app, to_do, &["One", "Two"]);

    send(&mut app, [ctrl("z", false)]);
    assert_eq!(card_titles(&app, 0), ["One"]);

    send(&mut app, [ctrl("y", false)]);
    assert_eq!(card_titles(&app, 0), ["One", "Two"]);

    send(&mut app, [ctrl("z", false), ctrl("z", false)]);
    assert!(card_titles(&app, 0).is_empty());

    send(&mut app, [ctrl("z", true)]);
    assert_eq!(card_titles(&app, 0), ["One"]);
}

#[test]
fn actions_that_change_nothing_record_no_step() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    capture(&mut app, to_do, &["One", "  ", ""]);
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged(" ".into()),
            Message::CreateBoard,
        ],
    );

    send(&mut app, [ctrl("z", false)]);

    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn the_101st_step_drops_the_oldest() {
    let mut app = app_with_board(); // step 1
    let to_do = column_id(&app, 0);
    let titles: Vec<String> = (1..=100).map(|n| n.to_string()).collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    capture(&mut app, to_do, &titles); // steps 2..=101

    send(&mut app, (0..150).map(|_| ctrl("z", false)));

    // 100 steps undone (every Card); the oldest step, creating the Board, is gone.
    assert_eq!(app.data.boards.len(), 1);
    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn a_new_step_after_undo_clears_redo() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    capture(&mut app, to_do, &["One", "Two"]);
    send(&mut app, [ctrl("z", false)]);

    capture(&mut app, to_do, &["Three"]);
    send(&mut app, [ctrl("y", false)]);

    assert_eq!(card_titles(&app, 0), ["One", "Three"]);
}

#[test]
fn undo_and_redo_show_the_board_stored_on_the_step() {
    let mut app = app_with_board();
    let work = app.data.boards[0].id;
    let to_do = column_id(&app, 0);
    capture(&mut app, to_do, &["One"]);
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Home".into()),
            Message::CreateBoard,
        ],
    );
    let home = app.data.boards[1].id;

    // Board-level step (creating Home) stores no Board: the current one stays.
    app.current_board = Some(work);
    send(&mut app, [ctrl("z", false)]);
    assert_eq!(app.data.boards.len(), 1);
    assert_eq!(app.current_board, Some(work));
    send(&mut app, [ctrl("y", false)]);
    assert_eq!(app.current_board, Some(work));
    app.current_board = Some(home);

    // Adding the Card to Work shows Work on undo and on redo.
    send(&mut app, [ctrl("z", false), ctrl("z", false)]);
    assert!(card_titles(&app, 0).is_empty());
    assert_eq!(app.current_board, Some(work));

    app.current_board = Some(home);
    send(&mut app, [ctrl("y", false)]);
    assert_eq!(app.current_board, Some(work));
}

#[test]
fn undo_never_rewinds_the_id_counter() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);
    capture(&mut app, to_do, &["One"]);
    let first = app.data.boards[0].columns[0].cards[0].id;

    send(&mut app, [ctrl("z", false)]);
    capture(&mut app, to_do, &["Two"]);

    assert!(app.data.boards[0].columns[0].cards[0].id > first);
}

// --- Text edits (#6) ---

use kanvibe::app::Field;

/// Double-clicks a name and types each text in turn, leaving the edit pending.
fn type_into(app: &mut App, field: Field, keystrokes: &[&str]) {
    send(app, [Message::StartEdit(field)]);
    send(
        app,
        keystrokes.iter().map(|t| Message::EditChanged((*t).into())),
    );
}

#[test]
fn a_rename_is_one_undo_step_not_one_per_keystroke() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;

    type_into(
        &mut app,
        Field::BoardName(board),
        &["H", "Ho", "Hom", "Home"],
    );
    send(&mut app, [Message::CommitEdit]);
    assert_eq!(app.data.boards[0].name, "Home");
    assert_eq!(app.editing, None);

    send(&mut app, [ctrl("z", false)]);
    assert_eq!(app.data.boards[0].name, "Work");
    send(&mut app, [ctrl("y", false)]);
    assert_eq!(app.data.boards[0].name, "Home");
}

#[test]
fn ctrl_z_during_a_rename_returns_the_name_to_its_starting_value() {
    let mut app = app_with_board();
    let doing = column_id(&app, 1);

    type_into(&mut app, Field::ColumnName(doing), &["In", "In progress"]);
    send(&mut app, [ctrl("z", false)]);

    assert_eq!(column_names(&app), ["To do", "Doing", "Done"]);
    assert_eq!(app.editing, None);
    assert_eq!(app.data.boards.len(), 1, "the step before is kept");
}

#[test]
fn a_blank_name_reverts_and_an_unchanged_one_records_no_step() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    let to_do = column_id(&app, 0);

    type_into(&mut app, Field::BoardName(board), &["Wo", "   "]);
    send(&mut app, [Message::CommitEdit]);
    assert_eq!(app.data.boards[0].name, "Work");
    type_into(&mut app, Field::ColumnName(to_do), &["To", "To do"]);
    send(&mut app, [Message::CommitEdit]);
    type_into(&mut app, Field::ColumnName(to_do), &["  Next  "]);
    send(&mut app, [Message::CommitEdit]);
    assert_eq!(column_names(&app)[0], "Next");

    send(&mut app, [ctrl("z", false)]);
    assert_eq!(column_names(&app)[0], "To do");
    send(&mut app, [ctrl("z", false)]);
    assert!(
        app.data.boards.is_empty(),
        "only creating the Board is left"
    );
}

#[test]
fn esc_and_opening_another_field_commit_the_pending_rename() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    let [to_do, doing, done] = [0, 1, 2].map(|i| column_id(&app, i));

    type_into(&mut app, Field::ColumnName(to_do), &["Backlog"]);
    send(&mut app, [Message::Escape]);
    type_into(&mut app, Field::ColumnName(doing), &["Now"]);
    type_into(&mut app, Field::ColumnName(done), &["Shipped"]);
    send(&mut app, [Message::OpenCapture(to_do)]);
    assert_eq!(app.editing, None);
    type_into(&mut app, Field::BoardName(board), &["Job"]);
    send(&mut app, [Message::NewBoard]);
    assert_eq!(app.editing, None);
    assert_eq!(app.data.boards[0].name, "Job");
    assert_eq!(column_names(&app), ["Backlog", "Now", "Shipped"]);

    // Each rename was its own step.
    send(&mut app, [ctrl("z", false), ctrl("z", false)]);
    assert_eq!(app.data.boards[0].name, "Work");
    assert_eq!(column_names(&app), ["Backlog", "Now", "Done"]);
}

#[test]
fn the_idle_write_records_no_step() {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    let later = std::time::Instant::now() + std::time::Duration::from_secs(5);

    type_into(&mut app, Field::BoardName(board), &["Home"]);
    send(
        &mut app,
        [
            Message::EditTick(later),
            Message::EditChanged("Homes".into()),
            Message::CommitEdit,
            ctrl("z", false),
        ],
    );

    assert_eq!(app.data.boards[0].name, "Work");
}

#[test]
fn the_capture_box_is_a_pending_edit_too() {
    let mut app = app_with_board();
    let to_do = column_id(&app, 0);

    // Opening another field adds the typed Card and closes the box.
    send(
        &mut app,
        [
            Message::OpenCapture(to_do),
            Message::CaptureChanged("Half typed".into()),
            Message::StartEdit(Field::ColumnName(to_do)),
        ],
    );
    assert_eq!(card_titles(&app, 0), ["Half typed"]);
    assert_eq!(app.capture, None);
    assert!(app.editing.is_some());

    // Opening the capture box commits the rename; Ctrl+Z in it drops the typed text.
    send(
        &mut app,
        [
            Message::OpenCapture(to_do),
            Message::CaptureChanged("Oops".into()),
            ctrl("z", false),
        ],
    );
    assert_eq!(card_titles(&app, 0), ["Half typed"]);
    assert_eq!(app.capture, None);
    assert!(app.editing.is_none());
}
