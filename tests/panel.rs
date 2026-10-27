//! Seam 1 for the Card panel (#8): open/close/follow, title edit, timestamps.
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{App, Direction, Field, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::keys::{self, Context};
use kanvibe::panel;

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

fn click(app: &mut App, title: &str) {
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
fn clicking_a_card_opens_the_panel_on_it() {
    let mut app = app_with_cards();
    assert_eq!(app.panel, None);
    click(&mut app, "A");
    assert_eq!(app.panel, Some(card(&app, "A")));
}

#[test]
fn enter_opens_the_panel_on_the_selected_card() {
    let mut app = app_with_cards();
    let message = keys::map(&press(Named::Enter), &Context::default());
    assert_eq!(message, Some(Message::OpenPanel));

    send(&mut app, [Message::OpenPanel]);
    assert_eq!(app.panel, None, "nothing selected");

    send(&mut app, [Message::MoveSelection(Direction::Down)]);
    assert_eq!(app.panel, None, "selecting alone does not open it");
    send(&mut app, [Message::OpenPanel]);
    assert_eq!(app.panel, Some(card(&app, "A")));
}

#[test]
fn the_panel_follows_arrow_keys_and_clicks() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    send(&mut app, [Message::MoveSelection(Direction::Down)]);
    assert_eq!(app.panel, Some(card(&app, "B")));
    click(&mut app, "D");
    assert_eq!(app.panel, Some(card(&app, "D")));
}

#[test]
fn clicking_empty_board_space_keeps_the_panel_open() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    let doing = app.data.boards[0].columns[1].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column: doing,
            card: None,
        })],
    );
    assert_eq!(app.panel, Some(card(&app, "A")));
}

#[test]
fn escape_closes_the_panel_then_clears_the_selection() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    send(&mut app, [Message::Escape]);
    assert_eq!(app.panel, None);
    assert!(app.selection.is_some());
    send(&mut app, [Message::Escape]);
    assert_eq!(app.selection, None);
}

#[test]
fn the_x_button_closes_the_panel_and_keeps_the_selection() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    send(&mut app, [Message::ClosePanel]);
    assert_eq!(app.panel, None);
    assert!(app.selection.is_some());
}

#[test]
fn a_card_leaving_the_board_clears_panel_and_selection() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    let id = card(&app, "A");
    app.data.boards[0].columns[0].cards.retain(|c| c.id != id);
    send(&mut app, [Message::CommitEdit]); // any update validates
    assert_eq!((app.panel, app.selection), (None, None));
}

#[test]
fn undoing_the_creation_of_the_open_card_clears_panel_and_selection() {
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
    send(&mut app, [Message::OpenPanel]);
    assert_eq!(app.panel, Some(card(&app, "E")));

    send(&mut app, [Message::Undo]);
    assert_eq!((app.panel, app.selection), (None, None));
}

#[test]
fn f2_edits_the_open_cards_title_and_blank_reverts() {
    let mut app = app_with_cards();
    let message = keys::map(&press(Named::F2), &Context::default());
    assert_eq!(message, Some(Message::EditTitle));

    click(&mut app, "A");
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
    click(&mut app, "A");
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
    click(&mut app, "A");
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
fn neighbours_shifting_does_not_touch_updated_but_a_column_change_does() {
    let mut app = app_with_cards();
    click(&mut app, "C");
    send(&mut app, [Message::MoveCard(Direction::Up)]); // C above B, same Column
    assert_eq!(updated(&app, "A"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
    assert_eq!(updated(&app, "C"), OLD);

    send(&mut app, [Message::MoveCard(Direction::Right)]); // C into Doing
    assert_ne!(updated(&app, "C"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
}

#[test]
fn a_pending_title_edit_commits_to_its_own_card_before_the_panel_switches() {
    let mut app = app_with_cards();
    click(&mut app, "A");
    let a = card(&app, "A");
    send(
        &mut app,
        [Message::EditTitle, Message::EditChanged("Alpha".into())],
    );

    click(&mut app, "B");
    assert_eq!(app.editing, None);
    assert_eq!(app.panel, Some(card(&app, "B")));
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
        panel::local_time("2026-10-27T15:04:05Z", &tz),
        "2026-10-27 12:04"
    );
    assert_eq!(panel::local_time("garbage", &tz), "garbage");
}

#[test]
fn f2_e_and_l_open_the_panel_on_the_selected_card() {
    use kanvibe::app::PendingEdit;
    let mut app = app_with_cards();
    click(&mut app, "B");
    send(&mut app, [Message::Escape]); // closes the panel, keeps the selection
    assert_eq!(app.panel, None);
    let b = card(&app, "B");
    send(&mut app, [Message::EditTitle]);
    assert_eq!(app.panel, Some(b));
    assert!(matches!(
        app.editing,
        Some(PendingEdit {
            field: Field::CardTitle(id),
            ..
        }) if id == b
    ));
    send(&mut app, [Message::Escape, Message::Escape]);
    send(&mut app, [Message::EditDescription]);
    assert_eq!(app.panel, Some(b));
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::Description(id)) if id == b
    ));
    send(&mut app, [Message::Escape, Message::Escape]);
    assert_eq!(app.panel, None);
    send(&mut app, [Message::OpenLabels]);
    assert_eq!(app.panel, Some(b));
    assert_eq!(app.label_picker.as_ref().map(|p| p.card), Some(b));
}
