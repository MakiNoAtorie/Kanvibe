//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::mouse_area;
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input, tooltip,
};
use iced::{Center, Element, Event, Fill, Subscription, Task, event, keyboard};
use iced::{time, window};

use crate::domain::{Board, Column, Data, Id};
use crate::history::History;
use crate::storage::{self, Settings};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const BOARD_NAME_INPUT: &str = "board-name";
const CAPTURE_INPUT: &str = "capture";
const EDIT_INPUT: &str = "edit";
const LIMIT_INPUT: &str = "limit";
/// How long after the last keystroke a pending edit's live text is written to disk.
const IDLE_WRITE: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct App {
    pub data: Data,
    pub current_board: Option<Id>,
    /// The new-Board name prompt; `Some` while it is open.
    pub board_name: Option<String>,
    /// The Capture box; at most one is open.
    pub capture: Option<Capture>,
    /// Undo history of the Boards; in memory only.
    pub history: History,
    /// Where `kanvibe.json` lives; `None` (the default) saves nothing.
    pub data_dir: Option<PathBuf>,
    /// What the data file last saved holds, so only changes are saved.
    pub saved: (Data, Settings),
    /// The rename in progress. At most one text edit is pending: this or `capture`.
    /// See `commit_pending`.
    pub editing: Option<PendingEdit>,
    /// The Column whose header ⋯ menu is open.
    pub column_menu: Option<Id>,
    /// The "Set limit…" prompt; stays open until a valid value is submitted or Esc.
    pub limit_input: Option<LimitInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitInput {
    pub column: Id,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub column: Id,
    pub text: String,
}

/// A text field whose edits go straight into the Boards and become one undo step on commit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Field {
    BoardName(Id),
    ColumnName(Id),
}

impl Field {
    fn id(self) -> Id {
        match self {
            Field::BoardName(id) | Field::ColumnName(id) => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingEdit {
    pub field: Field,
    /// The value when editing started; a blank text falls back to it.
    pub start: String,
    /// What the text box shows, as typed.
    pub text: String,
    /// The last keystroke not yet written to disk; saving waits until it is 1 s old.
    pub typed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum Message {
    NewBoard,
    BoardNameChanged(String),
    CreateBoard,
    OpenCapture(Id),
    CaptureChanged(String),
    SubmitCapture,
    Escape,
    /// A key press not handled elsewhere; Ctrl+Z undoes, Ctrl+Y / Ctrl+Shift+Z redo.
    Key(keyboard::Event),
    /// Double-click on a name: commits any pending edit and starts editing this field.
    StartEdit(Field),
    EditChanged(String),
    /// Enter in a single-line field.
    CommitEdit,
    /// Timer while a pending edit has unwritten keystrokes; carries the current time.
    EditTick(Instant),
    /// The window's close button: commits the pending edit, saves, exits.
    CloseRequested,
    /// "+ Add Column" on this Board.
    AddColumn(Id),
    /// The ⋯ button in a Column header.
    ToggleColumnMenu(Id),
    /// "Set limit…" in the Column menu.
    OpenLimit(Id),
    LimitChanged(String),
    SubmitLimit,
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.history.sync(&self.data.boards);
        match message {
            Message::NewBoard => {
                self.commit_pending();
                self.board_name = Some(String::new());
                return operation::focus(BOARD_NAME_INPUT);
            }
            Message::BoardNameChanged(name) => {
                if let Some(draft) = &mut self.board_name {
                    *draft = name;
                }
            }
            Message::CreateBoard => {
                let name = self.board_name.as_deref().unwrap_or_default();
                if let Some(id) = self.data.create_board(name) {
                    self.current_board = Some(id);
                    self.board_name = None;
                    self.history.record(&self.data.boards, None, None);
                }
            }
            Message::OpenCapture(column) => {
                self.commit_pending();
                self.capture = Some(Capture {
                    column,
                    text: String::new(),
                });
                return operation::focus(CAPTURE_INPUT);
            }
            Message::CaptureChanged(text) => {
                if let Some(capture) = &mut self.capture {
                    capture.text = text;
                }
            }
            Message::SubmitCapture => {
                if let Some(column) = self.capture.as_ref().map(|c| c.column) {
                    self.commit_pending();
                    self.capture = Some(Capture {
                        column,
                        text: String::new(),
                    });
                }
            }
            Message::Escape => {
                self.board_name = None;
                self.column_menu = None;
                self.limit_input = None;
                self.capture = None; // Esc discards the capture box; other fields commit.
                self.commit_pending();
            }
            Message::Key(event) => {
                if undo_shortcut(&event).is_some() {
                    self.commit_pending();
                }
                if let Some((boards, show)) = match undo_shortcut(&event) {
                    Some(false) => self.history.undo(),
                    Some(true) => self.history.redo(),
                    None => None,
                } {
                    self.data.boards = boards;
                    if show.is_some() {
                        self.current_board = show;
                    }
                }
            }
            Message::StartEdit(field) => {
                self.commit_pending();
                let id = field.id();
                if let Some(start) = self.data.name(id) {
                    self.editing = Some(PendingEdit {
                        field,
                        start: start.into(),
                        text: start.into(),
                        typed_at: None,
                    });
                    return operation::focus(EDIT_INPUT);
                }
            }
            Message::EditChanged(text) => {
                if let Some(edit) = &mut self.editing {
                    let live = if text.trim().is_empty() {
                        &edit.start
                    } else {
                        &text
                    };
                    self.data.rename(edit.field.id(), live);
                    edit.text = text;
                    edit.typed_at = Some(Instant::now());
                }
            }
            Message::CommitEdit => self.commit_pending(),
            Message::EditTick(now) => {
                if let Some(edit) = &mut self.editing
                    && edit.typed_at.is_some_and(|t| now >= t + IDLE_WRITE)
                {
                    edit.typed_at = None; // lets save_if_changed write the live text
                }
            }
            Message::CloseRequested => {
                self.commit_pending();
                // ponytail: exits even if the save fails; #21 adds the Retry / Save a copy / Quit dialog.
                return iced::exit();
            }
            Message::AddColumn(board) => {
                self.commit_pending();
                if self.data.add_column(board).is_some() {
                    self.history
                        .record(&self.data.boards, Some(board), Some(board));
                }
            }
            Message::ToggleColumnMenu(column) => {
                self.commit_pending();
                self.limit_input = None;
                self.column_menu = (self.column_menu != Some(column)).then_some(column);
            }
            Message::OpenLimit(column) => {
                self.commit_pending();
                self.column_menu = None;
                if let Some(col) = self.find_column(column) {
                    let text = col.limit.map(|l| l.to_string()).unwrap_or_default();
                    self.limit_input = Some(LimitInput { column, text });
                    return operation::focus(LIMIT_INPUT);
                }
            }
            Message::LimitChanged(text) => {
                if let Some(input) = &mut self.limit_input {
                    input.text = text;
                }
            }
            Message::SubmitLimit => {
                if let Some(LimitInput { column, text }) = self.limit_input.clone()
                    && self.data.set_limit(column, &text).is_some()
                {
                    self.limit_input = None;
                    let board = self.board_of_column(column);
                    self.history.record(&self.data.boards, board, board);
                }
            }
        }
        Task::none()
    }

    /// Commits the text edit in progress, if any, as one undo step (none if nothing changed),
    /// and closes its field. A Capture box adds its Card. Call it first in every arm that
    /// changes domain data or UI context.
    pub fn commit_pending(&mut self) {
        if let Some(capture) = self.capture.take() {
            self.data.add_card(capture.column, &capture.text);
            let board = self.board_of_column(capture.column);
            self.history.record(&self.data.boards, board, board);
        }
        if let Some(edit) = self.editing.take() {
            let board = match edit.field {
                Field::BoardName(_) => None,
                Field::ColumnName(column) => self.board_of_column(column),
            };
            self.history.record(&self.data.boards, board, board);
        }
    }

    fn find_column(&self, column: Id) -> Option<&Column> {
        self.data
            .boards
            .iter()
            .flat_map(|b| &b.columns)
            .find(|c| c.id == column)
    }

    fn board_of_column(&self, column: Id) -> Option<Id> {
        let board = self
            .data
            .boards
            .iter()
            .find(|b| b.columns.iter().any(|c| c.id == column))?;
        Some(board.id)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // ponytail: Esc only, ignoring capture status (a focused text_input swallows Esc);
        // the key-mapping module replaces this.
        let keys = event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => Some(Message::Escape),
            Event::Keyboard(key) if undo_shortcut(&key).is_some() => Some(Message::Key(key)),
            _ => None,
        });
        Subscription::batch([keys, self.edit_subscription()])
    }

    /// Window close, and a timer while a pending edit has unwritten keystrokes.
    fn edit_subscription(&self) -> Subscription<Message> {
        let close = window::close_requests().map(|_| Message::CloseRequested);
        let typing = self.editing.as_ref().is_some_and(|e| e.typed_at.is_some());
        let tick = if typing {
            time::every(IDLE_WRITE / 4).map(Message::EditTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([close, tick])
    }

    /// A name that turns into a text box on double-click, through the pending-edit model.
    fn editable_name<'a>(&'a self, field: Field, name: &'a str, size: u32) -> Element<'a, Message> {
        match &self.editing {
            Some(edit) if edit.field == field => text_input("Name", &edit.text)
                .id(EDIT_INPUT)
                .size(size)
                .on_input(Message::EditChanged)
                .on_submit(Message::CommitEdit)
                .into(),
            // One line, clipped (iced has no ellipsis), full name in a tooltip.
            _ => {
                let label = mouse_area(text(name).size(size).wrapping(text::Wrapping::None))
                    .on_double_click(Message::StartEdit(field));
                let full = container(text(name))
                    .padding(4)
                    .style(container::rounded_box);
                container(tooltip(label, full, tooltip::Position::Bottom))
                    .width(Fill)
                    .clip(true)
                    .into()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self
            .data
            .boards
            .iter()
            .find(|b| Some(b.id) == self.current_board)
        {
            Some(board) => self.view_board(board),
            None => self.view_empty(),
        }
    }

    fn view_board<'a>(&'a self, board: &'a Board) -> Element<'a, Message> {
        let columns = board
            .columns
            .iter()
            .map(|c| self.view_column(c))
            .chain([button("+ Add Column")
                .style(button::secondary)
                .on_press(Message::AddColumn(board.id))
                .into()]);
        column![
            self.editable_name(Field::BoardName(board.id), &board.name, 20),
            scrollable(row(columns).spacing(12).height(Fill))
                .horizontal()
                .height(Fill),
        ]
        .spacing(12)
        .padding(12)
        .into()
    }

    fn view_column<'a>(&'a self, col: &'a Column) -> Element<'a, Message> {
        let cards = col.cards.iter().map(|card| {
            container(text(&card.title))
                .width(Fill)
                .padding(8)
                .style(container::bordered_box)
                .into()
        });
        let footer: Element<_> = match &self.capture {
            Some(capture) if capture.column == col.id => text_input("Card title", &capture.text)
                .id(CAPTURE_INPUT)
                .on_input(Message::CaptureChanged)
                .on_submit(Message::SubmitCapture)
                .into(),
            _ => button("+ Add Card")
                .style(button::text)
                .on_press(Message::OpenCapture(col.id))
                .into(),
        };
        let count = text(col.count_label());
        let count = if col.over_limit() {
            count.style(text::danger)
        } else {
            count
        };
        let header = row![
            self.editable_name(Field::ColumnName(col.id), &col.name, 16),
            count,
            button("⋯")
                .style(button::text)
                .on_press(Message::ToggleColumnMenu(col.id)),
        ]
        .spacing(8)
        .align_y(Center);
        let menu: Option<Element<_>> = match &self.limit_input {
            Some(input) if input.column == col.id => Some(
                text_input("Limit (blank removes)", &input.text)
                    .id(LIMIT_INPUT)
                    .on_input(Message::LimitChanged)
                    .on_submit(Message::SubmitLimit)
                    .into(),
            ),
            _ if self.column_menu == Some(col.id) => Some(
                button("Set limit…")
                    .style(button::secondary)
                    .on_press(Message::OpenLimit(col.id))
                    .into(),
            ),
            _ => None,
        };
        container(
            column![header]
                .extend(menu)
                .push(scrollable(column(cards).spacing(6)).height(Fill))
                .push(footer)
                .spacing(8),
        )
        .width(280)
        .height(Fill)
        .padding(8)
        .style(container::rounded_box)
        .into()
    }

    fn view_empty(&self) -> Element<'_, Message> {
        let content: Element<_> = match &self.board_name {
            Some(name) => column![
                text("Name your Board"),
                text_input("Board name", name)
                    .id(BOARD_NAME_INPUT)
                    .on_input(Message::BoardNameChanged)
                    .on_submit(Message::CreateBoard)
                    .width(280),
            ]
            .spacing(8)
            .into(),
            None => button("Create your first Board")
                .on_press(Message::NewBoard)
                .into(),
        };
        center(content).into()
    }
}

/// `Some(false)` for Ctrl+Z (undo), `Some(true)` for Ctrl+Y or Ctrl+Shift+Z (redo).
// ponytail: the key-mapping function (#7) takes this over.
fn undo_shortcut(event: &keyboard::Event) -> Option<bool> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    if !modifiers.command() {
        return None;
    }
    match key.to_latin(*physical_key)? {
        'z' => Some(modifiers.shift()),
        'y' => Some(true),
        _ => None,
    }
}

impl App {
    /// Loads the data folder (created if missing) and reopens the last open Board, else the first.
    pub fn open(dir: PathBuf) -> Result<App, storage::LoadError> {
        fs::create_dir_all(&dir).map_err(storage::LoadError::Io)?;
        let (data, settings) = storage::load(&dir)?.unwrap_or_default();
        let current_board = settings
            .last_board
            .filter(|id| data.boards.iter().any(|b| b.id == *id))
            .or(data.boards.first().map(|b| b.id));
        Ok(App {
            saved: (data.clone(), settings),
            data,
            current_board,
            data_dir: Some(dir),
            ..App::default()
        })
    }

    /// Saves the data file if the Boards or settings differ from what it holds.
    /// Runs after every `update`.
    pub fn save_if_changed(&mut self) {
        let Some(dir) = &self.data_dir else { return };
        if self.editing.as_ref().is_some_and(|e| e.typed_at.is_some()) {
            return; // live text waits for the idle write (`EditTick`)
        }
        let settings = Settings {
            last_board: self.current_board,
        };
        if self.saved.0 == self.data && self.saved.1 == settings {
            return;
        }
        match storage::save(dir, &self.data, &settings) {
            Ok(()) => self.saved = (self.data.clone(), settings),
            // ponytail: stderr only; #21 adds the "Not saved" bar, the background writer and retries.
            // Keeping `saved` stale makes the next update retry.
            Err(e) => eprintln!("Not saved: {}: {e}", dir.display()),
        }
    }
}
