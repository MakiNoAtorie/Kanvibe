//! Moving a Card to another Board (#16): the picker / M, drop on a sidebar Board, Labels.
use iced::Point;
use iced::keyboard::{self, Key, Modifiers};
use kanvibe::app::{App, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::drag::{Item, Target};
use kanvibe::keys::{self, Context};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Boards "Src" (current; To do: A, B, C) and "Dst" (To do: X; Doing; Done).
fn app_with_two_boards() -> App {
    let mut app = App::default();
    for name in ["Dst", "Src"] {
        send(
            &mut app,
            [
                Message::NewBoard,
                Message::BoardNameChanged(name.into()),
                Message::CreateBoard,
            ],
        );
        let column = app.data.boards.last().unwrap().columns[0].id;
        let titles: &[&str] = if name == "Src" {
            &["A", "B", "C"]
        } else {
            &["X"]
        };
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
        send(&mut app, [Message::Escape]);
    }
    app.selection = None;
    app
}

fn board_id(app: &App, name: &str) -> Id {
    app.data.boards.iter().find(|b| b.name == name).unwrap().id
}

fn card_id(app: &App, title: &str) -> Id {
    let cards = app.data.boards.iter().flat_map(|b| &b.columns);
    let mut cards = cards.flat_map(|c| &c.cards);
    cards.find(|c| c.title == title).expect("card").id
}

/// Card titles of the first Column of `board`.
fn first_column(app: &App, board: &str) -> Vec<String> {
    let board = &app.data.boards[app
        .data
        .boards
        .iter()
        .position(|b| b.name == board)
        .unwrap()];
    let cards = &board.columns[0].cards;
    cards.iter().map(|c| c.title.clone()).collect()
}

fn move_to(app: &mut App, card: &str, board: &str) {
    let (card, board) = (card_id(app, card), board_id(app, board));
    send(app, [Message::MoveToBoard(card, board)]);
}

fn select(app: &mut App, title: &str) {
    let card = card_id(app, title);
    let column = app.data.boards[1].columns[0].id;
    send(
        app,
        [Message::Select(Selection {
            column,
            card: Some(card),
        })],
    );
}

fn press(key: Key, focused: Option<keys::Input>) -> Option<Message> {
    let event = keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    };
    keys::map(
        &event,
        &Context {
            focused,
            ..Context::default()
        },
    )
}

#[test]
fn moves_to_the_bottom_of_the_first_column_and_stays_on_the_current_board() {
    let mut app = app_with_two_boards();
    let src = board_id(&app, "Src");
    let a = card_id(&app, "A");
    send(&mut app, [Message::OpenBoard(src)]);
    move_to(&mut app, "A", "Dst");
    assert_eq!(first_column(&app, "Dst"), ["X", "A"]);
    assert_eq!(first_column(&app, "Src"), ["B", "C"]);
    assert_eq!(app.current_board, Some(src));
    assert_eq!(card_id(&app, "A"), a);
}

#[test]
fn keeps_created_description_and_checklist_and_updates_the_time() {
    let mut app = app_with_two_boards();
    let a = card_id(&app, "A");
    {
        let card = app
            .data
            .boards
            .iter_mut()
            .flat_map(|b| &mut b.columns)
            .flat_map(|c| &mut c.cards)
            .find(|c| c.id == a)
            .unwrap();
        card.created = "2020-01-01T00:00:00Z".into();
        card.updated = "2020-01-01T00:00:00Z".into();
        card.description = "**hi**".into();
    }
    send(
        &mut app,
        [
            Message::AddItem(a),
            Message::EditChanged("one".into()),
            Message::CommitEdit,
        ],
    );
    let src = board_id(&app, "Src");
    send(&mut app, [Message::OpenBoard(src)]);
    move_to(&mut app, "A", "Dst");
    let card = app.data.card(a).unwrap();
    assert_eq!(card.created, "2020-01-01T00:00:00Z");
    assert_eq!(card.description, "**hi**");
    assert_eq!(card.checklist.len(), 1);
    assert_ne!(card.updated, "2020-01-01T00:00:00Z");
}

#[test]
fn selection_moves_to_the_neighbour_and_the_panel_closes() {
    let mut app = app_with_two_boards();
    select(&mut app, "B");
    send(&mut app, [Message::OpenPanel]);
    let b = card_id(&app, "B");
    assert_eq!(app.panel, Some(b));
    move_to(&mut app, "B", "Dst");
    assert_eq!(app.panel, None);
    let c = card_id(&app, "C");
    assert_eq!(app.selection.and_then(|s| s.card), Some(c));
}

#[test]
fn m_opens_the_picker_for_the_open_card_and_choosing_moves_it() {
    let mut app = app_with_two_boards();
    select(&mut app, "A");
    send(&mut app, [Message::OpenPanel]);
    assert_eq!(
        press(Key::Character("m".into()), None),
        Some(Message::OpenMove)
    );
    let typing = Some(keys::Input::Edit);
    assert_eq!(press(Key::Character("m".into()), typing), None);
    send(&mut app, [Message::OpenMove]);
    let a = card_id(&app, "A");
    assert_eq!(app.move_picker, Some(a));
    move_to(&mut app, "A", "Dst");
    assert_eq!(app.move_picker, None);
    assert_eq!(first_column(&app, "Dst"), ["X", "A"]);
    // Esc closes an open picker without moving.
    select(&mut app, "B");
    send(&mut app, [Message::OpenMove, Message::Escape]);
    assert_eq!(app.move_picker, None);
    assert_eq!(first_column(&app, "Src"), ["B", "C"]);
}

#[test]
fn the_shortcut_is_listed() {
    assert!(kanvibe::chrome::SHORTCUTS.iter().any(|(k, _)| *k == "M"));
}

#[test]
fn own_and_archived_boards_are_no_ops_with_no_step() {
    let mut app = app_with_two_boards();
    let dst = board_id(&app, "Dst");
    send(&mut app, [Message::ArchiveBoard(dst)]);
    let before = app.data.boards.clone();
    move_to(&mut app, "A", "Src"); // own Board
    move_to(&mut app, "A", "Dst"); // archived
    let a = card_id(&app, "A");
    send(&mut app, [Message::MoveToBoard(a, 9999)]);
    assert_eq!(app.data.boards, before);
    // The only step left to undo is archiving Dst.
    send(&mut app, [Message::Undo]);
    assert!(!app.data.boards.iter().any(|b| b.archived));
}

#[test]
fn drop_on_a_sidebar_board_moves_the_card() {
    let mut app = app_with_two_boards();
    let (a, dst) = (card_id(&app, "A"), board_id(&app, "Dst"));
    send(
        &mut app,
        [
            Message::DragPress(Item::Card(a)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(10.0, 100.0)),
            Message::DragHover(Target::Board(dst)),
            Message::DragEnd,
        ],
    );
    assert_eq!(first_column(&app, "Dst"), ["X", "A"]);
    assert_eq!(first_column(&app, "Src"), ["B", "C"]);
}

#[test]
fn drop_on_the_own_board_row_does_nothing() {
    let mut app = app_with_two_boards();
    let (a, src) = (card_id(&app, "A"), board_id(&app, "Src"));
    send(
        &mut app,
        [
            Message::DragPress(Item::Card(a)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(10.0, 100.0)),
            Message::DragHover(Target::Board(src)),
            Message::DragEnd,
        ],
    );
    assert_eq!(first_column(&app, "Src"), ["A", "B", "C"]);
}

#[test]
fn a_target_without_columns_gets_a_to_do_column() {
    let mut app = app_with_two_boards();
    let dst = board_id(&app, "Dst");
    let columns: Vec<Id> = app.data.boards[0].columns.iter().map(|c| c.id).collect();
    send(&mut app, columns.into_iter().map(Message::RemoveColumn));
    assert!(app.data.boards[0].columns.is_empty());
    move_to(&mut app, "A", "Dst");
    let board = app.data.boards.iter().find(|b| b.id == dst).unwrap();
    assert_eq!(board.columns.len(), 1);
    assert_eq!(board.columns[0].name, "To do");
    assert_eq!(first_column(&app, "Dst"), ["A"]);
}

/// Labels of the Card `title`, as (name, colour).
fn labels_of(app: &App, title: &str) -> Vec<(String, String)> {
    let labels = app.data.card_labels(card_id(app, title));
    labels
        .into_iter()
        .map(|l| (l.name.clone(), l.color.clone()))
        .collect()
}

fn board_labels(app: &App, board: &str) -> Vec<(String, String)> {
    let board = app.data.boards.iter().find(|b| b.name == board).unwrap();
    let labels = board.labels.iter();
    labels.map(|l| (l.name.clone(), l.color.clone())).collect()
}

/// Gives Card A a Label on Src, and "Dst" the Labels `on_target`.
fn with_label(on_source: (&str, &str), on_target: &[(&str, &str)]) -> App {
    let mut app = app_with_two_boards();
    let (src, dst) = (board_id(&app, "Src"), board_id(&app, "Dst"));
    let label = app.data.add_label(src, on_source.0, on_source.1).unwrap();
    for (name, color) in on_target {
        app.data.add_label(dst, name, color).unwrap();
    }
    app.data.toggle_label(card_id(&app, "A"), label).unwrap();
    app
}

#[test]
fn a_matching_label_is_used() {
    let mut app = with_label(("Bug", "red"), &[("Bug", "red")]);
    move_to(&mut app, "A", "Dst");
    assert_eq!(labels_of(&app, "A"), [("Bug".into(), "red".into())]);
    assert_eq!(board_labels(&app, "Dst").len(), 1);
}

#[test]
fn labels_match_case_insensitively_trimmed_and_the_target_colour_wins() {
    let mut app = with_label(("Bug", "red"), &[(" bUG", "blue")]);
    move_to(&mut app, "A", "Dst");
    assert_eq!(labels_of(&app, "A"), [("bUG".into(), "blue".into())]);
    assert_eq!(board_labels(&app, "Dst").len(), 1);
}

#[test]
fn a_missing_label_is_appended_with_the_source_name_and_colour() {
    let mut app = with_label(("Bug", "red"), &[("Other", "green")]);
    move_to(&mut app, "A", "Dst");
    assert_eq!(labels_of(&app, "A"), [("Bug".into(), "red".into())]);
    assert_eq!(
        board_labels(&app, "Dst"),
        [
            ("Other".into(), "green".into()),
            ("Bug".into(), "red".into())
        ]
    );
}

#[test]
fn undo_shows_the_source_board_and_redo_the_target() {
    let mut app = app_with_two_boards();
    let (src, dst) = (board_id(&app, "Src"), board_id(&app, "Dst"));
    send(&mut app, [Message::OpenBoard(src)]);
    move_to(&mut app, "A", "Dst");
    send(&mut app, [Message::OpenBoard(dst), Message::Undo]);
    assert_eq!(app.current_board, Some(src));
    assert_eq!(first_column(&app, "Src"), ["A", "B", "C"]);
    assert_eq!(first_column(&app, "Dst"), ["X"]);
    send(&mut app, [Message::OpenBoard(src), Message::Redo]);
    assert_eq!(app.current_board, Some(dst));
    assert_eq!(first_column(&app, "Dst"), ["X", "A"]);
}

#[test]
fn the_picker_says_where_the_card_goes() {
    let mut app = app_with_two_boards();
    select(&mut app, "A");
    send(&mut app, [Message::OpenMove]);
    let mut ui = iced_test::simulator(app.view());
    assert!(
        ui.find("The Card goes to the bottom of the Board's first Column.")
            .is_ok()
    );
}
