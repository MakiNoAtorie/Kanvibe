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
            text: String::new(),
            description: String::new(),
        })
    );
}

#[test]
fn the_capture_box_adds_its_description_to_the_card_and_clears_both() {
    let mut app = app_with_board();
    let todo = column_id(&app, 0);
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("Spec".into()),
            Message::CaptureDescriptionChanged("  the *why*  ".into()),
            Message::SubmitCapture,
            Message::CaptureChanged("Bare".into()),
            Message::CaptureDescriptionChanged("   ".into()),
            Message::SubmitCapture,
        ],
    );
    let cards = &app.data.boards[0].columns[0].cards;
    assert_eq!(cards[0].description, "the *why*");
    assert_eq!(cards[1].description, "", "blank adds none");
    let capture = app.capture.as_ref().unwrap();
    assert_eq!(
        (capture.text.as_str(), capture.description.as_str()),
        ("", "")
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
    // As delivered after the app's focus query found no focused field.
    Message::KeyWithFocus(
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(letter.into()),
            modified_key: keyboard::Key::Character(letter.into()),
            physical_key: Physical::Code(code),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        },
        None,
    )
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

// --- Selection and keyboard navigation (#7) ---

use kanvibe::app::Selection;
use keyboard::key::Named;

/// A named key press, mapped by `keys::map` (no field focused) and sent through `update`.
fn key(named: Named, ctrl: bool) -> Message {
    let modifiers = if ctrl {
        keyboard::Modifiers::CTRL
    } else {
        keyboard::Modifiers::empty()
    };
    let event = keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(named),
        modified_key: keyboard::Key::Named(named),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    };
    kanvibe::keys::map(&event, &Default::default()).expect("a mapped key")
}

fn card_id(app: &App, column: usize, index: usize) -> Id {
    app.data.boards[0].columns[column].cards[index].id
}

fn selected(app: &App, column: usize, card: Option<usize>) -> Option<Selection> {
    Some(Selection {
        column: column_id(app, column),
        card: card.map(|index| card_id(app, column, index)),
    })
}

/// "Work" with To do: A, B, C; Doing: empty; Done: D. Capture box closed, nothing selected.
fn app_with_cards() -> App {
    let mut app = app_with_board();
    let (to_do, done) = (column_id(&app, 0), column_id(&app, 2));
    capture(&mut app, to_do, &["A", "B", "C"]);
    capture(&mut app, done, &["D"]);
    send(&mut app, [Message::Escape]);
    app.selection = None;
    app
}

#[test]
fn with_nothing_selected_an_arrow_selects_the_first_card_of_the_first_column() {
    let mut app = app_with_cards();

    send(&mut app, [key(Named::ArrowRight, false)]);

    assert_eq!(app.selection, selected(&app, 0, Some(0)));
}

#[test]
fn with_nothing_selected_and_an_empty_first_column_an_arrow_selects_that_column() {
    let mut app = app_with_board();

    send(&mut app, [key(Named::ArrowDown, false)]);

    assert_eq!(app.selection, selected(&app, 0, None));
}

#[test]
fn up_and_down_move_within_the_column_and_stop_at_the_ends() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, Some(0));

    send(&mut app, [key(Named::ArrowUp, false)]);
    assert_eq!(app.selection, selected(&app, 0, Some(0)));

    send(&mut app, (0..5).map(|_| key(Named::ArrowDown, false)));
    assert_eq!(app.selection, selected(&app, 0, Some(2)));

    send(&mut app, [key(Named::ArrowUp, false)]);
    assert_eq!(app.selection, selected(&app, 0, Some(1)));
}

#[test]
fn left_and_right_keep_the_position_clamped_and_never_skip_empty_columns() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, Some(2)); // C

    send(&mut app, [key(Named::ArrowRight, false)]);
    assert_eq!(app.selection, selected(&app, 1, None)); // empty Doing

    send(&mut app, [key(Named::ArrowRight, false)]);
    assert_eq!(app.selection, selected(&app, 2, Some(0))); // D, clamped

    send(&mut app, [key(Named::ArrowRight, false)]);
    assert_eq!(app.selection, selected(&app, 2, Some(0))); // outermost: stays

    app.selection = selected(&app, 0, Some(2));
    send(&mut app, [key(Named::ArrowLeft, false)]);
    assert_eq!(app.selection, selected(&app, 0, Some(2)));
}

#[test]
fn clicking_a_columns_empty_area_selects_the_column() {
    let mut app = app_with_cards();
    let doing = column_id(&app, 1);

    send(
        &mut app,
        [Message::Select(Selection {
            column: doing,
            card: None,
        })],
    );

    assert_eq!(app.selection, selected(&app, 1, None));
}

#[test]
fn ctrl_left_and_right_move_the_card_to_the_bottom_of_the_neighbour_column() {
    let mut app = app_with_cards();
    let b = card_id(&app, 0, 1);
    app.selection = selected(&app, 0, Some(1)); // B

    send(
        &mut app,
        [key(Named::ArrowRight, true), key(Named::ArrowRight, true)],
    );
    assert_eq!(card_titles(&app, 0), ["A", "C"]);
    assert_eq!(card_titles(&app, 2), ["D", "B"]);
    let in_done = Some(Selection {
        column: column_id(&app, 2),
        card: Some(b),
    });
    assert_eq!(app.selection, in_done);

    send(&mut app, [key(Named::ArrowRight, true)]); // outermost: no-op, no step
    assert_eq!(card_titles(&app, 2), ["D", "B"]);

    send(&mut app, [ctrl("z", false)]); // one step per move; selection follows the Card
    assert_eq!(card_titles(&app, 1), ["B"]);
    assert_eq!(app.selection.map(|s| s.column), Some(column_id(&app, 1)));
    send(&mut app, [key(Named::ArrowLeft, true)]);
    assert_eq!(card_titles(&app, 0), ["A", "C", "B"]);
}

#[test]
fn ctrl_up_and_down_reorder_the_card_within_its_column() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, Some(0)); // A

    send(&mut app, [key(Named::ArrowUp, true)]); // top: no-op
    assert_eq!(card_titles(&app, 0), ["A", "B", "C"]);

    send(&mut app, (0..3).map(|_| key(Named::ArrowDown, true)));
    assert_eq!(card_titles(&app, 0), ["B", "C", "A"]);
    assert_eq!(app.selection, selected(&app, 0, Some(2)));

    send(&mut app, [key(Named::ArrowUp, true)]);
    assert_eq!(card_titles(&app, 0), ["B", "A", "C"]);

    send(&mut app, [ctrl("z", false)]);
    assert_eq!(card_titles(&app, 0), ["B", "C", "A"]);
}

#[test]
fn ctrl_arrows_do_nothing_without_a_selected_card() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, None);

    send(
        &mut app,
        [key(Named::ArrowRight, true), key(Named::ArrowDown, true)],
    );

    assert_eq!(card_titles(&app, 0), ["A", "B", "C"]);
    assert_eq!(app.selection, selected(&app, 0, None));
}

#[test]
fn n_opens_the_capture_box_in_the_selected_column_else_the_first() {
    let mut app = app_with_cards();

    send(&mut app, [Message::NewCard]);
    assert_eq!(
        app.capture.as_ref().map(|c| c.column),
        Some(column_id(&app, 0))
    );

    send(&mut app, [Message::Escape]);
    app.selection = selected(&app, 2, Some(0));
    send(&mut app, [Message::NewCard]);
    assert_eq!(
        app.capture.as_ref().map(|c| c.column),
        Some(column_id(&app, 2))
    );
}

#[test]
fn a_new_card_becomes_the_selected_card_and_undo_clears_that_selection() {
    let mut app = app_with_cards();
    let doing = column_id(&app, 1);

    capture(&mut app, doing, &["E", "F"]);
    assert_eq!(app.selection, selected(&app, 1, Some(1)));

    send(&mut app, [ctrl("z", false)]);
    assert_eq!(app.selection, None);
}

#[test]
fn esc_leaves_the_text_field_first_then_clears_the_selection() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, Some(1));
    send(&mut app, [Message::NewCard]);

    send(&mut app, [key(Named::Escape, false)]);
    assert_eq!(app.capture, None);
    assert_eq!(app.selection, selected(&app, 0, Some(1)));

    send(&mut app, [key(Named::Escape, false)]);
    assert_eq!(app.selection, None);
}

#[test]
fn scrolling_into_view_moves_only_as_far_as_needed() {
    use kanvibe::app::scroll_to_show;
    // A 280-wide Column in a 600-wide viewport scrolled to 100.
    assert_eq!(scroll_to_show(200.0, 280.0, 100.0, 600.0), None); // fully visible
    assert_eq!(scroll_to_show(50.0, 280.0, 100.0, 600.0), Some(50.0)); // cut on the left
    assert_eq!(scroll_to_show(584.0, 280.0, 100.0, 600.0), Some(264.0)); // cut on the right
    assert_eq!(scroll_to_show(584.0, 800.0, 100.0, 600.0), Some(584.0)); // too big: show its start
}

#[test]
fn creating_another_board_clears_the_selection() {
    let mut app = app_with_cards();
    app.selection = selected(&app, 0, Some(0));

    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Home".into()),
            Message::CreateBoard,
        ],
    );

    assert_eq!(app.selection, None);
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
fn ctrl_z_during_an_unchanged_edit_only_closes_the_field() {
    let mut app = app_with_board();
    let doing = column_id(&app, 1);

    type_into(&mut app, Field::ColumnName(doing), &["In", "Doing"]);
    send(&mut app, [ctrl("z", false)]);

    assert_eq!(app.editing, None);
    assert_eq!(app.data.boards.len(), 1, "creating the Board is not undone");
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

#[test]
fn creating_a_board_while_a_capture_box_is_open_adds_its_card_first() {
    let mut app = app_with_board();
    let todo = column_id(&app, 0);
    // The name prompt is open, then a Capture box is opened and typed into.
    send(&mut app, [Message::NewBoard]);
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("Half typed".into()),
            Message::BoardNameChanged("Home".into()),
            Message::CreateBoard,
        ],
    );

    assert_eq!(app.data.boards.len(), 2);
    assert_eq!(card_titles(&app, 0), ["Half typed"]);
    assert_eq!(app.capture, None);
}

#[test]
fn pasting_several_lines_into_the_capture_box_adds_one_card_per_line_as_one_step() {
    let mut app = app_with_board();
    let todo = column_id(&app, 0);
    capture(&mut app, todo, &["Old"]);
    send(
        &mut app,
        [
            Message::CaptureChanged("typed".into()),
            Message::CaptureLines(Some("a\r\n b \n\n  \nc\n".into())),
        ],
    );
    assert_eq!(card_titles(&app, 0), ["Old", "a", "b", "c"]);
    assert_eq!(app.capture.as_ref().map(|c| c.text.as_str()), Some(""));
    let c = app.data.boards[0].columns[0].cards[3].id;
    assert_eq!(app.selection.and_then(|s| s.card), Some(c));
    send(&mut app, [Message::Escape, Message::Undo]);
    assert_eq!(card_titles(&app, 0), ["Old"]);
    // One line: the box already holds it; nothing else happens.
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("one".into()),
            Message::CaptureLines(Some("one\n".into())),
        ],
    );
    assert_eq!(card_titles(&app, 0), ["Old"]);
    assert_eq!(app.capture.as_ref().map(|c| c.text.as_str()), Some("one"));
}

/// `app_with_board` with Labels "Bug" and "Urgent" on the Board; returns their IDs.
fn app_with_labels() -> (App, Id, Id) {
    let mut app = app_with_board();
    let board = app.data.boards[0].id;
    let bug = app.data.add_label(board, "Bug", "red").unwrap();
    let urgent = app.data.add_label(board, "Urgent", "blue").unwrap();
    app.history = Default::default();
    (app, bug, urgent)
}

fn card_labels(app: &App, index: usize) -> Vec<Vec<Id>> {
    let cards = &app.data.boards[0].columns[index].cards;
    cards.iter().map(|c| c.labels.clone()).collect()
}

#[test]
fn a_hash_label_in_the_capture_box_attaches_it_and_one_undo_removes_both() {
    let (mut app, bug, _) = app_with_labels();
    let todo = column_id(&app, 0);
    capture(&mut app, todo, &["Fix login #bug", "Fix #typo"]);
    assert_eq!(card_titles(&app, 0), ["Fix login", "Fix #typo"]);
    assert_eq!(card_labels(&app, 0), [vec![bug], vec![]]);
    send(&mut app, [Message::Escape, Message::Undo]);
    assert_eq!(card_titles(&app, 0), ["Fix login"]);
    send(&mut app, [Message::Undo]);
    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn a_capture_title_of_only_label_tokens_creates_nothing() {
    let (mut app, _, _) = app_with_labels();
    let todo = column_id(&app, 0);
    capture(&mut app, todo, &["#bug", "  #Bug #urgent "]);
    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn a_multi_line_paste_labels_each_line() {
    let (mut app, bug, urgent) = app_with_labels();
    let todo = column_id(&app, 0);
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureLines(Some("a #bug\n#urgent b #Bug\n#bug\nc".into())),
        ],
    );
    assert_eq!(card_titles(&app, 0), ["a", "b", "c"]);
    assert_eq!(card_labels(&app, 0), [vec![bug], vec![urgent, bug], vec![]]);
    send(&mut app, [Message::Escape, Message::Undo]);
    assert!(card_titles(&app, 0).is_empty());
}

#[test]
fn the_capture_box_placeholder_states_enter_and_esc() {
    let mut app = app_with_board();
    let todo = column_id(&app, 0);
    send(&mut app, [Message::OpenCapture(todo)]);
    let mut ui = iced_test::simulator(app.view());
    assert!(ui.find("Card title (Enter adds, Esc cancels)").is_ok());
}
