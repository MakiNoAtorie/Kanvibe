//! The left sidebar: active Boards, the collapsed "Archived (n)" section and the footer.
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input, tooltip,
};
use iced::{Border, Center, Color, Element, Fill, Theme};

use crate::app::{App, BOARD_NAME_INPUT, Message};
use crate::domain::{Board, Id};
use crate::drag::{self, Drag, Target};

const WIDTH: u32 = 220;
const RAIL: u32 = 40;

pub fn view(app: &App) -> Element<'_, Message> {
    let toggle = |label| {
        button(text(label))
            .style(button::text)
            .on_press(Message::ToggleSidebar)
    };
    if app.sidebar_collapsed {
        return container(toggle("»"))
            .width(RAIL)
            .height(Fill)
            .padding(4)
            .style(container::bordered_box)
            .into();
    }
    let (archived, active): (Vec<&Board>, Vec<&Board>) =
        app.data.boards.iter().partition(|b| b.archived);
    let boards = active.iter().map(|b| {
        let current = Some(b.id) == app.current_board;
        let row = board_row(b, current);
        match &app.drag {
            Some(drag) if !current => drop_row(row, b.id, drag),
            _ => row,
        }
    });
    let archived_section = (!archived.is_empty()).then(|| {
        let heading = button(text(format!("Archived ({})", archived.len())))
            .style(button::text)
            .on_press(Message::ToggleArchivedBoards);
        let rows = archived.iter().filter(|_| app.archived_open).map(|b| {
            row![
                clipped(&b.name),
                small("Restore", Message::RestoreBoard(b.id)),
                small("Delete", Message::AskDeleteBoard(b.id)),
            ]
            .spacing(4)
            .align_y(Center)
            .into()
        });
        column![heading].extend(rows).spacing(4)
    });
    // On the "No open Boards" screen the name prompt sits in the middle instead.
    let footer: Element<_> = match &app.board_name {
        Some(name) if app.current_board.is_some() => text_input("Board name", name)
            .id(BOARD_NAME_INPUT)
            .on_input(Message::BoardNameChanged)
            .on_submit(Message::CreateBoard)
            .into(),
        _ => button("+ New Board")
            .style(button::text)
            .on_press(Message::NewBoard)
            .into(),
    };
    let list = column![column(boards).spacing(2)].extend(archived_section.map(Into::into));
    container(
        column![
            row![space::horizontal(), toggle("«")],
            scrollable(list.spacing(12)).height(Fill),
            footer,
            crate::chrome::footer(app),
        ]
        .spacing(8),
    )
    .width(WIDTH)
    .height(Fill)
    .padding(8)
    .style(container::bordered_box)
    .into()
}

/// An active Board's row: its name, highlighted when current; a click opens it.
/// Kept small so drag and drop can wrap it as a drop target.
pub fn board_row(board: &Board, current: bool) -> Element<'_, Message> {
    let style = if current {
        button::primary
    } else {
        button::text
    };
    button(clipped(&board.name))
        .style(style)
        .width(Fill)
        .on_press(Message::OpenBoard(board.id))
        .into()
}

/// A Board row as the drop target of a dragged Card (#16), outlined while it is hovered.
fn drop_row<'a>(row: Element<'a, Message>, board: Id, drag: &Drag) -> Element<'a, Message> {
    let target = Target::Board(board);
    let hovered = drag.started && drag.hovered.contains(&target);
    let outlined = container(row).style(move |theme: &Theme| container::Style {
        border: Border {
            color: if hovered {
                theme.palette().primary
            } else {
                Color::TRANSPARENT
            },
            width: 2.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });
    drag::target(mouse_area(outlined), target, true).into()
}

/// One line, clipped, with the full name in a tooltip.
fn clipped(name: &str) -> Element<'_, Message> {
    let full = container(text(name))
        .padding(4)
        .style(container::rounded_box);
    let label = tooltip(
        text(name).wrapping(text::Wrapping::None),
        full,
        tooltip::Position::Bottom,
    );
    container(label).width(Fill).clip(true).into()
}

fn small(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(12))
        .style(button::secondary)
        .padding([2, 6])
        .on_press(message)
        .into()
}
