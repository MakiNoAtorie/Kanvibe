//! Seam 1 for Labels (#13): picker messages, Label rules, chips, persistence.
use iced::keyboard::{self, Key, Modifiers};
use kanvibe::app::{App, Field, Message, Selection};
use kanvibe::domain::Id;
use kanvibe::labels::{self, PALETTE};
use kanvibe::{keys, storage};

const OLD: &str = "2020-01-01T00:00:00Z";

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Board "Work" with Cards A, B in To do; A selected with its panel open and the picker closed.
/// Timestamps are `OLD`.
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
    let todo = app.data.boards[0].columns[0].id;
    for title in ["A", "B"] {
        send(
            &mut app,
            [
                Message::OpenCapture(todo),
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
    app.history = Default::default();
    let a = card(&app, "A");
    send(
        &mut app,
        [Message::Select(Selection {
            column: todo,
            card: Some(a),
        })],
    );
    app
}

fn card(app: &App, title: &str) -> Id {
    let cards = app.data.boards[0].columns.iter().flat_map(|c| &c.cards);
    cards.clone().find(|c| c.title == title).unwrap().id
}

fn updated(app: &App, title: &str) -> String {
    app.data.card(card(app, title)).unwrap().updated.clone()
}

fn names(app: &App) -> Vec<&str> {
    app.data.boards[0]
        .labels
        .iter()
        .map(|l| l.name.as_str())
        .collect()
}

fn label(app: &App, name: &str) -> Id {
    let labels = &app.data.boards[0].labels;
    labels.iter().find(|l| l.name == name).unwrap().id
}

fn color(app: &App, name: &str) -> String {
    let labels = &app.data.boards[0].labels;
    labels
        .iter()
        .find(|l| l.name == name)
        .unwrap()
        .color
        .clone()
}

/// Opens the picker on the open panel's Card and creates a Label through "+ New Label".
fn new_label(app: &mut App, name: &str) {
    if app.label_picker.is_none() {
        send(app, [Message::OpenLabels]);
    }
    send(
        app,
        [
            Message::NewLabel,
            Message::NewLabelChanged(name.into()),
            Message::CreateLabel,
        ],
    );
}

fn on_card(app: &App, title: &str) -> Vec<String> {
    let card = app.data.card(card(app, title)).unwrap();
    app.data
        .card_labels(card.id)
        .iter()
        .map(|l| l.name.clone())
        .collect()
}

#[test]
fn creating_a_label_adds_it_to_the_board_and_is_one_undo_step() {
    let mut app = app_with_cards();
    new_label(&mut app, "  Bug ");
    assert_eq!(names(&app), ["Bug"]);
    assert_eq!(color(&app, "Bug"), PALETTE[0].0);
    send(&mut app, [Message::Undo]);
    assert!(names(&app).is_empty());
    send(&mut app, [Message::Redo]);
    assert_eq!(names(&app), ["Bug"]);
}

#[test]
fn a_blank_label_name_creates_nothing() {
    let mut app = app_with_cards();
    new_label(&mut app, "   ");
    assert!(names(&app).is_empty());
}

#[test]
fn a_clashing_name_is_refused_on_create_and_the_form_stays_open() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    send(
        &mut app,
        [Message::NewLabel, Message::NewLabelChanged(" bUG ".into())],
    );
    let board = app.data.boards[0].id;
    assert!(app.data.label_clash(board, " bUG ", None));
    send(&mut app, [Message::CreateLabel]);
    assert_eq!(names(&app), ["Bug"]);
    assert!(app.label_picker.as_ref().unwrap().new.is_some());
    // fixing the name creates it
    send(
        &mut app,
        [
            Message::NewLabelChanged("Bugs".into()),
            Message::CreateLabel,
        ],
    );
    assert_eq!(names(&app), ["Bug", "Bugs"]);
}

#[test]
fn toggling_attaches_and_detaches_one_undo_step_each_and_touches_the_card() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(&mut app, [Message::ToggleLabel(bug)]);
    assert_eq!(on_card(&app, "A"), ["Bug"]);
    assert_ne!(updated(&app, "A"), OLD);
    assert_eq!(updated(&app, "B"), OLD);
    send(&mut app, [Message::ToggleLabel(bug)]);
    assert!(on_card(&app, "A").is_empty());
    send(&mut app, [Message::Undo]);
    assert_eq!(on_card(&app, "A"), ["Bug"]);
    send(&mut app, [Message::Undo]);
    assert!(on_card(&app, "A").is_empty());
    assert_eq!(names(&app), ["Bug"]); // the Label itself is a separate step
}

#[test]
fn a_cards_labels_come_in_board_label_order() {
    let mut app = app_with_cards();
    for name in ["Bug", "Idea", "Chore"] {
        new_label(&mut app, name);
    }
    let (bug, idea, chore) = (
        label(&app, "Bug"),
        label(&app, "Idea"),
        label(&app, "Chore"),
    );
    send(
        &mut app,
        [
            Message::ToggleLabel(chore),
            Message::ToggleLabel(bug),
            Message::ToggleLabel(idea),
        ],
    );
    assert_eq!(on_card(&app, "A"), ["Bug", "Idea", "Chore"]);
}

#[test]
fn rename_goes_through_the_pending_edit_and_is_one_undo_step() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(
        &mut app,
        [
            Message::EditLabel(bug),
            Message::EditChanged("D".into()),
            Message::EditChanged("Defect".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(names(&app), ["Defect"]);
    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["Bug"]);
}

#[test]
fn renaming_to_a_clashing_name_is_refused_and_reverts() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    new_label(&mut app, "Idea");
    let idea = label(&app, "Idea");
    send(
        &mut app,
        [
            Message::EditLabel(idea),
            Message::EditChanged("Id".into()),
            Message::EditChanged(" BUG".into()),
        ],
    );
    let edit = app.editing.as_ref().unwrap();
    assert_eq!(edit.field, Field::LabelName(idea));
    assert!(
        app.data
            .label_clash(app.data.boards[0].id, &edit.text, Some(idea))
    );
    assert_eq!(names(&app), ["Bug", "Idea"]); // live data holds the old name, not "Id"
    send(&mut app, [Message::CommitEdit]);
    assert_eq!(names(&app), ["Bug", "Idea"]);
    send(&mut app, [Message::Undo]); // the previous step was the creation of Idea
    assert_eq!(names(&app), ["Bug"]);
}

#[test]
fn renaming_only_the_case_of_its_own_name_is_allowed() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(
        &mut app,
        [
            Message::EditLabel(bug),
            Message::EditChanged("bug".into()),
            Message::CommitEdit,
        ],
    );
    assert_eq!(names(&app), ["bug"]);
}

#[test]
fn recolour_does_not_touch_cards() {
    // Label name and colour live on the Board; `updated` is the Card's own change.
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(&mut app, [Message::ToggleLabel(bug)]);
    let before = updated(&app, "A");
    send(&mut app, [Message::SetLabelColor(bug, "red".into())]);
    assert_eq!(updated(&app, "A"), before);
}

#[test]
fn recolour_stores_the_palette_key_and_is_one_undo_step() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(&mut app, [Message::SetLabelColor(bug, "teal".into())]);
    assert_eq!(color(&app, "Bug"), "teal");
    send(&mut app, [Message::Undo]);
    assert_eq!(color(&app, "Bug"), PALETTE[0].0);
    send(&mut app, [Message::SetLabelColor(bug, "nope".into())]);
    assert_eq!(color(&app, "Bug"), PALETTE[0].0); // unknown key refused
}

#[test]
fn deleting_an_unused_label_needs_no_prompt() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(&mut app, [Message::DeleteLabel(bug)]);
    assert!(names(&app).is_empty());
    assert_eq!(app.confirm, None);
    send(&mut app, [Message::Undo]);
    assert_eq!(names(&app), ["Bug"]);
}

#[test]
fn deleting_an_in_use_label_asks_with_the_count_then_strips_every_card() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(&mut app, [Message::ToggleLabel(bug)]);
    let b = card(&app, "B");
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::Select(Selection {
                column: todo,
                card: Some(b),
            }),
            Message::OpenLabels,
            Message::ToggleLabel(bug),
        ],
    );
    send(&mut app, [Message::DeleteLabel(bug)]);
    assert_eq!(names(&app), ["Bug"]); // nothing yet
    assert_eq!(
        app.confirm.as_ref().map(|c| c.action.clone()),
        Some(Message::RemoveLabel(bug))
    );
    assert_eq!(
        app.confirm.as_ref().map(|c| c.prompt.as_str()),
        Some("Delete Label \"Bug\"? It is on 2 Cards.")
    );
    app.data.boards[0]
        .columns
        .iter_mut()
        .flat_map(|c| &mut c.cards)
        .for_each(|c| c.updated = OLD.into());
    send(&mut app, [Message::Confirm]);
    assert!(names(&app).is_empty());
    assert!(on_card(&app, "A").is_empty() && on_card(&app, "B").is_empty());
    assert_ne!(updated(&app, "A"), OLD);
    assert_ne!(updated(&app, "B"), OLD);
    assert_eq!(app.confirm, None);
    send(&mut app, [Message::Undo]); // one step restores the Label and both attachments
    assert_eq!(names(&app), ["Bug"]);
    assert_eq!(on_card(&app, "A"), ["Bug"]);
    assert_eq!(on_card(&app, "B"), ["Bug"]);
}

#[test]
fn an_archived_card_counts_toward_the_label_and_is_stripped_and_touched_on_delete() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    let a = card(&app, "A");
    send(
        &mut app,
        [Message::ToggleLabel(bug), Message::ArchiveCard(a)],
    );
    assert_eq!(app.data.label_use(bug), 1);

    send(&mut app, [Message::DeleteLabel(bug)]);
    assert_eq!(
        app.confirm.as_ref().map(|c| c.prompt.as_str()),
        Some("Delete Label \"Bug\"? It is on 1 Card.")
    );
    app.data.boards[0].archived_cards[0].card.updated = OLD.into();
    send(&mut app, [Message::Confirm]);
    let archived = &app.data.boards[0].archived_cards[0].card;
    assert!(archived.labels.is_empty());
    assert_ne!(archived.updated, OLD);
}

#[test]
fn the_prompt_uses_the_singular_for_one_card_and_cancel_keeps_the_label() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(
        &mut app,
        [Message::ToggleLabel(bug), Message::DeleteLabel(bug)],
    );
    assert_eq!(
        app.confirm.as_ref().map(|c| c.prompt.as_str()),
        Some("Delete Label \"Bug\"? It is on 1 Card.")
    );
    send(&mut app, [Message::Escape]);
    assert_eq!(names(&app), ["Bug"]);
    assert_eq!(on_card(&app, "A"), ["Bug"]);
}

#[test]
fn escape_backs_out_confirm_then_picker_then_panel() {
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(
        &mut app,
        [Message::ToggleLabel(bug), Message::DeleteLabel(bug)],
    );
    send(&mut app, [Message::Escape]);
    assert_eq!(app.confirm, None);
    assert_eq!(names(&app), ["Bug"]);
    send(&mut app, [Message::Escape]);
    assert!(app.label_picker.is_none());
    assert!(app.panel.is_some());
    send(&mut app, [Message::Escape]);
    assert!(app.panel.is_none());
}

#[test]
fn l_opens_the_picker_only_with_a_card_open_or_selected() {
    let press = |app: &mut App| {
        let event = keyboard::Event::KeyPressed {
            key: Key::Character("l".into()),
            modified_key: Key::Character("l".into()),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyL),
            location: keyboard::Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        };
        if let Some(message) = keys::map(&event, &keys::Context::default()) {
            let _ = app.update(message);
        }
    };
    let mut app = app_with_cards();
    app.panel = None;
    app.selection = None;
    press(&mut app);
    assert!(app.label_picker.is_none());
    let a = card(&app, "A");
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column: todo,
            card: Some(a),
        })],
    );
    press(&mut app);
    assert_eq!(app.label_picker.as_ref().unwrap().card, a);
}

#[test]
fn the_picker_closes_when_the_panel_closes_or_follows_another_card() {
    let mut app = app_with_cards();
    send(&mut app, [Message::OpenLabels, Message::ClosePanel]);
    assert!(app.label_picker.is_none());
    let (a, b) = (card(&app, "A"), card(&app, "B"));
    let todo = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::Select(Selection {
                column: todo,
                card: Some(a),
            }),
            Message::OpenLabels,
            Message::Select(Selection {
                column: todo,
                card: Some(b),
            }),
        ],
    );
    assert!(app.label_picker.is_none());
}

#[test]
fn chips_cut_names_at_12_characters() {
    assert_eq!(labels::chip_text("Short"), "Short");
    assert_eq!(labels::chip_text("123456789012"), "123456789012");
    assert_eq!(labels::chip_text("1234567890123"), "123456789012");
    assert_eq!(labels::chip_text("ééééééééééééé"), "éééééééééééé");
}

#[test]
fn palette_has_about_ten_unique_keys_and_unknown_keys_fall_back_to_the_first() {
    assert!((9..=11).contains(&PALETTE.len()));
    let mut keys: Vec<_> = PALETTE.iter().map(|(k, _)| *k).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), PALETTE.len());
    assert!(PALETTE.iter().any(|(k, _)| *k == "teal"));
    assert_ne!(labels::swatch("teal"), labels::swatch("nope"));
    assert_eq!(labels::swatch("nope"), PALETTE[0].1);
}

#[test]
fn labels_and_colour_keys_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_cards();
    new_label(&mut app, "Bug");
    let bug = label(&app, "Bug");
    send(
        &mut app,
        [
            Message::SetLabelColor(bug, "teal".into()),
            Message::ToggleLabel(bug),
        ],
    );
    storage::save(dir.path(), &app.data, &Default::default()).unwrap();
    let json = std::fs::read_to_string(dir.path().join("kanvibe.json")).unwrap();
    assert!(json.contains("\"color\": \"teal\""), "{json}");
    let (data, _) = storage::load(dir.path()).unwrap().unwrap();
    assert_eq!(data, app.data);
    assert_eq!(data.boards[0].labels[0].name, "Bug");
}

#[test]
fn files_without_labels_still_load() {
    let json = r#"{"version":1,"next_id":3,"boards":[{"id":1,"name":"W","columns":[{"id":2,"name":"C","cards":[{"id":3,"title":"T"}]}]}]}"#;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("kanvibe.json"), json).unwrap();
    let (data, _) = storage::load(dir.path()).unwrap().unwrap();
    assert!(data.boards[0].labels.is_empty());
    assert!(data.boards[0].columns[0].cards[0].labels.is_empty());
}
