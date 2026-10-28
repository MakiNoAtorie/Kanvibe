//! Seam 1 for multiple Boards and the sidebar (#12).
use kanvibe::app::{App, Field, Message, Selection};
use kanvibe::domain::Id;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn create_board(app: &mut App, name: &str) -> Id {
    send(
        app,
        [
            Message::NewBoard,
            Message::BoardNameChanged(name.into()),
            Message::CreateBoard,
        ],
    );
    app.current_board.unwrap()
}

/// An App with Boards "A", "B", "C" in that order; "C" (the last created) is current.
fn app_with_boards() -> (App, [Id; 3]) {
    let mut app = App::default();
    let ids = ["A", "B", "C"].map(|name| create_board(&mut app, name));
    (app, ids)
}

fn names(app: &App) -> Vec<&str> {
    app.data.boards.iter().map(|b| b.name.as_str()).collect()
}

#[test]
fn clicking_a_board_switches_to_it_and_clears_capture_and_selection() {
    let (mut app, [a, _, c]) = app_with_boards();
    let todo = app.data.boards[2].columns[0].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column: todo,
            card: None,
        })],
    );
    send(&mut app, [Message::OpenCapture(todo)]);

    send(&mut app, [Message::OpenBoard(a)]);

    assert_eq!(app.current_board, Some(a));
    assert_eq!(app.selection, None);
    assert_eq!(app.capture, None);
    assert_ne!(app.current_board, Some(c));
    assert_eq!(names(&app), ["A", "B", "C"]);
}

fn archived(app: &App) -> Vec<&str> {
    let boards = app.data.boards.iter().filter(|b| b.archived);
    boards.map(|b| b.name.as_str()).collect()
}

#[test]
fn archiving_the_current_board_opens_the_next_below_else_above_else_none() {
    let (mut app, [a, b, c]) = app_with_boards();
    send(&mut app, [Message::OpenBoard(b)]);

    send(&mut app, [Message::ArchiveBoard(b)]);
    assert_eq!(archived(&app), ["B"]);
    assert_eq!(app.current_board, Some(c), "the next below");

    send(&mut app, [Message::ArchiveBoard(c)]);
    assert_eq!(app.current_board, Some(a), "nothing below: the one above");

    send(&mut app, [Message::ArchiveBoard(a)]);
    assert_eq!(app.current_board, None, "No open Boards");
}

#[test]
fn archiving_another_board_keeps_the_current_one_and_archived_boards_cannot_be_opened() {
    let (mut app, [a, _, c]) = app_with_boards();

    send(&mut app, [Message::ArchiveBoard(a), Message::OpenBoard(a)]);

    assert_eq!(app.current_board, Some(c));
}

#[test]
fn archive_is_one_undo_step() {
    let (mut app, [_, b, c]) = app_with_boards();
    send(&mut app, [Message::OpenBoard(b), Message::ArchiveBoard(b)]);

    send(&mut app, [Message::Undo]);
    assert!(archived(&app).is_empty());
    assert_eq!(
        app.current_board,
        Some(c),
        "undo does not switch back by itself"
    );

    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["A", "B"], "the step before was creating C");
    assert_eq!(app.current_board, Some(b), "C is gone: the one above");
}

#[test]
fn a_restored_board_goes_to_the_bottom_of_the_active_list_in_one_undo_step() {
    let (mut app, [a, _, _]) = app_with_boards();
    send(&mut app, [Message::ArchiveBoard(a)]);

    send(&mut app, [Message::RestoreBoard(a)]);
    assert!(archived(&app).is_empty());
    assert_eq!(names(&app), ["B", "C", "A"]);
    assert_eq!(app.current_board, Some(a), "restoring opens it");

    send(&mut app, [Message::Undo]);
    assert_eq!(archived(&app), ["A"]);
    assert_eq!(names(&app), ["A", "B", "C"]);
}

/// Board "A" with two Cards, archived; the delete confirmation for it is open.
fn asking_to_delete_a() -> (App, Id) {
    let (mut app, [a, ..]) = app_with_boards();
    let todo = app.data.boards[0].columns[0].id;
    send(&mut app, [Message::OpenCapture(todo)]);
    for title in ["one", "two"] {
        send(
            &mut app,
            [
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
            ],
        );
    }
    send(
        &mut app,
        [Message::ArchiveBoard(a), Message::AskDeleteBoard(a)],
    );
    (app, a)
}

#[test]
fn delete_asks_first_naming_the_board_and_its_card_counts() {
    let (app, _) = asking_to_delete_a();

    let confirm = app.confirm.as_ref().expect("the confirmation is open");
    assert_eq!(
        confirm.prompt,
        "Permanently delete Board \"A\" with 2 Cards (0 archived)? \
         You can undo this until Kanvibe closes."
    );
    assert_eq!(archived(&app), ["A"], "nothing deleted yet");
}

#[test]
fn confirming_deletes_the_board_in_one_undo_step() {
    let (mut app, a) = asking_to_delete_a();

    send(&mut app, [Message::Confirm]);
    assert_eq!(app.confirm, None);
    assert_eq!(names(&app), ["B", "C"]);

    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["A", "B", "C"]);
    assert_eq!(archived(&app), ["A"]);
    let undone = app.data.boards.iter().find(|b| b.id == a).unwrap();
    assert_eq!(undone.columns[0].cards.len(), 2);
}

#[test]
fn esc_closes_the_confirmation_without_deleting() {
    let (mut app, _) = asking_to_delete_a();

    send(&mut app, [Message::Escape]);

    assert_eq!(app.confirm, None);
    assert_eq!(names(&app), ["A", "B", "C"]);
}

#[test]
fn rename_in_the_board_menu_closes_the_menu_and_is_one_undo_step() {
    let (mut app, [_, _, c]) = app_with_boards();
    send(&mut app, [Message::ToggleBoardMenu]);
    assert!(app.board_menu);

    send(&mut app, [Message::StartEdit(Field::BoardName(c))]);
    assert!(!app.board_menu);
    send(
        &mut app,
        [
            Message::EditChanged("Ca".into()),
            Message::EditChanged("Cat".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(names(&app), ["A", "B", "Cat"]);

    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["A", "B", "C"]);
}

#[test]
fn esc_closes_the_board_menu() {
    let (mut app, _) = app_with_boards();

    send(&mut app, [Message::ToggleBoardMenu, Message::Escape]);

    assert!(!app.board_menu);
}

/// Sends each Message, then runs every save it triggers, as the program does.
fn send_saving(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
        while let Some(job) = app.save_if_changed() {
            let _ = app.update(job.run());
        }
    }
}

#[test]
fn collapsed_sidebar_and_last_board_persist_across_restarts() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    let mut app = App::open(dir.clone()).unwrap();
    let [a, ..] = ["A", "B", "C"].map(|name| create_board(&mut app, name));
    send_saving(&mut app, [Message::OpenBoard(a), Message::ToggleSidebar]);
    assert!(app.sidebar_collapsed);

    let app = App::open(dir.clone()).unwrap();
    assert!(app.sidebar_collapsed);
    assert_eq!(app.current_board, Some(a));
}

#[test]
fn reopening_with_an_archived_last_board_opens_the_first_active_board() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("data");
    let mut app = App::open(dir.clone()).unwrap();
    let [a, b, _] = ["A", "B", "C"].map(|name| create_board(&mut app, name));
    send_saving(&mut app, [Message::OpenBoard(b), Message::ArchiveBoard(b)]);
    // Simulate a last_board that points at an archived Board.
    let mut settings = app.saved.1.clone();
    settings.last_board = Some(b);
    kanvibe::storage::save(&dir, &app.data, &settings).unwrap();

    let app = App::open(dir).unwrap();

    assert_eq!(app.current_board, Some(a));
}

#[test]
fn the_delete_prompt_counts_active_and_archived_cards_together() {
    let (mut app, [a, ..]) = app_with_boards();
    let todo = app.data.boards[0].columns[0].id;
    send(&mut app, [Message::OpenCapture(todo)]);
    for title in ["one", "two"] {
        send(
            &mut app,
            [
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
            ],
        );
    }
    let one = app.data.boards[0].columns[0].cards[0].id;
    send(
        &mut app,
        [
            Message::Escape,
            Message::ArchiveCard(one),
            Message::ArchiveBoard(a),
            Message::AskDeleteBoard(a),
        ],
    );

    assert_eq!(
        app.confirm.as_ref().unwrap().prompt,
        "Permanently delete Board \"A\" with 2 Cards (1 archived)? \
         You can undo this until Kanvibe closes."
    );
}

#[test]
fn archiving_a_board_stamps_archived_at_and_restoring_clears_it() {
    let (mut app, [a, ..]) = app_with_boards();

    send(&mut app, [Message::ArchiveBoard(a)]);
    let stamp = app.data.boards[0].archived_at.clone().expect("stamped");
    assert!(stamp.parse::<jiff::Timestamp>().is_ok(), "{stamp}");

    send(&mut app, [Message::RestoreBoard(a)]);
    let restored = app.data.boards.iter().find(|b| b.id == a).unwrap();
    assert_eq!(restored.archived_at, None);
}

#[test]
fn active_boards_cannot_be_deleted() {
    let (mut app, [_, b, _]) = app_with_boards();

    send(&mut app, [Message::DeleteBoard(b)]);

    assert_eq!(names(&app), ["A", "B", "C"]);
}

#[test]
fn undoing_the_archive_of_the_last_open_board_opens_it_again() {
    let (mut app, [a, b, c]) = app_with_boards();
    send(
        &mut app,
        [Message::ArchiveBoard(a), Message::ArchiveBoard(b)],
    );
    send(&mut app, [Message::ArchiveBoard(c)]);
    assert_eq!(app.current_board, None, "No open Boards");

    send(&mut app, [Message::Undo]);
    assert_eq!(
        app.current_board,
        Some(c),
        "an active Board is never left unopened"
    );

    send(
        &mut app,
        [Message::ArchiveBoard(c), Message::RestoreBoard(a)],
    );
    assert_eq!(
        app.current_board,
        Some(a),
        "restoring with nothing open opens it"
    );
}

#[test]
fn archiving_shows_the_archived_section_and_restoring_opens_the_board() {
    let (mut app, [a, b, c]) = app_with_boards();
    assert!(!app.archived_open);
    send(&mut app, [Message::ArchiveBoard(a)]);
    assert!(app.archived_open, "the user sees where the Board went");
    assert_eq!(app.current_board, Some(c));
    let todo = app.data.boards.iter().find(|x| x.id == c).unwrap().columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("Typed".into()),
            Message::RestoreBoard(a),
        ],
    );
    assert_eq!(app.current_board, Some(a));
    assert_eq!(names(&app), ["B", "C", "A"]);
    let c_board = app.data.boards.iter().find(|x| x.id == c).unwrap();
    assert_eq!(
        c_board.columns[0].cards.len(),
        1,
        "the Capture box was committed"
    );
    let _ = b;
}

#[test]
fn first_run_shows_the_app_name_above_create_your_first_board() {
    let app = App::default();
    let mut ui = iced_test::simulator(app.view());
    assert!(ui.find("Kanvibe").is_ok());
    assert!(ui.find("Create your first Board").is_ok());
}

#[test]
fn the_board_name_prompt_has_a_create_button() {
    let mut app = App::default();
    send(&mut app, [Message::NewBoard]);
    assert!(iced_test::simulator(app.view()).find("Create").is_ok());
    // The sidebar's prompt too.
    create_board(&mut app, "A");
    send(&mut app, [Message::NewBoard]);
    let mut ui = iced_test::simulator(app.view());
    let _ = ui.click("Create");
    let created = ui.into_messages().any(|m| m == Message::CreateBoard);
    assert!(!created, "disabled while blank");
    send(&mut app, [Message::BoardNameChanged("B".into())]);
    let mut ui = iced_test::simulator(app.view());
    ui.click("Create").expect("a Create button");
    for message in ui.into_messages() {
        send(&mut app, [message]);
    }
    assert_eq!(names(&app), ["A", "B"]);
}

#[test]
fn the_archived_heading_shows_whether_it_is_open() {
    let (mut app, [a, _, _]) = app_with_boards();
    send(&mut app, [Message::ArchiveBoard(a)]); // opens the section
    assert!(
        iced_test::simulator(app.view())
            .find("Archived (1) ▾")
            .is_ok()
    );
    send(&mut app, [Message::ToggleArchivedBoards]);
    assert!(
        iced_test::simulator(app.view())
            .find("Archived (1) ▸")
            .is_ok()
    );
}

#[test]
fn ctrl_tab_cycles_the_active_boards_and_wraps() {
    let (mut app, [a, b, c]) = app_with_boards();
    send(&mut app, [Message::CycleBoard(true)]);
    assert_eq!(
        app.current_board,
        Some(a),
        "wraps from the last to the first"
    );
    send(&mut app, [Message::CycleBoard(false)]);
    assert_eq!(app.current_board, Some(c));

    send(&mut app, [Message::ArchiveBoard(b), Message::OpenBoard(a)]);
    send(&mut app, [Message::CycleBoard(true)]);
    assert_eq!(app.current_board, Some(c), "skips archived Boards");

    send(
        &mut app,
        [Message::ArchiveBoard(a), Message::ArchiveBoard(c)],
    );
    send(&mut app, [Message::CycleBoard(true)]);
    assert_eq!(app.current_board, None, "no active Board: nothing to open");
}
