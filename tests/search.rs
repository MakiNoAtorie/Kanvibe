//! Search (spec v2 item 2): the hit function over all Boards, and the Search view through `update`.
use kanvibe::app::{App, Direction, Message, Selection};
use kanvibe::domain::{Data, Id};
use kanvibe::search;

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Work: To do "Plan sprint", "Lunch"; Doing "Fix sink" (description "call a plumber");
/// Archive "Old plan". Home: Done "Groceries" (Label "Plants"). Attic (archived): To do
/// "Plaster wall".
fn data() -> Data {
    let mut data = Data::default();
    let work = data.create_board("Work").unwrap();
    let home = data.create_board("Home").unwrap();
    let attic = data.create_board("Attic").unwrap();
    let column = |data: &Data, board: usize, i: usize| data.boards[board].columns[i].id;
    let (todo, doing) = (column(&data, 0, 0), column(&data, 0, 1));
    data.add_card(todo, "Plan sprint").unwrap();
    data.add_card(todo, "Lunch").unwrap();
    let sink = data.add_card(doing, "Fix sink").unwrap();
    data.set_description(sink, "call a plumber").unwrap();
    let old = data.add_card(todo, "Old plan").unwrap();
    data.archive_card(old).unwrap();
    let groceries = data.add_card(column(&data, 1, 2), "Groceries").unwrap();
    let plants = data.add_label(home, "Plants", "blue").unwrap();
    data.toggle_label(groceries, plants).unwrap();
    data.add_card(column(&data, 2, 0), "Plaster wall").unwrap();
    data.archive_board(attic).unwrap();
    let _ = work;
    data
}

/// (Board name, Board archived?, Card title, Column name or None for the Archive).
fn found(data: &Data, text: &str) -> Vec<(String, bool, String, Option<String>)> {
    let hits = search::hits(data, text);
    hits.iter()
        .map(|h| {
            (
                h.board.name.clone(),
                h.board.archived,
                h.card.title.clone(),
                h.column.map(String::from),
            )
        })
        .collect()
}

fn hit(
    board: &str,
    archived: bool,
    card: &str,
    column: Option<&str>,
) -> (String, bool, String, Option<String>) {
    (
        board.into(),
        archived,
        card.into(),
        column.map(String::from),
    )
}

#[test]
fn fewer_than_two_trimmed_characters_find_nothing() {
    let data = data();
    assert!(found(&data, "").is_empty());
    assert!(found(&data, "p").is_empty());
    assert!(found(&data, "  p  ").is_empty());
}

#[test]
fn two_characters_find_titles_descriptions_and_labels_on_every_board_in_order() {
    let data = data();
    assert_eq!(
        found(&data, " PL "),
        vec![
            hit("Work", false, "Plan sprint", Some("To do")),
            hit("Work", false, "Fix sink", Some("Doing")),
            hit("Work", false, "Old plan", None),
            hit("Home", false, "Groceries", Some("Done")),
            hit("Attic", true, "Plaster wall", Some("To do")),
        ]
    );
    assert_eq!(
        found(&data, "lunch"),
        vec![hit("Work", false, "Lunch", Some("To do"))]
    );
}

/// The App on Work with the fixture data.
fn app() -> App {
    let mut app = App::default();
    app.data = data();
    app.current_board = Some(app.data.boards[0].id);
    app
}

fn card(app: &App, title: &str) -> Id {
    let boards = app.data.boards.iter();
    let cards = boards.flat_map(|b| {
        let active = b.columns.iter().flat_map(|c| &c.cards);
        active.chain(b.archived_cards.iter().map(|a| &a.card))
    });
    cards.into_iter().find(|c| c.title == title).unwrap().id
}

fn search_text(app: &App) -> Option<&str> {
    app.search.as_ref().map(|s| s.text.as_str())
}

#[test]
fn search_opens_on_the_current_board_and_takes_text() {
    let mut app = app();
    send(
        &mut app,
        [Message::OpenSearch, Message::SearchChanged("pl".into())],
    );
    assert_eq!(search_text(&app), Some("pl"));
    assert_eq!(app.current_board, Some(app.data.boards[0].id));
}

#[test]
fn opening_search_commits_the_pending_edit() {
    let mut app = app();
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("Typed".into()),
            Message::OpenSearch,
        ],
    );
    assert!(app.capture.is_none());
    let titles: Vec<_> = app.data.boards[0].columns[0]
        .cards
        .iter()
        .map(|c| &c.title)
        .collect();
    assert_eq!(titles, ["Plan sprint", "Lunch", "Typed"]);
}

#[test]
fn picking_a_hit_on_another_board_opens_it_on_that_card() {
    let mut app = app();
    let groceries = card(&app, "Groceries");
    let home = app.data.boards[1].clone();
    send(
        &mut app,
        [
            Message::OpenSearch,
            Message::SearchChanged("pl".into()),
            Message::SearchPick(groceries),
        ],
    );
    assert!(app.search.is_none());
    assert_eq!(app.current_board, Some(home.id));
    let selection = Selection {
        column: home.columns[2].id,
        card: Some(groceries),
    };
    assert_eq!(app.selection, Some(selection));
    assert_eq!(app.open_card(), Some(groceries));
}

#[test]
fn picking_a_hit_in_a_collapsed_column_expands_it_as_one_undo_step() {
    let mut app = app();
    let groceries = card(&app, "Groceries");
    let done = app.data.boards[1].columns[2].id;
    app.data.toggle_collapse(done);
    send(
        &mut app,
        [
            Message::OpenSearch,
            Message::SearchChanged("groc".into()),
            Message::SearchPick(groceries),
        ],
    );
    assert!(!app.data.boards[1].columns[2].collapsed);
    assert_eq!(app.open_card(), Some(groceries));
    send(&mut app, [Message::Undo]);
    assert!(app.data.boards[1].columns[2].collapsed);
}

#[test]
fn picking_an_archived_card_opens_its_boards_archive_view() {
    let mut app = app();
    let home = app.data.boards[1].id;
    app.current_board = Some(home);
    let old = card(&app, "Old plan");
    send(
        &mut app,
        [
            Message::OpenSearch,
            Message::SearchChanged("old".into()),
            Message::SearchPick(old),
        ],
    );
    let work = app.data.boards[0].id;
    assert!(app.search.is_none());
    assert_eq!(app.current_board, Some(work));
    assert_eq!(app.archive, Some(work));
}

#[test]
fn a_hit_on_an_archived_board_is_inert() {
    let mut app = app();
    let wall = card(&app, "Plaster wall");
    let work = app.current_board;
    send(
        &mut app,
        [
            Message::OpenSearch,
            Message::SearchChanged("wall".into()),
            Message::SearchPick(wall),
        ],
    );
    assert_eq!(search_text(&app), Some("wall"));
    assert_eq!(app.current_board, work);
}

#[test]
fn esc_returns_to_the_board_and_forgets_the_text() {
    let mut app = app();
    let home = app.data.boards[1].id;
    app.current_board = Some(home);
    send(
        &mut app,
        [
            Message::OpenSearch,
            Message::SearchChanged("pl".into()),
            Message::Escape,
        ],
    );
    assert!(app.search.is_none());
    assert_eq!(app.current_board, Some(home));
    send(&mut app, [Message::OpenSearch]);
    assert_eq!(search_text(&app), Some(""));
    send(&mut app, [Message::CloseSearch]);
    assert!(app.search.is_none());
    // Esc captured by the focused search box arrives as LeaveField.
    send(&mut app, [Message::OpenSearch, Message::LeaveField]);
    assert!(app.search.is_none());
}

#[test]
fn the_view_builds_with_hits_from_every_kind_of_place() {
    let mut app = app();
    send(
        &mut app,
        [Message::OpenSearch, Message::SearchChanged("pl".into())],
    );
    let _ = app.view();
}

#[test]
fn up_and_down_move_the_highlight_within_the_hits() {
    let mut app = app();
    send(
        &mut app,
        [Message::OpenSearch, Message::SearchChanged("pl".into())],
    );
    let highlight = |app: &App| app.search.as_ref().unwrap().highlight;
    send(&mut app, [Message::SearchMove(Direction::Up)]);
    assert_eq!(highlight(&app), 0);
    send(
        &mut app,
        std::iter::repeat_n(Message::SearchMove(Direction::Down), 9),
    );
    assert_eq!(highlight(&app), 4); // five hits
    send(&mut app, [Message::SearchMove(Direction::Up)]);
    assert_eq!(highlight(&app), 3);
    send(&mut app, [Message::SearchChanged("pla".into())]);
    assert_eq!(highlight(&app), 0);
}
