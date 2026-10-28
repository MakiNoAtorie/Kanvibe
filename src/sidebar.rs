//! The left sidebar: active Boards, the collapsed "Archived (n)" section and the footer.
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text};
use iced::{Border, Center, Color, Element, Fill, Padding, Theme};

use crate::app::{App, Message, board_name_input, clipped, clipped_text};
use crate::domain::{Board, Id};
use crate::drag::{self, Drag, Item, Target};

pub(crate) const WIDTH: u32 = 220;
pub(crate) const RAIL: u32 = 40;

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
    // Other Boards' rows take a dragged Card (#16); only a Card drag registers them.
    let card_drag = app
        .drag
        .as_ref()
        .filter(|d| matches!(d.item, Item::Card(_)));
    let boards = active
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let current = Some(b.id) == app.current_board;
            let row = board_row(b, current);
            let row = match card_drag {
                Some(drag) if !current => drop_row(row, b.id, drag),
                _ => row,
            };
            gap(i, row)
        })
        .chain([gap(active.len(), space().height(12).into())]);
    let archived_section = (!archived.is_empty()).then(|| {
        let chevron = if app.archived_open { "▾" } else { "▸" };
        let heading = button(text(format!("Archived ({}) {chevron}", archived.len())))
            .style(button::text)
            .on_press(Message::ToggleArchivedBoards);
        let rows = archived.iter().filter(|_| app.archived_open).map(|b| {
            // Indented to line up with the Board names above; muted, as they are archived.
            let name = text(b.name.as_str())
                .style(text::secondary)
                .wrapping(text::Wrapping::None);
            let row = row![
                clipped(name, text(b.name.as_str())).width(Fill),
                small("Restore", Message::RestoreBoard(b.id)),
                small("Delete", Message::AskDeleteBoard(b.id)),
            ]
            .spacing(4)
            .align_y(Center);
            container(row).padding(Padding::ZERO.left(10)).into()
        });
        column![heading].extend(rows).spacing(4)
    });
    // On the "No open Boards" screen the name prompt sits in the middle instead.
    let footer: Element<_> = match &app.board_name {
        Some(name) if app.current_board.is_some() => board_name_input(name).into(),
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
            // 13px with the footer's other occasional links; "+ New Board" stays full size.
            button(text("Import Board…").size(13))
                .style(button::text)
                .on_press(Message::ImportBoard),
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
    // The current Board is marked with the weak primary shade, not a solid fill that would
    // outshine the Board itself.
    let style = move |theme: &Theme, status| match current {
        true => {
            let pair = theme.extended_palette().primary.weak;
            button::Style {
                background: Some(pair.color.into()),
                text_color: pair.text,
                border: Border::default().rounded(4),
                ..button::Style::default()
            }
        }
        false => button::text(theme, status),
    };
    // No `on_press`: the button must not capture the press the drag source needs. A press
    // released under the drag threshold opens the Board (see `Message::DragEnd`).
    let row = button(clipped_text(board.name.as_str()).width(Fill))
        .style(move |theme, status| match status {
            button::Status::Disabled => style(theme, button::Status::Active),
            status => style(theme, status),
        })
        .width(Fill);
    drag::source(row, Item::Board(board.id)).into()
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

fn small(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(12))
        .style(crate::app::quiet)
        .padding([2, 6])
        .on_press(message)
        .into()
}
