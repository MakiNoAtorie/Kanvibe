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

const NOTHING_FOCUSED: Context = Context { focused: None };
const CAPTURE_FOCUSED: Context = Context {
    focused: Some(Input::Capture),
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
