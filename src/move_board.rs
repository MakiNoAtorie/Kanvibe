//! Moving a Card to another Board (#16): the "Move to Board…" picker and the move itself.
use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Fill};

use crate::app::{App, Message, dialog, dialog_header};
use crate::domain::Id;

impl App {
    /// "Move to Board…" / M: opens the picker for the open Card, else the selected one.
    pub(crate) fn open_move(&mut self) {
        self.commit_pending();
        let selected = self.selection.and_then(|s| s.card);
        self.move_picker = self.panel.or(selected);
    }

    /// Moves `card` to the active Board `board` as one undo step (undo shows the source Board,
    /// redo the target). The user stays on the current Board. An invalid target does nothing.
    pub(crate) fn move_to_board(&mut self, card: Id, board: Id) {
        let source = self.data.board_of_card(card);
        if self.card_leaves(card, |data| data.move_to_board(card, board)) {
            self.move_picker = None;
            self.history.record(&self.data.boards, source, Some(board));
        }
    }

    /// The picker lives only while its Card is an active Card of the current Board.
    pub(crate) fn validate_move(&mut self) {
        self.move_picker = self.move_picker.filter(|card| self.on_board(*card));
    }

    /// The picker overlay: one button per other active Board.
    pub(crate) fn view_move_picker(&self, card: Id) -> Element<'_, Message> {
        let header = dialog_header("Move to Board", Message::CloseMove);
        let boards = self
            .data
            .boards
            .iter()
            .filter(|b| !b.archived && Some(b.id) != self.current_board);
        let rows = boards.map(|b| {
            button(text(b.name.as_str()))
                .style(button::text)
                .width(Fill)
                .on_press(Message::MoveToBoard(card, b.id))
                .into()
        });
        let rows: Vec<Element<_>> = rows.collect();
        let list: Element<_> = if rows.is_empty() {
            text("No other Boards").size(12).into()
        } else {
            scrollable(column(rows)).into()
        };
        let hint = text("The Card goes to the bottom of the Board's first Column.").size(12);
        dialog(container(column![header, hint, list].spacing(12)).width(320))
    }
}
