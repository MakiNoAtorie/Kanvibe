//! The Card panel: 400px on the right of the Board, showing the open Card.
use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::{App, Field, Message};
use crate::chrome::tip;
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
        // The title takes the row; ✕ stays at its top right however many lines it wraps to.
        let header = row![
            self.editable_name(Field::CardTitle(card.id), &card.title, 20),
            tip(
                button("✕")
                    .style(button::text)
                    .on_press(Message::ClosePanel),
                "Close (Esc)"
            )
        ]
        .spacing(8);
        container(
            column![
                header,
                self.labels_row(card),
                self.view_description(card),
                self.view_checklist(card),
                row![
                    tip(
                        button("Move to Board…")
                            .style(crate::app::quiet)
                            .on_press(Message::OpenMove),
                        "Move to Board (M)"
                    ),
                    tip(
                        button("Archive Card")
                            .style(crate::app::quiet)
                            .on_press(Message::ArchiveCard(card.id)),
                        "Archive Card (Del)"
                    ),
                ]
                .spacing(8),
                text(times).size(12),
            ]
            .spacing(16),
        )
        .width(WIDTH)
        .height(Fill)
        .padding(12)
        .style(container::bordered_box)
        .into()
    }
}
