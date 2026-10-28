//! In-house drag and drop (ADR 0001): the drag state, drop targets, the global cursor
//! subscription, the ghost and autoscroll. `app::update` applies the drop.
use std::time::Duration;

use iced::widget::{self, MouseArea, container, mouse_area, operation, pin, selector, space, text};
use iced::{Element, Event, Fill, Point, Subscription, Task, Theme, Vector, event, mouse, time};

use crate::app::Message;
use crate::domain::Id;

/// Movement in px after a press before it is a drag; less is a click.
pub const THRESHOLD: f32 = 5.0;
/// Autoscroll timer period.
const TICK: Duration = Duration::from_millis(16);

/// Autoscroll starts this close (px) to the Board's left/right or a Column's top/bottom edge.
pub const EDGE: f32 = 40.0;
/// Autoscroll step per tick (px) at the very edge; it falls off linearly to 0 at [`EDGE`].
const MAX_STEP: f32 = 12.0;

/// The autoscroll step for a cursor at `at` along an area from `start` spanning `len`:
/// negative near the start, positive near the end, 0 elsewhere and outside the area.
pub fn edge_speed(at: f32, start: f32, len: f32) -> f32 {
    let (from_start, from_end) = (at - start, start + len - at);
    if from_start < 0.0 || from_end < 0.0 {
        0.0
    } else if from_start < EDGE {
        -MAX_STEP * (EDGE - from_start) / EDGE
    } else if from_end < EDGE {
        MAX_STEP * (EDGE - from_end) / EDGE
    } else {
        0.0
    }
}

/// What is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Card(Id),
    /// A Column, dragged by its header.
    Column(Id),
    /// A sidebar Board.
    Board(Id),
}

/// A drop target under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The Card slot before the Card now at `index` in `column`.
    Card { column: Id, index: usize },
    /// A Column's body outside its Cards: the slot after its last Card.
    Column(Id),
    /// The gap before the current Board's Column at `.0` (`len` = after the last).
    ColumnGap(usize),
    /// The gap before the active Board at `.0` in the sidebar (`len` = after the last).
    BoardGap(usize),
    /// A sidebar Board row other than the current one: moves the Card to that Board.
    Board(Id),
}

/// A press on an item: a drag once the cursor has moved [`THRESHOLD`] px.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    pub item: Item,
    /// The first cursor position seen after the press; the threshold is measured from it.
    // ponytail: press events carry no position, so the first move stands in for it (a px or two off).
    pub origin: Option<Point>,
    pub cursor: Option<Point>,
    /// Past the threshold: a drag, not a click.
    pub started: bool,
    /// Every target the cursor is over (a Card slot sits inside a Column body).
    pub hovered: Vec<Target>,
}

impl Drag {
    pub fn new(item: Item) -> Drag {
        Drag {
            item,
            origin: None,
            cursor: None,
            started: false,
            hovered: Vec::new(),
        }
    }

    pub fn moved(&mut self, to: Point) {
        let origin = *self.origin.get_or_insert(to);
        self.cursor = Some(to);
        self.started |= origin.distance(to) >= THRESHOLD;
    }

    pub fn hover(&mut self, target: Target) {
        if !self.hovered.contains(&target) {
            self.hovered.push(target);
        }
    }

    pub fn leave(&mut self, target: Target) {
        self.hovered.retain(|t| *t != target);
    }

    /// Where a release would drop: the innermost hovered target, `None` before the drag starts.
    pub fn target(&self) -> Option<Target> {
        if !self.started {
            return None;
        }
        let card_slot = self
            .hovered
            .iter()
            .find(|t| matches!(t, Target::Card { .. }));
        card_slot.or(self.hovered.first()).copied()
    }
}

/// While a press or drag is in progress: cursor moves and left releases anywhere in the
/// window (a `mouse_area` only sees releases over itself), plus the autoscroll timer once
/// the drag has started.
pub fn subscription(drag: Option<&Drag>) -> Subscription<Message> {
    let Some(drag) = drag else {
        return Subscription::none();
    };
    let mouse = event::listen_with(|event, _status, _window| match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::DragMoved(position)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::DragEnd),
        _ => None,
    });
    let tick = match drag.started {
        true => time::every(TICK).map(|_| Message::DragTick),
        false => Subscription::none(),
    };
    Subscription::batch([mouse, tick])
}

/// A draggable item. The release over it is also sent, in case the global subscription
/// started too late to see a quick click's release.
pub fn source<'a>(content: impl Into<Element<'a, Message>>, item: Item) -> MouseArea<'a, Message> {
    mouse_area(content)
        .on_press(Message::DragPress(item))
        .on_release(Message::DragEnd)
}

/// Makes `area` report itself as `target` while a press or drag is in progress. `on_move`
/// re-reports it, since a drag that starts over a target sees no `on_enter`.
pub fn target(
    area: MouseArea<'_, Message>,
    target: Target,
    dragging: bool,
) -> MouseArea<'_, Message> {
    if !dragging {
        return area;
    }
    area.on_enter(Message::DragHover(target))
        .on_move(move |_| Message::DragHover(target))
        .on_exit(Message::DragLeave(target))
}

/// The 2px drop indicator, always laid out (transparent unless `active`) so showing it
/// never shifts the targets under the cursor.
pub fn indicator<'a>(active: bool) -> Element<'a, Message> {
    container(space().width(Fill).height(2))
        .style(move |theme: &Theme| container::Style {
            background: active.then(|| theme.palette().primary.into()),
            ..Default::default()
        })
        .into()
}

/// The width of the gaps between Columns (and at both ends), which hold [`indicator_v`].
pub const GAP: f32 = crate::app::COLUMN_GAP;

/// The 2px vertical drop indicator, centred in a [`GAP`]-wide gap between Columns.
pub fn indicator_v<'a>(active: bool) -> Element<'a, Message> {
    let line =
        container(space().width(2).height(Fill)).style(move |theme: &Theme| container::Style {
            background: active.then(|| theme.palette().primary.into()),
            ..Default::default()
        });
    container(line).width(GAP).height(Fill).center_x(GAP).into()
}

/// The floating copy of the dragged item under the cursor; it sets no mouse interaction,
/// so hover still reaches the targets below. Goes on top of the view in a `stack`.
pub fn ghost(title: &str, cursor: Point) -> Element<'_, Message> {
    let card = container(text(title))
        .width(264)
        .padding(8)
        .style(|theme: &Theme| {
            let base = container::bordered_box(theme);
            container::Style {
                border: base.border.color(theme.palette().primary),
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..base
            }
        });
    pin(card).position(cursor - Vector::new(24.0, 16.0)).into()
}

/// Scrolls the Board (`board`, horizontally) and the hovered Column's Cards (`cards`,
/// vertically) when the cursor is within [`EDGE`] px of their edges.
pub fn autoscroll(cursor: Point, board: widget::Id, cards: Option<widget::Id>) -> Task<Message> {
    let cards = cards.map_or(Task::none(), |id| scroll_near_edge(id, cursor, false));
    Task::batch([scroll_near_edge(board, cursor, true), cards])
}

fn scroll_near_edge(id: widget::Id, cursor: Point, horizontal: bool) -> Task<Message> {
    selector::find(id.clone()).and_then(move |area| {
        let Some(b) = area.visible_bounds().filter(|b| b.contains(cursor)) else {
            return Task::none();
        };
        let by = if horizontal {
            Vector::new(edge_speed(cursor.x, b.x, b.width), 0.0)
        } else {
            Vector::new(0.0, edge_speed(cursor.y, b.y, b.height))
        };
        if by == Vector::ZERO {
            return Task::none();
        }
        operation::scroll_by(id.clone(), operation::AbsoluteOffset { x: by.x, y: by.y })
    })
}
