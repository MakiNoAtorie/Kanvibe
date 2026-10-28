//! Seam 1 for the open Card (v2 item 9): open/close/follow, title edit, timestamps.
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{App, Direction, Field, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::keys::{self, Context};

const OLD: &str = "2020-01-01T00:00:00Z";

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// To do: A, B, C; Doing: empty; Done: D. All timestamps set to `OLD`.
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
    let (todo, done) = (
        app.data.boards[0].columns[0].id,
        app.data.boards[0].columns[2].id,
    );
    for (column, title) in [(todo, "A"), (todo, "B"), (todo, "C"), (done, "D")] {
        send(
            &mut app,
            [
                Message::OpenCapture(column),
                Message::CaptureChanged(title.into()),
                Message::SubmitCapture,
                Message::Escape,
            ],
        );
    }
    for card in app.data.boards[0]
        .columns
        .iter_mut()
        .flat_map(|c| &mut c.cards)
    {
        card.created = OLD.into();
        card.updated = OLD.into();
    }
    app.selection = None;
    app.history = Default::default(); // the edited timestamps are the starting point
    app
}

fn card(app: &App, title: &str) -> Id {
    let cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.clone().find(|c| c.title == title).unwrap().id
}

fn updated(app: &App, title: &str) -> String {
    let cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards
        .clone()
        .find(|c| c.title == title)
        .unwrap()
        .updated
        .clone()
}

fn select(app: &mut App, title: &str) {
    let id = card(app, title);
    let column = app.data.boards[0]
        .columns
        .iter()
        .find(|c| c.cards.iter().any(|x| x.id == id))
        .unwrap()
        .id;
    send(
        app,
        [Message::Select(Selection {
            column,
            card: Some(id),
        })],
    );
}

fn open(app: &mut App, title: &str) {
    select(app, title);
    send(app, [Message::OpenCard]);
}

fn press(key: Named) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        modified_key: Key::Named(key),
        key: Key::Named(key),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

#[test]
fn clicking_a_card_only_selects_it() {
    let mut app = app_with_cards();
    select(&mut app, "A");
    assert_eq!(app.selection.and_then(|s| s.card), Some(card(&app, "A")));
    assert_eq!(app.open_card(), None);
}

#[test]
fn a_click_inside_the_open_card_keeps_it_open() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    select(&mut app, "A");
    assert_eq!(app.open_card(), Some(card(&app, "A")));
}

#[test]
fn enter_opens_the_selected_card() {
    let mut app = app_with_cards();
    let message = keys::map(&press(Named::Enter), &Context::default());
    assert_eq!(message, Some(Message::OpenCard));

    send(&mut app, [Message::OpenCard]);
    assert!(!app.open, "nothing selected");

    send(&mut app, [Message::MoveSelection(Direction::Down)]);
    assert!(!app.open, "selecting alone does not open it");
    send(&mut app, [Message::OpenCard]);
    assert!(app.open);
    assert_eq!(app.open_card(), Some(card(&app, "A")));
    assert_eq!(app.editing, None, "opening edits nothing");
}

#[test]
fn the_open_card_shows_all_its_fields_in_its_column() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    let mut ui = iced_test::simulator(app.view());
    for shown in [
        "+ Add Label",
        "Description",
        "+ Add Checklist Item",
        "Move to Board…",
        "Archive",
    ] {
        assert!(ui.find(shown).is_ok(), "{shown}");
    }
}

#[test]
fn double_clicking_a_card_edits_its_title_and_tab_moves_to_the_description() {
    let mut app = app_with_cards();
    let b = card(&app, "B");
    // A double-click's events: press, release, press, double-click, release.
    let item = kanvibe::drag::Item::Card(b);
    send(
        &mut app,
        [
            Message::DragPress(item),
            Message::DragEnd,
            Message::DragPress(item),
            Message::EditCard(b),
            Message::DragEnd,
        ],
    );
    assert_eq!(app.open_card(), Some(b));
    assert_eq!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::CardTitle(b)),
        "the second release does not commit the edit"
    );
    let title = Context {
        focused: Some(keys::Input::CardTitle),
        dragging: false,
    };
    let message = keys::map(&press(Named::Tab), &title);
    assert_eq!(message, Some(Message::EditDescription));
    send(
        &mut app,
        [
            Message::EditChanged("Beta".into()),
            Message::EditDescription,
        ],
    );
    assert_eq!(app.data.name(b), Some("Beta"));
    assert_eq!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::Description(b))
    );

    let description = Context {
        focused: Some(keys::Input::CardDescription),
        dragging: false,
    };
    let shift_tab = keyboard::Event::KeyPressed {
        modified_key: Key::Named(Named::Tab),
        key: Key::Named(Named::Tab),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Tab),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::SHIFT,
        text: None,
        repeat: false,
    };
    assert_eq!(
        keys::map(&shift_tab, &description),
        Some(Message::EditTitle)
    );
    send(&mut app, [Message::EditTitle]);
    assert_eq!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::CardTitle(b)),
        "Shift+Tab goes back to the title"
    );

    // A title is edited on its Card face, so editing one selects that Card.
    let a = card(&app, "A");
    send(&mut app, [Message::StartEdit(Field::CardTitle(a))]);
    assert_eq!(app.selection.and_then(|s| s.card), Some(a));
}

#[test]
fn arrows_and_clicks_on_other_cards_close_the_open_card() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(&mut app, [Message::MoveSelection(Direction::Down)]);
    assert_eq!(app.selection.and_then(|s| s.card), Some(card(&app, "B")));
    assert_eq!(app.open_card(), None, "navigating shows faces");
    open(&mut app, "B");
    select(&mut app, "D");
    assert_eq!(app.open_card(), None);
}

#[test]
fn clicking_a_column_body_closes_the_open_card() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    let doing = app.data.boards[0].columns[1].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column: doing,
            card: None,
        })],
    );
    assert!(!app.open);
    send(&mut app, [Message::MoveSelection(Direction::Left)]);
    assert_eq!(app.open_card(), None, "the next Card shows its face");
}

#[test]
fn a_click_on_the_background_closes_the_open_card() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    let a = card(&app, "A");
    send(&mut app, [Message::StartEdit(Field::CardTitle(a))]);
    send(&mut app, [Message::ClickOutside]);
    assert!(app.open, "a rename keeps the Card open");
    send(&mut app, [Message::Escape, Message::ClickOutside]);
    assert_eq!(app.open_card(), None);
    assert!(app.selection.is_some(), "the Card stays selected");
}

#[test]
fn an_arrow_that_moves_nothing_keeps_the_card_open() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(&mut app, [Message::MoveSelection(Direction::Up)]);
    assert_eq!(app.open_card(), Some(card(&app, "A")));
}

#[test]
fn a_board_switch_closes_it() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Home".into()),
            Message::CreateBoard,
        ],
    );
    let work = app.data.boards[0].id;
    send(&mut app, [Message::OpenBoard(work)]);
    assert!(!app.open);
}

#[test]
fn archiving_the_open_card_closes_it() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(&mut app, [Message::ArchiveSelected]);
    assert_eq!(app.selection.and_then(|s| s.card), Some(card(&app, "B")));
    assert!(!app.open, "the neighbour is selected, not opened");
}

#[test]
fn escape_closes_it_then_clears_the_selection() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(&mut app, [Message::Escape]);
    assert_eq!(app.open_card(), None);
    assert!(app.selection.is_some());
    send(&mut app, [Message::Escape]);
    assert_eq!(app.selection, None);
}

#[test]
fn the_x_button_closes_it_and_keeps_the_selection() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(&mut app, [Message::CloseCard]);
    assert_eq!(app.open_card(), None);
    assert!(app.selection.is_some());
}

#[test]
fn a_card_leaving_the_board_closes_it_and_clears_the_selection() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    let id = card(&app, "A");
    app.data.boards[0].columns[0].cards.retain(|c| c.id != id);
    send(&mut app, [Message::CommitEdit]); // any update validates
    assert_eq!((app.open_card(), app.selection), (None, None));
}

#[test]
fn undoing_the_creation_of_the_open_card_closes_it() {
    let mut app = app_with_cards();
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("E".into()),
            Message::SubmitCapture,
            Message::Escape,
        ],
    );
    send(&mut app, [Message::OpenCard]);
    assert_eq!(app.open_card(), Some(card(&app, "E")));

    send(&mut app, [Message::Undo]);
    assert_eq!((app.open_card(), app.selection), (None, None));
}

#[test]
fn f2_edits_the_open_cards_title_and_blank_reverts() {
    let mut app = app_with_cards();
    let message = keys::map(&press(Named::F2), &Context::default());
    assert_eq!(message, Some(Message::EditTitle));

    open(&mut app, "A");
    let a = card(&app, "A");
    send(&mut app, [Message::EditTitle]);
    assert_eq!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::CardTitle(a))
    );

    send(
        &mut app,
        [Message::EditChanged("Alpha".into()), Message::CommitEdit],
    );
    assert_eq!(app.data.name(a), Some("Alpha"));

    send(
        &mut app,
        [
            Message::EditTitle,
            Message::EditChanged("  ".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(app.data.name(a), Some("Alpha"));
}

#[test]
fn editing_a_title_updates_only_that_cards_timestamp() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(
        &mut app,
        [Message::EditTitle, Message::EditChanged("Alpha".into())],
    );
    send(&mut app, [Message::CommitEdit]);

    assert_ne!(updated(&app, "Alpha"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
    assert_eq!(updated(&app, "C"), OLD);
}

#[test]
fn an_edit_that_changes_nothing_leaves_updated_alone() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    send(
        &mut app,
        [
            Message::EditTitle,
            Message::EditChanged("x".into()),
            Message::EditChanged("A".into()),
            Message::CommitEdit,
            Message::EditTitle,
            Message::EditChanged("".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(updated(&app, "A"), OLD);
}

#[test]
fn moving_touches_updated_but_neighbours_shifting_does_not() {
    let mut app = app_with_cards();
    open(&mut app, "C");
    send(&mut app, [Message::MoveCard(Direction::Up)]); // C above B, same Column
    assert_eq!(updated(&app, "A"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
    assert_ne!(updated(&app, "C"), OLD);

    send(&mut app, [Message::MoveCard(Direction::Right)]); // C into Doing
    assert_ne!(updated(&app, "C"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
}

#[test]
fn a_pending_title_edit_commits_to_its_own_card_before_another_opens() {
    let mut app = app_with_cards();
    open(&mut app, "A");
    let a = card(&app, "A");
    send(
        &mut app,
        [Message::EditTitle, Message::EditChanged("Alpha".into())],
    );

    open(&mut app, "B");
    assert_eq!(app.editing, None);
    assert_eq!(app.open_card(), Some(card(&app, "B")));
    assert_eq!(app.data.name(a), Some("Alpha"));
    assert_eq!(app.data.name(card(&app, "B")), Some("B"));
    assert_ne!(updated(&app, "Alpha"), OLD);
    assert_eq!(updated(&app, "B"), OLD);

    // One undo step for the edit.
    send(&mut app, [Message::Undo]);
    assert_eq!(app.data.name(a), Some("A"));
    assert_eq!(updated(&app, "A"), OLD);
}

#[test]
fn new_cards_get_equal_rfc_3339_utc_second_precision_timestamps() {
    let mut app = App::default();
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(todo),
            Message::CaptureChanged("A".into()),
            Message::SubmitCapture,
        ],
    );
    let c = &app.data.boards[0].columns[0].cards[0];
    assert_eq!(c.created, c.updated);
    assert_eq!(c.created.len(), "2020-01-01T00:00:00Z".len());
    assert!(c.created.parse::<jiff::Timestamp>().is_ok());
    assert!(c.created.ends_with('Z'));
}

#[test]
fn timestamps_show_in_local_time() {
    let tz = jiff::tz::TimeZone::fixed(jiff::tz::offset(-3));
    assert_eq!(
        kanvibe::app::local_time("2026-10-27T15:04:05Z", &tz),
        "2026-10-27 12:04"
    );
    assert_eq!(kanvibe::app::local_time("garbage", &tz), "garbage");
}

#[test]
fn f2_e_l_and_m_act_on_the_open_or_selected_card() {
    use kanvibe::app::PendingEdit;
    let mut app = app_with_cards();
    open(&mut app, "B");
    send(&mut app, [Message::Escape]); // closes the Card, keeps the selection
    assert_eq!(app.open_card(), None);
    let b = card(&app, "B");
    send(&mut app, [Message::EditTitle]);
    assert_eq!(app.open_card(), Some(b));
    assert!(matches!(
        app.editing,
        Some(PendingEdit {
            field: Field::CardTitle(id),
            ..
        }) if id == b
    ));
    send(&mut app, [Message::Escape, Message::Escape]);
    send(&mut app, [Message::EditDescription]);
    assert_eq!(app.open_card(), Some(b));
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::Description(id)) if id == b
    ));
    send(&mut app, [Message::Escape, Message::Escape]);
    assert_eq!(app.open_card(), None);
    send(&mut app, [Message::OpenLabels]);
    assert_eq!(app.open_card(), Some(b));
    assert_eq!(app.label_picker.as_ref().map(|p| p.card), Some(b));

    // M opens the Board picker on the open Card; with one Board it lists none, but opens.
    let m = keyboard::Event::KeyPressed {
        modified_key: Key::Character("m".into()),
        key: Key::Character("m".into()),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyM),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    };
    assert_eq!(keys::map(&m, &Context::default()), Some(Message::OpenMove));
    send(&mut app, [Message::Escape, Message::OpenMove]);
    assert_eq!(app.move_picker, Some(b));
    assert_eq!(app.open_card(), Some(b), "the picker leaves the Card open");
}
