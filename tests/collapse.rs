//! Collapse a Column: the fold, its file field, the Selection and drops around a strip.
use std::fs;

use kanvibe::app::{App, Direction, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::storage::{self, Settings};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One Board: To do: A, B, C; Doing: X; Done: D.
fn app_with_cards() -> App {
    let mut app = App::default();
    let work = app.data.create_board("Work").unwrap();
    for (column, titles) in [(0, &["A", "B", "C"][..]), (1, &["X"]), (2, &["D"])] {
        let column = column_id(&app, column);
        for title in titles {
            app.data.add_card(column, title);
        }
    }
    app.current_board = Some(work);
    app.history = Default::default();
    app
}

fn column_id(app: &App, index: usize) -> Id {
    app.data.boards[0].columns[index].id
}

fn collapsed(app: &App, index: usize) -> bool {
    app.data.boards[0].columns[index].collapsed
}

#[test]
fn collapse_persists_through_save_and_load_and_is_written_only_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_with_cards();
    let doing = column_id(&app, 1);
    send(&mut app, [Message::ToggleCollapse(doing)]);
    assert!(collapsed(&app, 1));

    storage::save(tmp.path(), &app.data, &Settings::default()).unwrap();
    let text = fs::read_to_string(tmp.path().join("kanvibe.json")).unwrap();
    assert_eq!(text.matches("\"collapsed\": true").count(), 1, "{text}");
    assert!(!text.contains("\"collapsed\": false"));
    let (loaded, _) = storage::load(tmp.path()).unwrap().unwrap();
    assert_eq!(loaded, app.data);
}

fn card_id(app: &App, title: &str) -> Id {
    let mut cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.find(|c| c.title == title).expect("card").id
}

fn titles(app: &App, column: usize) -> Vec<&str> {
    let cards = &app.data.boards[0].columns[column].cards;
    cards.iter().map(|c| c.title.as_str()).collect()
}

fn select(app: &mut App, column: usize, card: Option<&str>) {
    let selection = Selection {
        column: column_id(app, column),
        card: card.map(|t| card_id(app, t)),
    };
    send(app, [Message::Select(selection)]);
}

fn selected(app: &App, column: usize, card: Option<&str>) -> bool {
    app.selection
        == Some(Selection {
            column: column_id(app, column),
            card: card.map(|t| card_id(app, t)),
        })
}

/// The middle Column (Doing: X) collapsed, with no undo history.
fn app_with_doing_collapsed() -> App {
    let mut app = app_with_cards();
    app.data.toggle_collapse(column_id(&app, 1));
    app
}

#[test]
fn left_and_right_land_on_a_collapsed_column_itself_and_up_down_stay_there() {
    let mut app = app_with_doing_collapsed();
    select(&mut app, 0, Some("B"));

    send(&mut app, [Message::MoveSelection(Direction::Right)]);
    assert!(selected(&app, 1, None), "{:?}", app.selection);
    for direction in [
        Direction::Up,
        Direction::Down,
        Direction::Top,
        Direction::Bottom,
    ] {
        send(&mut app, [Message::MoveSelection(direction)]);
        assert!(selected(&app, 1, None), "{direction:?}");
    }
    send(&mut app, [Message::MoveSelection(Direction::Right)]);
    assert!(selected(&app, 2, Some("D")));
    send(&mut app, [Message::MoveSelection(Direction::Left)]);
    assert!(selected(&app, 1, None));
    send(&mut app, [Message::MoveSelection(Direction::Left)]);
    assert!(selected(&app, 0, Some("A")));
}

#[test]
fn collapsing_the_column_of_the_selected_card_keeps_only_the_column_selected() {
    let mut app = app_with_cards();
    select(&mut app, 1, Some("X"));
    let doing = column_id(&app, 1);
    send(&mut app, [Message::ToggleCollapse(doing)]);
    assert!(selected(&app, 1, None));
}

#[test]
fn ctrl_right_into_a_collapsed_column_appends_and_selects_the_column() {
    let mut app = app_with_doing_collapsed();
    select(&mut app, 0, Some("A"));

    send(&mut app, [Message::MoveCard(Direction::Right)]);

    assert_eq!(titles(&app, 0), ["B", "C"]);
    assert_eq!(titles(&app, 1), ["X", "A"]);
    assert!(selected(&app, 1, None));
    // With the Column selected and no Card, Ctrl+Right moves the Column.
    send(&mut app, [Message::MoveCard(Direction::Right)]);
    assert_eq!(titles(&app, 2), ["X", "A"]);
    assert!(selected(&app, 2, None));
}

#[test]
fn ctrl_right_into_a_collapsed_column_appends_below_cards_the_filter_hides() {
    let mut app = app_with_doing_collapsed();
    let doing = column_id(&app, 1);
    app.data.add_card(doing, "Na"); // shown by the Filter, above a hidden Card
    app.data.add_card(doing, "Y");
    send(&mut app, [Message::FilterChanged("a".into())]); // shows A and Na
    select(&mut app, 0, Some("A"));

    send(&mut app, [Message::MoveCard(Direction::Right)]);

    assert_eq!(titles(&app, 1), ["X", "Na", "Y", "A"]);
}

#[test]
fn new_card_on_a_collapsed_column_expands_it_and_opens_the_capture_box() {
    let mut app = app_with_doing_collapsed();
    select(&mut app, 1, None);
    send(&mut app, [Message::NewCard]);
    assert!(!collapsed(&app, 1));
    assert_eq!(
        app.capture.as_ref().map(|c| c.column),
        Some(column_id(&app, 1))
    );
    send(&mut app, [Message::Escape, Message::Undo]);
    assert!(collapsed(&app, 1), "the expand is its own undo step");
}

#[test]
fn collapse_and_expand_are_one_undo_step_each() {
    let mut app = app_with_cards();
    let doing = column_id(&app, 1);
    send(&mut app, [Message::ToggleCollapse(doing)]);
    send(&mut app, [Message::ToggleCollapse(doing)]);
    assert!(!collapsed(&app, 1));
    send(&mut app, [Message::Undo]);
    assert!(collapsed(&app, 1));
    send(&mut app, [Message::Undo]);
    assert!(!collapsed(&app, 1));
    send(&mut app, [Message::Redo]);
    assert!(collapsed(&app, 1));
}

#[test]
fn the_column_menu_offers_collapse_and_closes() {
    let mut app = app_with_cards();
    let doing = column_id(&app, 1);
    send(&mut app, [Message::ToggleColumnMenu(doing)]);
    assert!(iced_test::simulator(app.view()).find("Collapse").is_ok());
    send(&mut app, [Message::ToggleCollapse(doing)]);
    assert_eq!(app.column_menu, None);
}

#[test]
fn a_collapsed_column_shows_its_name_and_count_but_not_its_cards() {
    let mut app = app_with_doing_collapsed();
    let doing = column_id(&app, 1);
    app.data.add_card(doing, "Y");
    app.data.set_limit(doing, "1");
    let mut ui = iced_test::simulator(app.view());
    assert!(ui.find("Doing").is_ok());
    assert!(ui.find("2/1").is_ok());
    assert!(ui.find("X").is_err());
    assert!(ui.find("Y").is_err());
    assert!(ui.find("A").is_ok());
}

#[test]
fn a_click_on_the_strip_expands_the_column_and_a_click_on_a_header_does_not_collapse() {
    use kanvibe::drag::Item;
    let mut app = app_with_doing_collapsed();
    let [todo, doing] = [0, 1].map(|i| column_id(&app, i));
    send(
        &mut app,
        [Message::DragPress(Item::Column(doing)), Message::DragEnd],
    );
    assert!(!collapsed(&app, 1));
    send(
        &mut app,
        [Message::DragPress(Item::Column(todo)), Message::DragEnd],
    );
    assert!(!collapsed(&app, 0));
    send(&mut app, [Message::Undo]);
    assert!(collapsed(&app, 1));
}

#[test]
fn dropping_a_card_on_a_collapsed_column_appends_it() {
    use iced::Point;
    use kanvibe::drag::{Item, Target};
    let mut app = app_with_doing_collapsed();
    let doing = column_id(&app, 1);
    app.data.add_card(doing, "Na");
    app.data.add_card(doing, "Y");
    send(&mut app, [Message::FilterChanged("a".into())]); // shows A and Na
    let a = card_id(&app, "A");

    send(
        &mut app,
        [
            Message::DragPress(Item::Card(a)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(100.0, 140.0)),
            Message::DragHover(Target::Column(doing)),
            Message::DragEnd,
        ],
    );

    assert_eq!(titles(&app, 1), ["X", "Na", "Y", "A"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(titles(&app, 1), ["X", "Na", "Y"]);
}

#[test]
fn a_v1_file_loads_with_every_column_expanded_and_saves_as_version_2() {
    let tmp = tempfile::tempdir().unwrap();
    let json = r#"{ "version": 1, "next_id": 3,
        "boards": [ { "id": 1, "name": "Work", "columns": [ { "id": 2, "name": "To do" } ] } ] }"#;
    fs::write(tmp.path().join("kanvibe.json"), json).unwrap();

    let (data, settings) = storage::load(tmp.path()).unwrap().unwrap();
    assert!(!data.boards[0].columns[0].collapsed);

    storage::save(tmp.path(), &data, &settings).unwrap();
    let text = fs::read_to_string(tmp.path().join("kanvibe.json")).unwrap();
    assert!(text.starts_with("{\n  \"version\": 2,\n"), "{text}");
}
