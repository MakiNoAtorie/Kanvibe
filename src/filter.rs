//! The Filter on the current Board: text against titles and descriptions, any-of Labels.
//! [`Filter`], [`Filter::matches`] (the strict predicate, also for the image export) and
//! [`placement`] are plain domain code; `App::shown` adds the selected and just-created Cards.
//! The Filter is never saved and is cleared on a Board switch.
use iced::widget::{button, container, operation, row, text, text_input};
use iced::{Center, Element, Fill, Task};

use crate::app::{App, Message, Selection};
use crate::domain::{Board, Card, Column, Id, Label};

/// The filter box's widget Id.
pub const INPUT: &str = "filter";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    /// Matched case-insensitively, trimmed, against title and description.
    pub text: String,
    /// The Card must have any of these Labels; empty = no Label condition.
    pub labels: Vec<Id>,
}

impl Filter {
    /// Something is typed or a Label is chosen.
    pub fn is_on(&self) -> bool {
        !self.text.trim().is_empty() || !self.labels.is_empty()
    }

    /// Whether `card` passes both the text and the Labels (an off Filter passes everything).
    /// The text also matches the names of the Card's Labels among `labels` (its Board's).
    pub fn matches(&self, card: &Card, labels: &[Label]) -> bool {
        let text = self.text.trim().to_lowercase();
        let label_names = labels.iter().filter(|l| card.labels.contains(&l.id));
        let text_ok = text.is_empty()
            || card.title.to_lowercase().contains(&text)
            || card.description.to_lowercase().contains(&text)
            || label_names
                .into_iter()
                .any(|l| l.name.to_lowercase().contains(&text));
        let labels_ok =
            self.labels.is_empty() || card.labels.iter().any(|l| self.labels.contains(l));
        text_ok && labels_ok
    }

    /// `Column::count_label`, prefixed with "visible of " while the Filter is on.
    pub(crate) fn count_label(&self, column: &Column, shown: impl Fn(&Card) -> bool) -> String {
        if !self.is_on() {
            return column.count_label();
        }
        let visible = column.cards.iter().filter(|c| shown(c)).count();
        format!("{visible} of {}", column.count_label())
    }
}

/// The drop gap under a Filter for the slot before `cards[slot]` (`cards.len()` = below them
/// all): right after the visible Card above the slot, else the slot itself (before the first
/// visible Card), so hidden Cards keep their relative order.
pub fn placement(cards: &[Card], slot: usize, shown: impl Fn(&Card) -> bool) -> usize {
    let slot = slot.min(cards.len());
    cards[..slot]
        .iter()
        .rposition(shown)
        .map_or(slot, |i| i + 1)
}

impl App {
    pub(crate) fn update_filter(&mut self, message: Message) -> Task<Message> {
        if message == Message::LeaveField {
            // Every other text box keeps state that tells it was the one focused.
            let field_open = self.field_open()
                || self.label_picker.is_some()
                || self.shortcuts
                || self.search.is_some();
            return match field_open {
                true => self.update(Message::Escape),
                false => Task::none(), // the filter box: it has already unfocused
            };
        }
        self.commit_pending();
        match message {
            Message::FocusFilter => return operation::focus(INPUT),
            Message::SubmitFilter => {
                let board = self.board().into_iter().flat_map(|b| &b.columns);
                let mut cards = board.flat_map(|c| c.cards.iter().map(move |x| (c.id, x)));
                if let Some((column, card)) = cards.find(|(_, card)| self.shown(card)) {
                    let card = Some(card.id);
                    self.selection = Some(Selection { column, card });
                    // Leave the box so the arrow keys move the selection: focusing an Id no
                    // widget has unfocuses every widget (iced's `unfocus` needs "advanced").
                    let unfocus = operation::focus("no-widget");
                    return Task::batch([self.reveal_selection(), unfocus]);
                }
            }
            Message::ClearFilter => _ = self.clear_filter(),
            Message::ToggleFilterMenu => self.filter_menu = !self.filter_menu,
            Message::FilterChanged(text) => {
                self.filter.text = text;
                self.filter_kept.clear();
            }
            Message::FilterLabel(label) => {
                match self.filter.labels.iter().position(|l| *l == label) {
                    Some(i) => _ = self.filter.labels.remove(i),
                    None => self.filter.labels.push(label),
                }
                self.filter_kept.clear();
            }
            _ => {}
        }
        Task::none()
    }

    /// The Filter is on and no Card of the current Board matches it: "No Cards match".
    pub fn nothing_matches(&self) -> bool {
        let Some(board) = self.board() else {
            return false;
        };
        let mut cards = board.columns.iter().flat_map(|c| &c.cards);
        self.filter.is_on() && !cards.any(|c| self.filter.matches(c, &board.labels))
    }

    /// The Board header's filter controls: the text box with ✕, and the Labels button.
    /// `width` is the text box's: 220 px, or `Fill` on its own line in a narrow Board.
    pub(crate) fn filter_bar(&self, width: iced::Length) -> Element<'_, Message> {
        // Always laid out (invisible and inert while the Filter is off), so the text box
        // doesn't shift when typing starts.
        let on = self.filter.is_on();
        let clear = button("✕")
            .style(move |theme, status| match on {
                true => button::text(theme, status),
                false => button::Style {
                    text_color: iced::Color::TRANSPARENT,
                    ..button::Style::default()
                },
            })
            .on_press_maybe(on.then_some(Message::ClearFilter));
        let clear = match on {
            true => crate::chrome::tip(clear, "Clear filter"),
            false => clear.into(),
        };
        let labels = match self.filter.labels.len() {
            0 => "Labels".to_string(),
            n => format!("Labels ({n})"),
        };
        row![crate::chrome::tip(
            text_input("Filter (Ctrl+F)", &self.filter.text)
                .id(INPUT)
                .width(width)
                .style(raised_input)
                .on_input(Message::FilterChanged)
                .on_submit(Message::SubmitFilter),
            "Filter (Ctrl+F)"
        )]
        .push(clear)
        .push(
            button(text(labels).wrapping(text::Wrapping::None))
                .style(crate::app::quiet)
                .on_press(Message::ToggleFilterMenu),
        )
        .spacing(4)
        .align_y(Center)
        .into()
    }

    /// Under the Board header: the Labels filter list when open, and "No Cards match".
    pub(crate) fn filter_lines<'a>(&'a self, board: &'a Board) -> Vec<Element<'a, Message>> {
        let menu = self.filter_menu.then(|| {
            let rows: Element<_> = match board.labels.is_empty() {
                true => text("No Labels on this Board").size(12).into(),
                false => self.picker_rows(board, &self.filter.labels, Message::FilterLabel, None),
            };
            let list = container(rows)
                .width(300)
                .padding(8)
                .style(container::rounded_box);
            // Under the Labels button, at the header's right end.
            container(list).align_right(Fill).into()
        });
        let none = self.nothing_matches().then(|| {
            row![
                text("No Cards match."),
                button("Clear filter")
                    .style(button::text)
                    .on_press(Message::ClearFilter)
            ]
            .spacing(4)
            .align_y(Center)
            .into()
        });
        menu.into_iter().chain(none).collect()
    }

    /// Turns the Filter off; `false` when it was already off (the Esc layer is then skipped).
    pub(crate) fn clear_filter(&mut self) -> bool {
        let on = self.filter.is_on();
        self.filter = Filter::default();
        self.filter_kept.clear();
        on
    }

    /// The Column header count: `Column::count_label`, prefixed with "visible of " while the
    /// Filter is on. The Column Limit stays judged on the total (`Column::over_limit`).
    pub fn count_label(&self, column: &Column) -> String {
        self.filter.count_label(column, |c| self.shown(c))
    }

    /// Run after every update: filter Labels that are not the current Board's are dropped.
    pub(crate) fn validate_filter(&mut self) {
        let labels: Vec<Id> = self
            .board()
            .iter()
            .flat_map(|b| &b.labels)
            .map(|l| l.id)
            .collect();
        self.filter.labels.retain(|l| labels.contains(l));
    }

    /// Whether `card` shows on the Board: it matches the Filter, or is the selected Card, or was
    /// created while the Filter is on.
    pub fn shown(&self, card: &Card) -> bool {
        self.filter
            .matches(card, self.board().map_or(&[], |b| &b.labels))
            || self.selection.and_then(|s| s.card) == Some(card.id)
            || self.filter_kept.contains(&card.id)
    }
}

/// The filter box on the raised surface (as the header's buttons), with their weak edge.
fn raised_input(theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let bg = theme.extended_palette().background;
    let style = text_input::default(theme, status);
    let border = match status {
        text_input::Status::Active => style.border.color(bg.weak.color),
        _ => style.border,
    };
    text_input::Style {
        background: bg.weakest.color.into(),
        border: border.rounded(6),
        ..style
    }
}
