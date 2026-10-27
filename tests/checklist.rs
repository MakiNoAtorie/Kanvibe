//! Seam 1 for Checklists (#14): items are added, edited, ticked, reordered and removed by ID.
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{App, Direction, Field, Message};
use kanvibe::domain::{Id, Item};
use kanvibe::keys::{self, Context, Input};

const OLD: &str = "2020-01-01T00:00:00Z";

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// One Board with Card "A" in To do; its timestamps are `OLD`.
fn app_with_card() -> (App, Id) {
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
            Message::Escape,
        ],
    );
    let card = &mut app.data.boards[0].columns[0].cards[0];
    card.created = OLD.into();
    card.updated = OLD.into();
    let id = card.id;
    app.history = Default::default();
    (app, id)
}

fn items(app: &App, card: Id) -> Vec<Item> {
    app.data.card(card).unwrap().checklist.clone()
}

fn texts(app: &App, card: Id) -> Vec<String> {
    items(app, card).into_iter().map(|i| i.text).collect()
}

/// Adds an Item with this text through the add button and commits it.
fn add(app: &mut App, card: Id, text: &str) -> Id {
    send(
        app,
        [
            Message::AddItem(card),
            Message::EditChanged(text.into()),
            Message::CommitEdit,
        ],
    );
    items(app, card).last().unwrap().id
}

fn press(key: Named, modifiers: Modifiers) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        modified_key: Key::Named(key),
        key: Key::Named(key),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    }
}

#[test]
fn an_added_item_is_one_undo_step_with_its_own_id() {
    let (mut app, card) = app_with_card();
    let a = add(&mut app, card, "  write spec ");
    let b = add(&mut app, card, "ship");
    assert_eq!(texts(&app, card), ["write spec", "ship"]);
    assert_ne!(a, b);
    assert!(items(&app, card).iter().all(|i| !i.done));

    send(&mut app, [Message::Undo]);
    assert_eq!(texts(&app, card), ["write spec"]);
    send(&mut app, [Message::Undo]);
    assert!(items(&app, card).is_empty());
}

#[test]
fn a_blank_item_is_removed_on_commit_and_records_no_step() {
    let (mut app, card) = app_with_card();
    add(&mut app, card, "one");
    send(&mut app, [Message::AddItem(card), Message::CommitEdit]);
    assert_eq!(texts(&app, card), ["one"]);
    send(&mut app, [Message::Undo]); // undoes "one", not a phantom step
    assert!(items(&app, card).is_empty());
}

#[test]
fn clearing_an_item_and_committing_removes_it() {
    let (mut app, card) = app_with_card();
    let one = add(&mut app, card, "one");
    send(
        &mut app,
        [
            Message::StartEdit(Field::ChecklistItem(one)),
            Message::EditChanged("  ".into()),
            Message::CommitEdit,
        ],
    );
    assert!(items(&app, card).is_empty());
    send(&mut app, [Message::Undo]);
    assert_eq!(texts(&app, card), ["one"]);
}

#[test]
fn renaming_an_item_is_one_undo_step() {
    let (mut app, card) = app_with_card();
    let one = add(&mut app, card, "one");
    send(
        &mut app,
        [
            Message::StartEdit(Field::ChecklistItem(one)),
            Message::EditChanged("o".into()),
            Message::EditChanged("uno".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(texts(&app, card), ["uno"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(texts(&app, card), ["one"]);
}

#[test]
fn ticking_and_unticking_are_undo_steps_and_update_the_card() {
    let (mut app, card) = app_with_card();
    let one = add(&mut app, card, "one");
    app.data.boards[0].columns[0].cards[0].updated = OLD.into();
    send(&mut app, [Message::ToggleItem(one)]);
    assert!(items(&app, card)[0].done);
    assert_ne!(app.data.card(card).unwrap().updated, OLD);
    send(&mut app, [Message::ToggleItem(one)]);
    assert!(!items(&app, card)[0].done);
    send(&mut app, [Message::Undo]);
    assert!(items(&app, card)[0].done);
}

#[test]
fn items_reorder_with_buttons_and_stop_at_the_ends() {
    let (mut app, card) = app_with_card();
    let a = add(&mut app, card, "a");
    let _b = add(&mut app, card, "b");
    let c = add(&mut app, card, "c");
    send(&mut app, [Message::MoveItem(c, Direction::Up)]);
    assert_eq!(texts(&app, card), ["a", "c", "b"]);
    send(&mut app, [Message::MoveItem(a, Direction::Up)]); // already first
    assert_eq!(texts(&app, card), ["a", "c", "b"]);
    send(&mut app, [Message::MoveItem(a, Direction::Down)]);
    assert_eq!(texts(&app, card), ["c", "a", "b"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(texts(&app, card), ["a", "c", "b"]);
}

#[test]
fn a_reorder_never_redirects_an_edit() {
    let (mut app, card) = app_with_card();
    add(&mut app, card, "a");
    let b = add(&mut app, card, "b");
    send(
        &mut app,
        [
            Message::StartEdit(Field::ChecklistItem(b)),
            Message::MoveEditedItem(Direction::Up),
        ],
    );
    assert_eq!(texts(&app, card), ["b", "a"]);
    send(
        &mut app,
        [Message::EditChanged("bee".into()), Message::CommitEdit],
    );
    assert_eq!(texts(&app, card), ["bee", "a"]);
}

#[test]
fn ctrl_up_and_down_reorder_only_while_an_item_is_focused() {
    let ctrl = Modifiers::CTRL;
    let item = Context {
        focused: Some(Input::Item),
        ..Context::default()
    };
    assert_eq!(
        keys::map(&press(Named::ArrowUp, ctrl), &item),
        Some(Message::MoveEditedItem(Direction::Up))
    );
    assert_eq!(
        keys::map(&press(Named::ArrowDown, ctrl), &item),
        Some(Message::MoveEditedItem(Direction::Down))
    );
    let edit = Context {
        focused: Some(Input::Edit),
        ..Context::default()
    };
    assert_eq!(keys::map(&press(Named::ArrowUp, ctrl), &edit), None);
}

#[test]
fn enter_adds_an_item_below_and_a_blank_one_stops_the_chain() {
    let (mut app, card) = app_with_card();
    let a = add(&mut app, card, "a");
    add(&mut app, card, "c");
    send(
        &mut app,
        [
            Message::StartEdit(Field::ChecklistItem(a)),
            Message::SubmitItem(a),
            Message::EditChanged("b".into()),
        ],
    );
    assert_eq!(texts(&app, card), ["a", "b", "c"]);
    let b = items(&app, card)[1].id;
    assert_eq!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::ChecklistItem(b))
    );

    send(&mut app, [Message::SubmitItem(b)]); // adds a blank one below b
    assert_eq!(texts(&app, card), ["a", "b", "", "c"]);
    let blank = items(&app, card)[2].id;
    send(&mut app, [Message::SubmitItem(blank)]);
    assert_eq!(texts(&app, card), ["a", "b", "c"]);
    assert_eq!(app.editing, None);

    send(&mut app, [Message::Undo]); // one step for "b" together with its add
    assert_eq!(texts(&app, card), ["a", "c"]);
}

#[test]
fn removing_an_item_is_one_undo_step() {
    let (mut app, card) = app_with_card();
    let a = add(&mut app, card, "a");
    add(&mut app, card, "b");
    send(&mut app, [Message::RemoveItem(a)]);
    assert_eq!(texts(&app, card), ["b"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(texts(&app, card), ["a", "b"]);
}

#[test]
fn progress_is_done_over_total_when_there_are_items() {
    let (mut app, card) = app_with_card();
    assert_eq!(app.data.card(card).unwrap().progress(), None);
    let a = add(&mut app, card, "a");
    add(&mut app, card, "b");
    send(&mut app, [Message::ToggleItem(a)]);
    assert_eq!(
        app.data.card(card).unwrap().progress().as_deref(),
        Some("1/2")
    );
}

#[test]
fn the_checklist_is_saved_and_loaded() {
    let (mut app, card) = app_with_card();
    let a = add(&mut app, card, "a");
    send(&mut app, [Message::ToggleItem(a)]);
    let dir = tempfile::tempdir().unwrap();
    let settings = Default::default();
    kanvibe::storage::save(dir.path(), &app.data, &settings).unwrap();
    let (loaded, _) = kanvibe::storage::load(dir.path()).unwrap().unwrap();
    assert_eq!(loaded, app.data);
    assert!(loaded.card(card).unwrap().checklist[0].done);
}
