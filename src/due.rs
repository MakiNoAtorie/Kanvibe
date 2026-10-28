//! Due date (spec v2 item 1): an optional calendar date on a Card, typed in the open Card and
//! shown as a badge on its face.
use jiff::civil::Date;

use iced::Element;
use iced::widget::text_input;

use crate::app::{App, Field, Message, Tone, pill};
use crate::domain::{Card, Data, Id};

/// `now`'s date in the system time zone: "today" for the due badges of one view.
pub fn today(now: jiff::Timestamp) -> Date {
    now.to_zoned(jiff::tz::TimeZone::system()).date()
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
            .size(crate::app::SECTION_TEXT_SIZE)
            .padding([3, 6])
            .on_input(move |t| Message::DueChanged(id, t))
            .on_submit(Message::CommitEdit);
        crate::app::section(crate::app::section_heading("Due"), [input.into()])
    }
}

/// The face's due badge: neutral, warning when due today, danger when overdue. `None` when
/// the Card has no (readable) due date. Shared by the Board view and the image export.
pub(crate) fn badge<'a>(card: &Card, today: Date) -> Option<Element<'a, Message>> {
    let due = card.due.as_deref()?;
    let tone = match status(due, today)? {
        Status::Future => Tone::Neutral,
        Status::Today => Tone::Warning,
        Status::Overdue => Tone::Danger,
    };
    Some(pill(badge_text(due, today)?, tone))
}

/// Widget Id of the open Card's Due field.
pub const INPUT: &str = "due";

impl Data {
    /// Sets or clears the Card's due date (already ISO, from [`parse`]); a change touches the
    /// Card. `None` for an unknown Card.
    pub fn set_due(&mut self, card: Id, due: Option<String>) -> Option<()> {
        let found = self.card_mut(card)?;
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
