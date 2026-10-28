//! The Checklist section of the open Card.
use iced::widget::{button, checkbox, mouse_area, row, text, text_input};
use iced::{Center, Element, Fill};

use crate::app::{App, Direction, EDIT_INPUT, Field, Message, Part};
use crate::chrome::tip;
use crate::domain::{Card, Item};

impl App {
    pub(crate) fn view_checklist<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let items = card.checklist.iter().map(|item| self.view_item(item));
        let heading = match card.progress() {
            Some(p) => format!("Checklist {p}"),
            None => "Checklist".into(),
        };
        let add = crate::app::small_button("+ Add Checklist Item", Message::AddItem(card.id));
        let body = items.chain([self.part_box(Part::Checklist, add)]);
        crate::app::section(crate::app::section_heading(heading), body)
    }

    fn view_item<'a>(&'a self, item: &'a Item) -> Element<'a, Message> {
        let field = Field::ChecklistItem(item.id);
        let label: Element<_> = match &self.editing {
            Some(edit) if edit.field == field => text_input("Checklist Item", &edit.text)
                .id(EDIT_INPUT)
                .on_input(Message::EditChanged)
                .on_submit(Message::SubmitItem(item.id))
                .into(),
            _ => mouse_area(
                text(&item.text)
                    .size(crate::app::SECTION_TEXT_SIZE)
                    .width(Fill),
            )
            .on_press(Message::StartEdit(field))
            .into(),
        };
        let small = |label, message| {
            button(text(label).size(12).style(text::secondary))
                .style(button::text)
                .on_press(message)
        };
        let row = row![
            checkbox(item.done).on_toggle(|_| Message::ToggleItem(item.id)),
            label,
            tip(
                small("▲", Message::MoveItem(item.id, Direction::Up)),
                "Move up"
            ),
            tip(
                small("▼", Message::MoveItem(item.id, Direction::Down)),
                "Move down"
            ),
            tip(
                small("✕", Message::RemoveItem(item.id)),
                "Remove Checklist Item"
            ),
        ]
        .spacing(6)
        .align_y(Center);
        self.part_box(Part::Item(item.id), row)
    }
}
