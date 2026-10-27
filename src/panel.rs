//! The Card panel: 400px on the right of the Board, showing the open Card.
use iced::widget::{button, column, container, row, space, text};
use iced::{Center, Element, Fill};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::{App, Field, Message};
use crate::domain::Card;

pub const WIDTH: f32 = 400.0;

/// An RFC 3339 timestamp as "YYYY-MM-DD HH:MM" in `tz`; text that doesn't parse is shown as is.
pub fn local_time(timestamp: &str, tz: &TimeZone) -> String {
    match timestamp.parse::<Timestamp>() {
        Ok(t) => t
            .to_zoned(tz.clone())
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
        Err(_) => timestamp.into(),
    }
}

impl App {
    pub(crate) fn view_panel<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let tz = TimeZone::system();
        let times = format!(
            "Created {} · Updated {}",
            local_time(&card.created, &tz),
            local_time(&card.updated, &tz)
        );
        let header = row![
            space::horizontal(),
            button("✕")
                .style(button::text)
                .on_press(Message::ClosePanel)
        ]
        .align_y(Center);
        container(
            column![
                header,
                self.editable_name(Field::CardTitle(card.id), &card.title, 20),
                self.view_description(card),
                text(times).size(12),
            ]
            .spacing(12),
        )
        .width(WIDTH)
        .height(Fill)
        .padding(12)
        .style(container::bordered_box)
        .into()
    }
}
