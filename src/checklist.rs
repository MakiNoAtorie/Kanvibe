//! The Checklist section of the Card panel.
use iced::widget::{button, checkbox, column, mouse_area, row, text, text_input};
use iced::{Center, Element, Fill};

use crate::app::{App, Direction, EDIT_INPUT, Field, Message};
use crate::domain::{Card, Item};

impl App {
    pub(crate) fn view_checklist<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let items = card.checklist.iter().map(|item| self.view_item(item));
        column![text("Checklist").size(14)]
            .extend(items)
            .push(
                button("+ Add item")
                    .style(button::text)
                    .on_press(Message::AddItem(card.id)),
            )
            .spacing(6)
            .into()
    }

    fn view_item<'a>(&'a self, item: &'a Item) -> Element<'a, Message> {
        let field = Field::ChecklistItem(item.id);
        let label: Element<_> = match &self.editing {
            Some(edit) if edit.field == field => text_input("Item", &edit.text)
                .id(EDIT_INPUT)
                .on_input(Message::EditChanged)
                .on_submit(Message::SubmitItem(item.id))
                .into(),
            _ => mouse_area(text(&item.text).width(Fill))
                .on_press(Message::StartEdit(field))
                .into(),
        };
        let small = |label, message| {
            button(text(label).size(12))
                .style(button::text)
                .on_press(message)
        };
        row![
            checkbox(item.done).on_toggle(|_| Message::ToggleItem(item.id)),
            label,
            small("▲", Message::MoveItem(item.id, Direction::Up)),
            small("▼", Message::MoveItem(item.id, Direction::Down)),
            small("×", Message::RemoveItem(item.id)),
        ]
        .spacing(6)
        .align_y(Center)
        .into()
    }
}
