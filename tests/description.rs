//! Seam 1 for the Markdown description (#9): editor toggle, Ctrl+B/I/K, links, undo, idle write.
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::text_editor;
use kanvibe::app::{App, Field, Message, Selection};
use kanvibe::description::{self, Markup};
use kanvibe::domain::Id;
use kanvibe::keys::{self, Context};
use std::time::{Duration, Instant};

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Board "Work" with Card "A" in To do, selected, panel open.
fn app_with_panel() -> (App, Id) {
    let mut app = App::default();
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    let column = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenCapture(column),
            Message::CaptureChanged("A".into()),
            Message::SubmitCapture,
            Message::Escape,
        ],
    );
    let card = app.data.boards[0].columns[0].cards[0].id;
    send(
        &mut app,
        [Message::Select(Selection {
            column,
            card: Some(card),
        })],
    );
    assert_eq!(app.panel, Some(card));
    (app, card)
}

fn editing_description(app: &App) -> bool {
    matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(Field::Description(_))
    )
}

fn press(c: &str) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        key: Key::Character(c.into()),
        modified_key: Key::Character(c.into()),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: Some(c.into()),
        repeat: false,
    }
}

#[test]
fn e_opens_the_editor_and_done_returns_to_the_rendered_view() {
    let (mut app, _) = app_with_panel();
    let e = keys::map(&press("e"), &Context::default());
    assert_eq!(e, Some(Message::EditDescription));

    send(&mut app, [Message::EditDescription]);
    assert!(editing_description(&app));

    send(&mut app, [Message::CommitEdit]);
    assert!(!editing_description(&app));
    assert!(app.panel.is_some(), "Done keeps the panel open");
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        send(app, [edit(text_editor::Edit::Insert(c))]);
    }
}

fn edit(edit: text_editor::Edit) -> Message {
    Message::DescriptionAction(text_editor::Action::Edit(edit))
}

fn description(app: &App, card: Id) -> &str {
    app.data.description(card).unwrap()
}

#[test]
fn typing_writes_live_and_the_whole_edit_is_one_undo_step() {
    let (mut app, card) = app_with_panel();
    send(&mut app, [Message::EditDescription]);
    type_text(&mut app, "Hi");
    send(&mut app, [edit(text_editor::Edit::Enter)]);
    type_text(&mut app, "there");
    assert_eq!(
        description(&app, card),
        "Hi\nthere",
        "Enter inserts a newline"
    );

    send(&mut app, [Message::Escape]);
    assert!(!editing_description(&app), "Esc leaves the editor");
    assert!(app.panel.is_some(), "the first Esc only leaves the editor");
    assert_eq!(description(&app, card), "Hi\nthere");

    send(&mut app, [Message::Undo]);
    assert_eq!(description(&app, card), "");
    send(&mut app, [Message::Redo]);
    assert_eq!(description(&app, card), "Hi\nthere");
}

#[test]
fn clicking_outside_leaves_the_editor_but_not_a_rename() {
    let (mut app, card) = app_with_panel();
    send(&mut app, [Message::EditDescription]);
    type_text(&mut app, "x");
    send(&mut app, [Message::ClickOutside]);
    assert!(!editing_description(&app));
    assert_eq!(description(&app, card), "x");

    send(
        &mut app,
        [
            Message::StartEdit(Field::CardTitle(card)),
            Message::ClickOutside,
        ],
    );
    assert!(
        app.editing.is_some(),
        "a rename keeps its own commit points"
    );
}

#[test]
fn the_live_description_is_written_a_second_after_typing_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, card) = app_with_panel();
    app.data_dir = Some(tmp.path().into());
    let saved = |app: &mut App| {
        while let Some(job) = app.save_if_changed() {
            let _ = app.update(job.run());
        }
        let (data, _) = kanvibe::storage::load(tmp.path()).unwrap().unwrap();
        data.description(card).unwrap().to_owned()
    };
    assert_eq!(saved(&mut app), "");
    send(&mut app, [Message::EditDescription]);
    type_text(&mut app, "draft");
    assert_eq!(saved(&mut app), "", "not while typing");

    send(
        &mut app,
        [Message::EditTick(Instant::now() + Duration::from_secs(1))],
    );
    assert_eq!(saved(&mut app), "draft");
    assert!(
        editing_description(&app),
        "the idle write keeps the editor open"
    );
}

#[test]
fn only_http_https_and_mailto_links_open() {
    for url in [
        "http://example.com",
        "https://example.com/a?b",
        "HTTPS://EXAMPLE.COM",
        "mailto:ann@example.com",
    ] {
        assert!(description::opens_in_browser(url), "{url}");
    }
    for url in [
        "file:///C:/Windows/System32/calc.exe",
        "C:\\Windows\\notepad.exe",
        "javascript:alert(1)",
        "ftp://example.com",
        "httpx://example.com",
        "relative/page",
        "",
    ] {
        assert!(!description::opens_in_browser(url), "{url}");
    }
}

fn ctrl_in_editor(c: &str) -> text_editor::KeyPress {
    text_editor::KeyPress {
        key: Key::Character(c.into()),
        modified_key: Key::Character(c.into()),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        modifiers: Modifiers::CTRL,
        text: None,
        status: text_editor::Status::Focused { is_hovered: false },
    }
}

#[test]
fn ctrl_b_i_k_are_editor_bindings_for_bold_italic_link() {
    for (c, markup) in [
        ("b", Markup::Bold),
        ("i", Markup::Italic),
        ("k", Markup::Link),
    ] {
        assert_eq!(
            description::key_binding(ctrl_in_editor(c)),
            Some(text_editor::Binding::Custom(Message::Format(markup)))
        );
    }
    let unfocused = text_editor::KeyPress {
        status: text_editor::Status::Active,
        ..ctrl_in_editor("b")
    };
    assert_eq!(description::key_binding(unfocused), None);
}

#[test]
fn markup_wraps_the_selection() {
    let (mut app, card) = app_with_panel();
    send(&mut app, [Message::EditDescription]);
    type_text(&mut app, "note");
    send(
        &mut app,
        [
            Message::DescriptionAction(text_editor::Action::SelectAll),
            Message::Format(Markup::Bold),
        ],
    );
    assert_eq!(description(&app, card), "**note**");
    send(
        &mut app,
        [
            Message::DescriptionAction(text_editor::Action::SelectAll),
            Message::Format(Markup::Link),
        ],
    );
    assert_eq!(description(&app, card), "[**note**](url)");
}

#[test]
fn markup_without_selection_puts_the_cursor_between_the_markers() {
    for (markup, expected) in [
        (Markup::Bold, "**x**"),
        (Markup::Italic, "*x*"),
        (Markup::Link, "[x](url)"),
    ] {
        let (mut app, card) = app_with_panel();
        send(
            &mut app,
            [Message::EditDescription, Message::Format(markup)],
        );
        type_text(&mut app, "x");
        assert_eq!(description(&app, card), expected);
    }
}
