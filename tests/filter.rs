//! The Filter (#18): the pure predicate, then the Board through Messages sent to `update`.
use iced::Point;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{App, Message, Selection};
use kanvibe::domain::{Card, Id};
use kanvibe::drag::{Item, Target};
use kanvibe::filter::Filter;
use kanvibe::keys::{self, Context, Input};

fn card(title: &str, description: &str, labels: &[Id]) -> Card {
    Card {
        id: 1,
        title: title.into(),
        created: String::new(),
        updated: String::new(),
        checklist: Vec::new(),
        labels: labels.to_vec(),
        description: description.into(),
        files: Vec::new(),
        due: None,
    }
}

fn filter(text: &str, labels: &[Id]) -> Filter {
    Filter {
        text: text.into(),
        labels: labels.to_vec(),
    }
}

#[test]
fn text_matches_titles_and_descriptions_ignoring_case() {
    let f = filter("  bug ", &[]);
    assert!(f.matches(&card("Fix the BUG", "", &[]), &[]));
    assert!(f.matches(&card("Login", "a nasty Bug here", &[]), &[]));
    assert!(!f.matches(&card("Login", "works", &[]), &[]));
}

#[test]
fn labels_match_any_of_and_combine_with_the_text() {
    let f = filter("", &[7, 8]);
    assert!(f.matches(&card("A", "", &[8]), &[]));
    assert!(!f.matches(&card("A", "", &[9]), &[]));
    assert!(!f.matches(&card("A", "", &[]), &[]));
    let f = filter("login", &[7]);
    assert!(f.matches(&card("Login", "", &[7]), &[]));
    assert!(!f.matches(&card("Login", "", &[]), &[]));
    assert!(!f.matches(&card("Logout", "", &[7]), &[]));
}

#[test]
fn an_empty_filter_is_off_and_matches_everything() {
    let f = filter("   ", &[]);
    assert!(!f.is_on());
    assert!(f.matches(&card("anything", "", &[]), &[]));
    assert!(filter("x", &[]).is_on());
    assert!(filter("", &[1]).is_on());
}

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One Board "Work" with `titles[c]` in Column `c` (To do, Doing, Done); nothing selected.
fn app_with(titles: &[&[&str]]) -> App {
    let mut app = App::default();
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    for (c, titles) in titles.iter().enumerate() {
        let open = Message::OpenCapture(column(&app, c));
        send(&mut app, [open]);
        for title in *titles {
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

fn id(app: &App, title: &str) -> Id {
    let mut cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.find(|c| c.title == title).expect("card").id
}

fn column(app: &App, index: usize) -> Id {
    app.data.boards[0].columns[index].id
}

fn titles(app: &App, index: usize) -> Vec<&str> {
    let cards = &app.data.boards[0].columns[index].cards;
    cards.iter().map(|c| c.title.as_str()).collect()
}

/// The titles shown in Column `index`, top to bottom.
fn shown(app: &App, index: usize) -> Vec<&str> {
    let cards = &app.data.boards[0].columns[index].cards;
    cards
        .iter()
        .filter(|c| app.shown(c))
        .map(|c| c.title.as_str())
        .collect()
}

fn selected(app: &App) -> Option<&str> {
    let card = app.selection?.card?;
    let mut cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    Some(cards.find(|c| c.id == card)?.title.as_str())
}

fn select(app: &mut App, title: &str) {
    let card = id(app, title);
    let columns = &app.data.boards[0].columns;
    let column = columns
        .iter()
        .find(|c| c.cards.iter().any(|x| x.id == card))
        .expect("on the Board")
        .id;
    app.selection = Some(Selection {
        column,
        card: Some(card),
    });
}

#[test]
fn typing_in_the_filter_box_hides_cards_that_do_not_match() {
    let mut app = app_with(&[&["bug one", "feature", "bug two"], &["other"]]);
    send(&mut app, [Message::FilterChanged("BUG".into())]);
    assert_eq!(shown(&app, 0), ["bug one", "bug two"]);
    assert!(shown(&app, 1).is_empty());
}

#[test]
fn the_selected_card_and_cards_just_created_stay_visible() {
    let mut app = app_with(&[&["bug one", "feature"]]);
    select(&mut app, "feature");
    send(&mut app, [Message::FilterChanged("bug".into())]);
    assert_eq!(shown(&app, 0), ["bug one", "feature"]);
    let todo = column(&app, 0);
    send(&mut app, [Message::OpenCapture(todo)]);
    for title in ["new one", "new two"] {
        send(
            &mut app,
            [
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
            ],
        );
    }
    send(&mut app, [Message::Escape]);
    assert_eq!(shown(&app, 0), ["bug one", "new one", "new two"]);
}

/// Adds Label `name` to the Board and attaches it to the Cards `on`.
fn label(app: &mut App, name: &str, on: &[&str]) -> Id {
    let board = app.data.boards[0].id;
    let label = app
        .data
        .add_label(board, name, kanvibe::labels::PALETTE[0].0)
        .expect("label");
    for title in on {
        let card = id(app, title);
        app.data.toggle_label(card, label);
    }
    label
}

#[test]
fn the_labels_filter_shows_cards_with_any_chosen_label_combined_with_the_text() {
    let mut app = app_with(&[&["A", "B", "C", "D"]]);
    let red = label(&mut app, "Red", &["A", "C"]);
    let blue = label(&mut app, "Blue", &["B"]);
    send(&mut app, [Message::FilterLabel(red)]);
    assert_eq!(shown(&app, 0), ["A", "C"]);
    send(&mut app, [Message::FilterLabel(blue)]);
    assert_eq!(shown(&app, 0), ["A", "B", "C"]);
    send(&mut app, [Message::FilterChanged("c".into())]);
    assert_eq!(shown(&app, 0), ["C"]);
    send(&mut app, [Message::FilterLabel(red)]); // unticks Red
    assert!(shown(&app, 0).is_empty());
    assert_eq!(app.filter.labels, [blue]);
}

#[test]
fn column_headers_count_visible_of_total_and_the_limit_is_judged_on_the_total() {
    let mut app = app_with(&[&["bug 1", "x", "bug 2", "y", "z"], &["a"]]);
    let todo = column(&app, 0);
    app.data.set_limit(todo, "4");
    let col = |app: &App, i: usize| app.data.boards[0].columns[i].clone();
    assert_eq!(app.count_label(&col(&app, 0)), "5/4");
    send(&mut app, [Message::FilterChanged("bug".into())]);
    assert_eq!(app.count_label(&col(&app, 0)), "2 of 5/4");
    assert_eq!(app.count_label(&col(&app, 1)), "0 of 1");
    assert!(col(&app, 0).over_limit());
}

#[test]
fn arrow_keys_skip_hidden_cards() {
    use kanvibe::app::Direction::*;
    let mut app = app_with(&[&["x", "bug 1", "y", "bug 2"], &["z", "bug 3"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    let mut moves = vec![];
    for direction in [Down, Down, Up, Down, Right, Up, Left] {
        send(&mut app, [Message::MoveSelection(direction)]);
        moves.push(selected(&app).expect("a Card").to_string());
    }
    // No selection: the first visible Card. Right keeps the visible position, clamped.
    let expected = [
        "bug 1", "bug 2", "bug 1", "bug 2", "bug 3", "bug 3", "bug 1",
    ];
    assert_eq!(moves, expected);
}

#[test]
fn ctrl_up_down_jump_over_hidden_cards_to_the_next_visible_one() {
    use kanvibe::app::Direction::*;
    let mut app = app_with(&[&["bug 1", "h1", "bug 2", "h2", "bug 3"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    select(&mut app, "bug 3");
    let steps: [(_, &[&str]); 5] = [
        (Up, &["bug 1", "h1", "bug 3", "bug 2", "h2"]),
        (Up, &["bug 3", "bug 1", "h1", "bug 2", "h2"]),
        (Down, &["bug 1", "bug 3", "h1", "bug 2", "h2"]),
        (Down, &["bug 1", "h1", "bug 2", "bug 3", "h2"]),
        (Down, &["bug 1", "h1", "bug 2", "bug 3", "h2"]), // no visible Card below
    ];
    for (direction, expected) in steps {
        send(&mut app, [Message::MoveCard(direction)]);
        assert_eq!(titles(&app, 0), expected, "after {direction:?}");
    }
    assert_eq!(selected(&app), Some("bug 3"));
}

/// Drags Card `title` onto `target` and releases.
fn drop_on(app: &mut App, title: &str, target: Target) {
    let card = id(app, title);
    send(
        app,
        [
            Message::DragPress(Item::Card(card)),
            Message::DragMoved(Point::new(100.0, 100.0)),
            Message::DragMoved(Point::new(100.0, 140.0)),
            Message::DragHover(target),
            Message::DragEnd,
        ],
    );
}

#[test]
fn a_dropped_card_lands_right_after_the_visible_card_above_the_slot() {
    let mut app = app_with(&[
        &["bug 1", "h1", "bug 2", "h2", "bug 3"],
        &["h3", "bug 4", "h4"],
    ]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    let (todo, doing) = (column(&app, 0), column(&app, 1));
    // The slot before "bug 2": below "bug 1", so right after it.
    drop_on(
        &mut app,
        "bug 3",
        Target::Card {
            column: todo,
            index: 2,
        },
    );
    assert_eq!(titles(&app, 0), ["bug 1", "bug 3", "h1", "bug 2", "h2"]);
    // The top slot of Doing (before "bug 4", the first visible Card): right before it.
    drop_on(
        &mut app,
        "bug 1",
        Target::Card {
            column: doing,
            index: 1,
        },
    );
    assert_eq!(titles(&app, 1), ["h3", "bug 1", "bug 4", "h4"]);
    // The Column body, below every Card: right after the last visible one.
    drop_on(&mut app, "bug 2", Target::Column(doing));
    assert_eq!(titles(&app, 1), ["h3", "bug 1", "bug 4", "bug 2", "h4"]);
    assert_eq!(titles(&app, 0), ["bug 3", "h1", "h2"]);
}

fn press(key: Key, modifiers: Modifiers) -> keyboard::Event {
    use keyboard::key::{Code, Physical};
    let code = if key == Key::Character("f".into()) {
        Code::KeyF
    } else {
        Code::F24
    };
    keyboard::Event::KeyPressed {
        modified_key: key.clone(),
        key,
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    }
}

#[test]
fn ctrl_f_focuses_the_filter_even_while_typing_and_the_filter_box_keeps_arrows() {
    let ctrl_f = press(Key::Character("f".into()), Modifiers::CTRL);
    let in_filter = Context {
        focused: Some(Input::Filter),
        ..Context::default()
    };
    for context in [Context::default(), in_filter.clone()] {
        assert_eq!(keys::map(&ctrl_f, &context), Some(Message::FocusFilter));
    }
    let down = press(Key::Named(Named::ArrowDown), Modifiers::empty());
    assert_eq!(keys::map(&down, &in_filter), None);
    let f = press(Key::Character("f".into()), Modifiers::empty());
    assert_eq!(keys::map(&f, &Context::default()), None);
}

#[test]
fn enter_in_the_filter_box_selects_the_first_visible_card() {
    let mut app = app_with(&[&["x"], &["y", "bug 2"], &["bug 3"]]);
    send(
        &mut app,
        [Message::FilterChanged("bug".into()), Message::SubmitFilter],
    );
    assert_eq!(selected(&app), Some("bug 2"));
    assert_eq!(app.open_card(), None);
}

#[test]
fn enter_in_the_filter_box_skips_collapsed_columns() {
    let mut app = app_with(&[&["x"], &["y", "bug 2"], &["bug 3"]]);
    let doing = column(&app, 1);
    send(&mut app, [Message::ToggleCollapse(doing)]);
    send(
        &mut app,
        [Message::FilterChanged("bug".into()), Message::SubmitFilter],
    );
    assert_eq!(selected(&app), Some("bug 3"));
}

#[test]
fn esc_closes_the_open_card_then_clears_the_filter_then_the_selection() {
    let mut app = app_with(&[&["bug 1", "x"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    let (todo, bug) = (column(&app, 0), id(&app, "bug 1"));
    send(
        &mut app,
        [
            Message::Select(Selection {
                column: todo,
                card: Some(bug),
            }),
            Message::OpenCard,
        ],
    );
    // Esc the filter box captured: it only leaves the box.
    send(&mut app, [Message::LeaveField]);
    assert_eq!((app.open_card(), app.filter.is_on()), (Some(bug), true));
    send(&mut app, [Message::Escape]);
    assert_eq!((app.open_card(), app.filter.is_on()), (None, true));
    send(&mut app, [Message::Escape]);
    assert!(!app.filter.is_on());
    assert_eq!(selected(&app), Some("bug 1"));
    send(&mut app, [Message::Escape]);
    assert_eq!(app.selection, None);
}

#[test]
fn a_captured_esc_in_another_field_still_closes_that_field() {
    let mut app = app_with(&[&["A"]]);
    let todo = column(&app, 0);
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("draft".into()),
            Message::LeaveField,
        ],
    );
    assert_eq!(app.capture, None);
    assert_eq!(titles(&app, 0), ["A"]);
}

#[test]
fn clear_filter_and_a_board_switch_clear_the_filter() {
    let mut app = app_with(&[&["bug 1", "x"]]);
    let red = label(&mut app, "Red", &["x"]);
    send(
        &mut app,
        [Message::FilterChanged("bug".into()), Message::ClearFilter],
    );
    assert_eq!(shown(&app, 0), ["bug 1", "x"]);
    send(
        &mut app,
        [
            Message::FilterChanged("bug".into()),
            Message::FilterLabel(red),
            Message::ToggleFilterMenu,
            Message::NewBoard,
            Message::BoardNameChanged("Home".into()),
            Message::CreateBoard,
        ],
    );
    assert_eq!(app.filter, Filter::default());
    assert!(!app.filter_menu);
    let work = app.data.boards[0].id;
    send(&mut app, [Message::OpenBoard(work)]);
    assert_eq!(shown(&app, 0), ["bug 1", "x"]);
}

#[test]
fn no_cards_match_only_when_the_filter_is_on_and_no_card_on_the_board_matches() {
    let mut app = app_with(&[&["bug 1"], &["x"]]);
    assert!(!app.nothing_matches());
    send(&mut app, [Message::FilterChanged("bug".into())]);
    assert!(!app.nothing_matches());
    send(&mut app, [Message::FilterChanged("nope".into())]);
    assert!(app.nothing_matches());
}

#[test]
fn filter_labels_that_no_longer_exist_are_dropped() {
    let mut app = app_with(&[&["A", "B"]]);
    let red = label(&mut app, "Red", &["A"]);
    send(
        &mut app,
        [Message::FilterLabel(red), Message::RemoveLabel(red)],
    );
    assert!(app.filter.labels.is_empty());
    assert_eq!(shown(&app, 0), ["A", "B"]);
}

#[test]
fn filter_text_also_matches_label_names() {
    let mut app = app_with(&[&["A", "B", "C"]]);
    label(&mut app, "Urgent", &["B"]);
    send(&mut app, [Message::FilterChanged(" urg ".into())]);
    assert_eq!(shown(&app, 0), ["B"]);
    assert!(!app.nothing_matches());
}

#[test]
fn ctrl_left_right_under_a_filter_lands_after_the_last_visible_card() {
    use kanvibe::app::Direction::*;
    let mut app = app_with(&[&["bug 1"], &["bug 2", "h1", "h2"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    select(&mut app, "bug 1");
    send(&mut app, [Message::MoveCard(Right)]);
    assert_eq!(titles(&app, 1), ["bug 2", "bug 1", "h1", "h2"]); // as a drop on the Column body
    send(&mut app, [Message::MoveCard(Left)]);
    assert_eq!(titles(&app, 0), ["bug 1"]);
    // No Filter: the bottom of the Column, as before.
    send(&mut app, [Message::ClearFilter]);
    select(&mut app, "bug 1");
    send(&mut app, [Message::MoveCard(Right)]);
    assert_eq!(titles(&app, 1), ["bug 2", "h1", "h2", "bug 1"]);
}

#[test]
fn home_and_end_select_the_first_and_last_visible_card() {
    use kanvibe::app::Direction::*;
    let mut app = app_with(&[&["x", "bug 1", "y", "bug 2", "z"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    send(&mut app, [Message::MoveSelection(Bottom)]);
    assert_eq!(
        selected(&app),
        Some("bug 1"),
        "no selection: the first visible Card"
    );
    send(&mut app, [Message::MoveSelection(Bottom)]);
    assert_eq!(selected(&app), Some("bug 2"));
    send(&mut app, [Message::MoveSelection(Top)]);
    assert_eq!(selected(&app), Some("bug 1"));
}

#[test]
fn ctrl_home_end_move_the_card_past_the_other_visible_cards() {
    use kanvibe::app::Direction::*;
    let mut app = app_with(&[&["h0", "bug 1", "h1", "bug 2", "bug 3", "h2"]]);
    send(&mut app, [Message::FilterChanged("bug".into())]);
    select(&mut app, "bug 2");
    send(&mut app, [Message::MoveCard(Top)]);
    assert_eq!(
        titles(&app, 0),
        ["h0", "bug 2", "bug 1", "h1", "bug 3", "h2"]
    );
    send(&mut app, [Message::MoveCard(Bottom)]);
    assert_eq!(
        titles(&app, 0),
        ["h0", "bug 1", "h1", "bug 3", "bug 2", "h2"]
    );
    send(&mut app, [Message::MoveCard(Bottom)]);
    let same = ["h0", "bug 1", "h1", "bug 3", "bug 2", "h2"];
    assert_eq!(titles(&app, 0), same, "already the last visible Card");
    assert_eq!(selected(&app), Some("bug 2"));
}
