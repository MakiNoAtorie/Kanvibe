//! The key mapping as a pure function: (key event, context) -> optional Message.
use iced::keyboard::{self, Key, Modifiers, key::Named};
use kanvibe::app::{Direction, Message};
use kanvibe::keys::{self, Context, Input};

fn press(key: Key, modifiers: Modifiers) -> keyboard::Event {
    use keyboard::key::{Code, Physical};
    let physical_key = match &key {
        Key::Character(c) => match c.as_str() {
            "n" => Physical::Code(Code::KeyN),
            "z" => Physical::Code(Code::KeyZ),
            "y" => Physical::Code(Code::KeyY),
            _ => unreachable!(),
        },
        _ => Physical::Code(Code::F24),
    };
    keyboard::Event::KeyPressed {
        modified_key: key.clone(),
        key,
        physical_key,
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    }
}

fn named(key: Named) -> keyboard::Event {
    press(Key::Named(key), Modifiers::empty())
}

fn ctrl_named(key: Named) -> keyboard::Event {
    press(Key::Named(key), Modifiers::CTRL)
}

fn letter(c: &str, modifiers: Modifiers) -> keyboard::Event {
    press(Key::Character(c.into()), modifiers)
}

const NOTHING_FOCUSED: Context = Context {
    focused: None,
    dragging: false,
};
const CAPTURE_FOCUSED: Context = Context {
    focused: Some(Input::Capture),
    dragging: false,
};

#[test]
fn arrows_move_the_selection_and_ctrl_arrows_move_the_card() {
    use Direction::*;
    for (key, direction) in [
        (Named::ArrowLeft, Left),
        (Named::ArrowRight, Right),
        (Named::ArrowUp, Up),
        (Named::ArrowDown, Down),
    ] {
        assert_eq!(
            keys::map(&named(key), &NOTHING_FOCUSED),
            Some(Message::MoveSelection(direction))
        );
        assert_eq!(
            keys::map(&ctrl_named(key), &NOTHING_FOCUSED),
            Some(Message::MoveCard(direction))
        );
    }
}

#[test]
fn n_opens_the_capture_box_unless_typing_or_with_ctrl() {
    assert_eq!(
        keys::map(&letter("n", Modifiers::empty()), &NOTHING_FOCUSED),
        Some(Message::NewCard)
    );
    assert_eq!(
        keys::map(&letter("n", Modifiers::CTRL), &NOTHING_FOCUSED),
        None
    );
    assert_eq!(
        keys::map(&letter("n", Modifiers::empty()), &CAPTURE_FOCUSED),
        None
    );
}

#[test]
fn a_focused_text_field_keeps_arrows_but_not_undo_redo_or_esc() {
    for key in [Named::ArrowUp, Named::ArrowDown] {
        assert_eq!(keys::map(&named(key), &CAPTURE_FOCUSED), None);
        assert_eq!(keys::map(&ctrl_named(key), &CAPTURE_FOCUSED), None);
    }
    for context in [NOTHING_FOCUSED, CAPTURE_FOCUSED] {
        assert_eq!(
            keys::map(&letter("z", Modifiers::CTRL), &context),
            Some(Message::Undo)
        );
        assert_eq!(
            keys::map(&letter("y", Modifiers::CTRL), &context),
            Some(Message::Redo)
        );
        assert_eq!(
            keys::map(&letter("z", Modifiers::CTRL | Modifiers::SHIFT), &context),
            Some(Message::Redo)
        );
        assert_eq!(
            keys::map(&named(Named::Escape), &context),
            Some(Message::Escape)
        );
    }
    assert_eq!(
        keys::map(&letter("z", Modifiers::empty()), &NOTHING_FOCUSED),
        None
    );
}

#[test]
fn l_maps_by_its_latin_key_like_n_e_and_m() {
    use keyboard::key::{Code, Physical};
    // A non-Latin layout: the key in the L position types a Cyrillic letter.
    let event = keyboard::Event::KeyPressed {
        modified_key: Key::Character("д".into()),
        key: Key::Character("д".into()),
        physical_key: Physical::Code(Code::KeyL),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    };
    assert_eq!(
        keys::map(&event, &NOTHING_FOCUSED),
        Some(Message::OpenLabels)
    );
}

#[test]
fn key_releases_map_to_nothing() {
    let release = keyboard::Event::KeyReleased {
        key: Key::Named(Named::ArrowDown),
        modified_key: Key::Named(Named::ArrowDown),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::ArrowDown),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
    };
    assert_eq!(keys::map(&release, &NOTHING_FOCUSED), None);
}

// --- Dialogs: only Esc reaches the app ---

use kanvibe::app::{App, Closing, Selection};

fn letter_key(c: &str) -> keyboard::Event {
    use keyboard::key::{Code, Physical};
    let code = match c {
        "n" => Code::KeyN,
        "m" => Code::KeyM,
        "e" => Code::KeyE,
        "l" => Code::KeyL,
        _ => unreachable!(),
    };
    keyboard::Event::KeyPressed {
        modified_key: Key::Character(c.into()),
        key: Key::Character(c.into()),
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

/// Boards "Work" (current) and "Home"; Cards "A", "B" in Work's To do; "A" selected, panel open.
fn app_with_open_card() -> App {
    let mut app = App::default();
    for name in ["Work", "Home"] {
        send(
            &mut app,
            [
                Message::NewBoard,
                Message::BoardNameChanged(name.into()),
                Message::CreateBoard,
            ],
        );
    }
    let work = app.data.boards[0].id;
    let to_do = app.data.boards[0].columns[0].id;
    send(
        &mut app,
        [
            Message::OpenBoard(work),
            Message::OpenCapture(to_do),
            Message::CaptureChanged("A".into()),
            Message::SubmitCapture,
            Message::CaptureChanged("B".into()),
            Message::SubmitCapture,
            Message::Escape,
        ],
    );
    let card = Some(app.data.boards[0].columns[0].cards[0].id);
    send(
        &mut app,
        [Message::Select(Selection {
            column: to_do,
            card,
        })],
    );
    assert_eq!(app.panel, card);
    app
}

/// Everything a Board key could change.
fn state(app: &App) -> String {
    format!(
        "{:?}",
        (
            &app.data,
            app.current_board,
            app.selection,
            app.panel,
            &app.capture,
            &app.editing,
            &app.label_picker,
            app.move_picker,
            app.shortcuts,
            app.closing,
        )
    )
}

/// Presses every Board key while a dialog is open: none reaches the Board. Esc then closes it.
fn only_esc_gets_through(app: &mut App, closed: impl Fn(&App) -> bool) {
    let keys = [
        named(Named::Delete),
        named(Named::ArrowDown),
        named(Named::ArrowRight),
        ctrl_named(Named::ArrowRight),
        ctrl_named(Named::ArrowDown),
        named(Named::Enter),
        named(Named::F2),
        letter_key("n"),
        letter_key("m"),
        letter_key("e"),
        letter_key("l"),
    ];
    for key in keys {
        let before = state(app);
        send(app, [Message::KeyWithFocus(key.clone(), None)]);
        assert_eq!(state(app), before, "{key:?} reached the Board");
    }
    assert!(!closed(app), "the dialog is open");
    send(app, [Message::KeyWithFocus(named(Named::Escape), None)]);
    assert!(closed(app), "Esc closes the dialog");
}

#[test]
fn only_esc_reaches_the_app_while_the_shortcuts_overlay_is_open() {
    let mut app = app_with_open_card();
    send(&mut app, [Message::ShowShortcuts]);
    only_esc_gets_through(&mut app, |app| !app.shortcuts);
}

#[test]
fn only_esc_reaches_the_app_while_the_label_picker_is_open() {
    let mut app = app_with_open_card();
    send(&mut app, [Message::OpenLabels]);
    only_esc_gets_through(&mut app, |app| app.label_picker.is_none());
}

#[test]
fn only_esc_reaches_the_app_while_the_move_list_is_open() {
    let mut app = app_with_open_card();
    send(&mut app, [Message::OpenMove]);
    only_esc_gets_through(&mut app, |app| app.move_picker.is_none());
}

#[test]
fn only_esc_reaches_the_app_while_the_close_dialog_is_open() {
    let mut app = app_with_open_card();
    app.closing = Closing::Asking;
    app.not_saved = Some("Not saved: kanvibe.json: disk full".into());
    only_esc_gets_through(&mut app, |app| app.closing == Closing::No);
}

#[test]
fn ctrl_tab_cycles_boards_even_while_typing_and_home_end_select_or_move() {
    let ctrl_shift_tab = press(Key::Named(Named::Tab), Modifiers::CTRL | Modifiers::SHIFT);
    for context in [NOTHING_FOCUSED, CAPTURE_FOCUSED] {
        assert_eq!(
            keys::map(&ctrl_named(Named::Tab), &context),
            Some(Message::CycleBoard(true))
        );
        assert_eq!(
            keys::map(&ctrl_shift_tab, &context),
            Some(Message::CycleBoard(false))
        );
    }
    assert_eq!(keys::map(&named(Named::Tab), &NOTHING_FOCUSED), None);
    for (key, direction) in [
        (Named::Home, Direction::Top),
        (Named::End, Direction::Bottom),
    ] {
        assert_eq!(
            keys::map(&named(key), &NOTHING_FOCUSED),
            Some(Message::MoveSelection(direction))
        );
        assert_eq!(
            keys::map(&ctrl_named(key), &NOTHING_FOCUSED),
            Some(Message::MoveCard(direction))
        );
        assert_eq!(keys::map(&named(key), &CAPTURE_FOCUSED), None);
    }
}
