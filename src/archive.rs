//! Archiving Cards, Remove Column and the Archive view (#11).
use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Center, Element, Fill};

use crate::app::{App, Confirm, Message, Selection};
use crate::domain::{Board, Column, Id};

/// One line of the Archive view.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub card: Id,
    pub title: String,
    /// The original Column's name, or "removed Column".
    pub column: String,
}

/// The Board's archived Cards, newest archived first.
pub fn rows(board: &Board) -> Vec<Row> {
    let name = |id| board.columns.iter().find(|c| c.id == id).map(|c| &c.name);
    let rows = board.archived_cards.iter().rev();
    rows.map(|a| Row {
        card: a.card.id,
        title: a.card.title.clone(),
        column: name(a.column_id).map_or("removed Column".into(), |n| n.clone()),
    })
    .collect()
}

/// The Column ⋯ menu item.
pub fn remove_label(column: &Column) -> String {
    let n = column.cards.len();
    format!(
        "Remove Column (archives {n} Card{})",
        if n == 1 { "" } else { "s" }
    )
}

/// The question for permanently deleting an archived Card.
pub fn delete_question(board: &Board, card: Id) -> Option<String> {
    let archived = board.archived_cards.iter().find(|a| a.card.id == card)?;
    Some(format!("Permanently delete \"{}\"?", archived.card.title))
}

impl App {
    /// Archives an active Card as one undo step. If it is the selected Card the selection moves
    /// to the Card below, else above, else the Column; its panel closes.
    pub(crate) fn archive_card(&mut self, card: Id) {
        self.commit_pending();
        let Some(board) = self.board_of_card(card) else {
            return;
        };
        let follow = self
            .selection
            .filter(|s| s.card == Some(card))
            .and_then(|s| {
                let cards = &self
                    .data
                    .boards
                    .iter()
                    .flat_map(|b| &b.columns)
                    .find(|c| c.id == s.column)?
                    .cards;
                let i = cards.iter().position(|c| c.id == card)?;
                let near = cards
                    .get(i + 1)
                    .or(i.checked_sub(1).and_then(|j| cards.get(j)));
                Some(Selection {
                    column: s.column,
                    card: near.map(|c| c.id),
                })
            });
        if self.data.archive_card(card).is_some() {
            self.selection = follow.or(self.selection);
            self.history
                .record(&self.data.boards, Some(board), Some(board));
        }
        if self.panel == Some(card) {
            self.panel = None;
        }
    }

    /// Removes a Column, archiving its Cards, as one undo step.
    pub(crate) fn remove_column(&mut self, column: Id) {
        self.commit_pending();
        self.column_menu = None;
        self.limit_input = None;
        let board = self.board_of_column(column);
        if self.data.remove_column(column).is_some() {
            self.history.record(&self.data.boards, board, board);
        }
    }

    /// Restores an archived Card (see `Data::restore_card`) as one undo step.
    pub(crate) fn restore_card(&mut self, card: Id) {
        self.commit_pending();
        let board = self.board_of_archived(card);
        if self.data.restore_card(card).is_some() {
            self.history.record(&self.data.boards, board, board);
        }
    }

    /// Opens the permanent-delete question (the shared `App.confirm`) for an archived Card.
    pub(crate) fn ask_delete(&mut self, card: Id) {
        self.commit_pending();
        let question = self.board().and_then(|b| delete_question(b, card));
        self.confirm = question.map(|prompt| Confirm {
            prompt,
            action: Message::DeleteCard(card),
        });
    }

    /// Deletes an archived Card for good, as one undo step.
    pub(crate) fn delete_card(&mut self, card: Id) {
        let board = self.board_of_archived(card);
        if self.data.delete_archived(card).is_some() {
            self.history.record(&self.data.boards, board, board);
        }
    }

    /// Opens the Archive view in place of the Columns; the panel and selection belong to the
    /// Columns, so they close.
    pub(crate) fn open_archive(&mut self) {
        self.commit_pending();
        self.column_menu = None;
        self.limit_input = None;
        self.panel = None;
        self.selection = None;
        self.archive = self.current_board;
    }

    /// Part of `validate_ui`: the Archive view belongs to the current Board, and the delete
    /// question to a Card still in its Archive.
    pub(crate) fn validate_archive(&mut self) {
        self.archive = self
            .archive
            .filter(|b| self.board().is_some_and(|c| c.id == *b));
        if let Some(Confirm {
            action: Message::DeleteCard(card),
            ..
        }) = &self.confirm
            && self.board_of_archived(*card).is_none()
        {
            self.confirm = None;
        }
    }

    fn board_of_archived(&self, card: Id) -> Option<Id> {
        let has = |b: &&Board| b.archived_cards.iter().any(|a| a.card.id == card);
        Some(self.data.boards.iter().find(has)?.id)
    }

    pub(crate) fn view_archive<'a>(&'a self, board: &'a Board) -> Element<'a, Message> {
        let header = row![
            text(&board.name).size(20),
            space::horizontal(),
            button("Back to Board").on_press(Message::CloseArchive),
        ]
        .align_y(Center);
        let rows = rows(board);
        let list: Element<_> = if rows.is_empty() {
            text("No archived Cards").into()
        } else {
            scrollable(column(rows.into_iter().map(archived_row)).spacing(6)).into()
        };
        column![header, list].spacing(12).padding(12).into()
    }
}

fn archived_row<'a>(row_: Row) -> Element<'a, Message> {
    let clipped = |s: String| container(text(s).wrapping(text::Wrapping::None)).clip(true);
    container(
        row![
            clipped(row_.title).width(Fill),
            clipped(row_.column).width(160),
            button("Restore").on_press(Message::Restore(row_.card)),
            button("Delete")
                .style(button::danger)
                .on_press(Message::AskDelete(row_.card)),
        ]
        .spacing(8)
        .align_y(Center),
    )
    .padding(8)
    .style(container::bordered_box)
    .into()
}
