//! The key mapping: a pure function from (key event, context) to an optional Message.
//! The app queries which widget has focus on each key event, then calls [`map`].
use iced::keyboard;

use crate::app::{Direction, Message};

/// What the mapping needs to know besides the key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// The text field that has keyboard focus, if any.
    pub focused: Option<Input>,
    /// A press or drag is in progress: Esc cancels it, other keys do nothing.
    pub dragging: bool,
}

/// The app's text boxes (one widget Id each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// The new-Board name prompt.
    BoardName,
    Capture,
    /// The box of the pending edit (`app::Field`): a rename, later titles and the like.
    Edit,
}

/// Maps a key press to the Message it stands for. iced only delivers keys the focused widget
/// did not capture; a focused text field still lets arrows through, which stay with the field.
pub fn map(event: &keyboard::Event, context: &Context) -> Option<Message> {
    use keyboard::{Key, key::Named};
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    if context.dragging {
        return (*key == Key::Named(Named::Escape)).then_some(Message::DragCancel);
    }
    let ctrl = modifiers.command();
    let typing = context.focused.is_some();
    match (ctrl, key.to_latin(*physical_key)) {
        (true, Some('z')) if modifiers.shift() => return Some(Message::Redo),
        (true, Some('z')) => return Some(Message::Undo),
        (true, Some('y')) => return Some(Message::Redo),
        (false, Some('n')) if !typing && !modifiers.alt() => return Some(Message::NewCard),
        _ => {}
    }
    if *key == Key::Named(Named::Escape) {
        return Some(Message::Escape);
    }
    if typing {
        return None;
    }
    match key {
        Key::Named(Named::Enter) if !ctrl => return Some(Message::OpenPanel),
        Key::Named(Named::F2) => return Some(Message::EditTitle),
        Key::Named(Named::Delete) => return Some(Message::ArchiveSelected),
        _ => {}
    }
    let direction = match key {
        Key::Named(Named::ArrowLeft) => Direction::Left,
        Key::Named(Named::ArrowRight) => Direction::Right,
        Key::Named(Named::ArrowUp) => Direction::Up,
        Key::Named(Named::ArrowDown) => Direction::Down,
        _ => return None,
    };
    Some(if ctrl {
        Message::MoveCard(direction)
    } else {
        Message::MoveSelection(direction)
    })
}
