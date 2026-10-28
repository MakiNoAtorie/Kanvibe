//! Search (spec v2 item 2): a view in place of the Columns listing Cards from every Board,
//! active or archived, that match typed text. [`hits`] is plain domain code; picking a hit
//! opens its Board on the Card (or its Archive view). Leaving forgets the text.
use iced::widget::{button, column, container, mouse_area, operation, row, scrollable, space};
use iced::widget::{text, text_input};
use iced::{Center, Element, Fill, Task, mouse};

use crate::app::{App, Direction, HEADING, Message, Selection, card_style, clipped_text};
use crate::domain::{Board, Card, Data, Id};
use crate::filter::Filter;

/// The search box's widget Id.
pub const INPUT: &str = "search";

/// The open Search view (`App.search`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Search {
    pub text: String,
    /// The hit Up/Down move through and Enter picks; clamped to the hits when used.
    pub highlight: usize,
}

/// One matching Card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit<'a> {
    pub board: &'a Board,
    pub card: &'a Card,
    /// The Card's Column name; `None` = the Board's Archive.
    pub column: Option<&'a str>,
}

/// Cards on every Board whose title, description or Label names contain `text`
/// (case-insensitive, trimmed; the Filter's text rule), in Board order, then Column order with
/// the Archive last, then Card order. Nothing for fewer than two trimmed characters.
pub fn hits<'a>(data: &'a Data, text: &str) -> Vec<Hit<'a>> {
    if text.trim().chars().count() < 2 {
        return Vec::new();
    }
    let filter = Filter {
        text: text.into(),
        labels: Vec::new(),
    };
    let mut hits = Vec::new();
    for board in &data.boards {
        let active = (board.columns.iter()).flat_map(|c| {
            c.cards
                .iter()
                .map(move |card| (card, Some(c.name.as_str())))
        });
        let archived = board.archived_cards.iter().map(|a| (&a.card, None));
        hits.extend(
            active
                .chain(archived)
                .filter(|(card, _)| filter.matches(card, &board.labels))
                .map(|(card, column)| Hit {
                    board,
                    card,
                    column,
                }),
        );
    }
    hits
}

impl App {
    pub(crate) fn update_search(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSearch => {
                self.commit_pending();
                // Hidden menus would still take an Esc.
                self.column_menu = None;
                self.limit_input = None;
                self.board_menu = false;
                self.filter_menu = false;
                self.search.get_or_insert_default();
                return operation::focus(INPUT);
            }
            Message::SearchChanged(text) => {
                if let Some(search) = &mut self.search {
                    *search = Search { text, highlight: 0 };
                }
            }
            Message::SearchMove(direction) => {
                if let Some(search) = &mut self.search {
                    let last = hits(&self.data, &search.text).len().saturating_sub(1);
                    search.highlight = match direction {
                        Direction::Up => search.highlight.saturating_sub(1),
                        _ => search.highlight + 1,
                    }
                    .min(last);
                }
            }
            Message::CloseSearch => self.search = None,
            _ => {}
        }
        Task::none()
    }

    /// Opens the hit's Board on `card`: selected and open, or revealed in the Archive view if
    /// archived. Nothing for a Card on an archived Board. Runs outside `apply`, as nested
    /// `update`s, so the Board switch's `validate_ui` doesn't undo the Selection.
    pub(crate) fn pick(&mut self, card: Id) -> Task<Message> {
        let column = self.data.column_of_card(card);
        let board = (self.data.board_of_card(card))
            .or(self.data.board_of_archived(card))
            .filter(|b| self.data.active_board(*b).is_some());
        let Some(board) = board else {
            return Task::none();
        };
        self.search = None;
        let _ = self.update(Message::OpenBoard(board));
        match column {
            Some(column) => {
                self.archive = None;
                let card = Some(card);
                let reveal = self.update(Message::Select(Selection { column, card }));
                Task::batch([reveal, self.update(Message::OpenCard)])
            }
            None => {
                let _ = self.update(Message::OpenArchive);
                crate::app::reveal(
                    crate::archive::SCROLL.into(),
                    crate::archive::row_widget(card),
                    false,
                )
            }
        }
    }

    pub(crate) fn view_search(&self) -> Element<'_, Message> {
        let Some(search) = &self.search else {
            return space().into();
        };
        let header = row![
            text("Search").size(20),
            space::horizontal(),
            crate::chrome::tip(
                button("Back to Board").on_press(Message::CloseSearch),
                "Back to Board (Esc)"
            ),
        ]
        .align_y(Center);
        let hits = hits(&self.data, &search.text);
        let highlight = search.highlight.min(hits.len().saturating_sub(1));
        let pick = |hit: &Hit| (!hit.board.archived).then_some(Message::SearchPick(hit.card.id));
        let input = text_input("Search every Board", &search.text)
            .id(INPUT)
            .on_input(Message::SearchChanged)
            .on_submit_maybe(hits.get(highlight).and_then(pick))
            .padding(8);
        let body: Element<_> = if search.text.trim().chars().count() < 2 {
            text("Type at least two characters")
                .style(text::secondary)
                .into()
        } else if hits.is_empty() {
            text("No Cards match").style(text::secondary).into()
        } else {
            let groups = hits.chunk_by(|a, b| a.board.id == b.board.id);
            let mut i = 0;
            let groups = groups.map(|group| {
                let heading = text(group[0].board.name.as_str()).font(HEADING);
                let rows = group.iter().map(|hit| {
                    i += 1;
                    hit_row(*hit, i - 1 == highlight, pick(hit))
                });
                column![heading].extend(rows).spacing(6).into()
            });
            let groups: Vec<Element<_>> = groups.collect();
            scrollable(column(groups).spacing(16).padding([0, 12])).into()
        };
        column![header, input, body].spacing(12).padding(12).into()
    }
}

fn hit_row<'a>(hit: Hit<'a>, highlighted: bool, pick: Option<Message>) -> Element<'a, Message> {
    let place = hit.column.unwrap_or("Archive");
    let archived = hit.column.is_none() || hit.board.archived;
    let line = row![
        clipped_text(hit.card.title.as_str()).width(Fill),
        text(place).style(text::secondary),
    ]
    .extend(crate::labels::chips(hit.board, hit.card))
    .extend(archived.then(|| text("archived").style(text::secondary).into()))
    .spacing(8)
    .align_y(Center);
    let line = container(line)
        .padding(8)
        .width(Fill)
        .style(crate::app::outlined(card_style, highlighted));
    match pick {
        Some(message) => mouse_area(line)
            .on_press(message)
            .interaction(mouse::Interaction::Pointer)
            .into(),
        None => crate::chrome::tip(line, "Restore the Board first"),
    }
}
