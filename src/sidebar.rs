//! The left sidebar: active Boards, the collapsed "Archived (n)" section and the footer.
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input, tooltip,
};
use iced::{Center, Element, Fill};

use crate::app::{App, BOARD_NAME_INPUT, Message};
use crate::domain::Board;
use crate::drag::{self, Drag, Item, Target};

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
    // Gaps before each active Board and after the last; only a Board drag registers them.
    let dragged = matches!(app.drag.as_ref().map(|d| d.item), Some(Item::Board(_)));
    let hovered = app.drag.as_ref().and_then(Drag::target);
    let gap = |i: usize, content| gap(i, content, hovered, dragged);
    let boards = active
        .iter()
        .enumerate()
        .map(|(i, b)| gap(i, board_row(b, Some(b.id) == app.current_board)))
        .chain([gap(active.len(), space().height(12).into())]);
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

/// A drop gap before the active Board at `i`, holding `content` below its indicator.
fn gap<'a>(
    i: usize,
    content: Element<'a, Message>,
    hovered: Option<Target>,
    dragged: bool,
) -> Element<'a, Message> {
    let slot = column![
        drag::indicator(hovered == Some(Target::BoardGap(i))),
        content
    ];
    drag::target(mouse_area(slot), Target::BoardGap(i), dragged).into()
}

/// An active Board's row: its name, highlighted when current; a click opens it.
/// Kept small so drag and drop can wrap it as a drop target.
pub fn board_row(board: &Board, current: bool) -> Element<'_, Message> {
    let style = if current {
        button::primary
    } else {
        button::text
    };
    // No `on_press`: the button must not capture the press the drag source needs. A press
    // released under the drag threshold opens the Board (see `Message::DragEnd`).
    let row = button(clipped(&board.name))
        .style(move |theme, status| match status {
            button::Status::Disabled => style(theme, button::Status::Active),
            status => style(theme, status),
        })
        .width(Fill);
    drag::source(row, Item::Board(board.id)).into()
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
