//! Due date (spec v2 item 1): an optional calendar date on a Card, typed in the open Card and
//! shown as a badge on its face.
use jiff::civil::Date;

use iced::widget::{container, row, text, text_input};
use iced::{Border, Element, Theme};

use crate::app::{App, Field, Message};
use crate::domain::{Card, Data, Id};

/// Today in the system time zone, read when the Board is drawn.
pub fn today() -> Date {
    jiff::Zoned::now().date()
}

impl App {
    /// The open Card's "Due" row: the typed text while editing, else the stored ISO date.
    pub(crate) fn view_due<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let value = match &self.editing {
            Some(edit) if edit.field == Field::Due(card.id) => edit.text.as_str(),
            _ => card.due.as_deref().unwrap_or(""),
        };
        let id = card.id;
        let input = text_input("Due date", value)
            .id(INPUT)
            .size(14)
            .on_input(move |t| Message::DueChanged(id, t))
            .on_submit(Message::CommitEdit);
        row![text("Due").size(14), input]
            .spacing(8)
            .align_y(iced::Center)
            .into()
    }
}

/// The face's due badge: neutral, warning when due today, danger when overdue. `None` when
/// the Card has no (readable) due date. Shared by the Board view and the image export.
pub(crate) fn badge<'a>(card: &Card) -> Option<Element<'a, Message>> {
    let due = card.due.as_deref()?;
    let today = today();
    let label = badge_text(due, today)?;
    let status = status(due, today)?;
    let badge = container(text(label).size(12))
        .padding([1, 7])
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            let (background, text) = match status {
                Status::Future => (
                    palette.background.weaker.color,
                    palette.background.weaker.text.scale_alpha(0.75),
                ),
                Status::Today => (palette.warning.base.color, palette.warning.base.text),
                Status::Overdue => (palette.danger.base.color, palette.danger.base.text),
            };
            container::Style {
                background: Some(background.into()),
                text_color: Some(text),
                border: Border::default().rounded(10),
                ..container::Style::default()
            }
        });
    Some(badge.into())
}

/// Widget Id of the open Card's Due field.
pub const INPUT: &str = "due";

impl Data {
    /// Sets or clears the Card's due date (already ISO, from [`parse`]); a change touches the
    /// Card. `None` for an unknown Card.
    pub fn set_due(&mut self, card: Id, due: Option<String>) -> Option<()> {
        let columns = self.boards.iter_mut().flat_map(|b| &mut b.columns);
        let found = columns.flat_map(|c| &mut c.cards).find(|c| c.id == card)?;
        if found.due != due {
            found.due = due;
            self.touch(card);
        }
        Some(())
    }
}

/// The typed text as an ISO date (`YYYY-MM-DD`), when it is `YYYY-MM-DD`, `DD/MM/YYYY` or
/// `DD-MM-YYYY` and a real calendar date; `None` otherwise (blank included).
pub fn parse(text: &str) -> Option<String> {
    let text = text.trim();
    // Digits as '9', so the shape check also proves every slice below is ASCII digits.
    let shape: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect();
    let (year, month, day) = match shape.as_str() {
        "9999-99-99" => (&text[0..4], &text[5..7], &text[8..10]),
        "99/99/9999" | "99-99-9999" => (&text[6..10], &text[3..5], &text[0..2]),
        _ => return None,
    };
    let date = jiff::civil::Date::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
    date.ok().map(|d| d.to_string())
}

/// How a due date stands against today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Future,
    Today,
    Overdue,
}

/// How the stored ISO `due` stands against `today`; `None` when it isn't a date.
pub fn status(due: &str, today: Date) -> Option<Status> {
    Some(match due.parse::<Date>().ok()?.cmp(&today) {
        std::cmp::Ordering::Greater => Status::Future,
        std::cmp::Ordering::Equal => Status::Today,
        std::cmp::Ordering::Less => Status::Overdue,
    })
}

/// The badge on the Card face: "28 Oct", with the year only when it isn't `today`'s.
pub fn badge_text(due: &str, today: Date) -> Option<String> {
    let due = due.parse::<Date>().ok()?;
    let format = if due.year() == today.year() {
        "%-d %b"
    } else {
        "%-d %b %Y"
    };
    Some(due.strftime(format).to_string())
}
