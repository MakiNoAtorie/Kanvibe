//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::mouse_area;
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input,
};
use iced::widget::{opaque, stack};
use iced::{Element, Event, Fill, Subscription, Task, event, keyboard};
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
/// How long after the last keystroke a pending edit's live text is written to disk.
const IDLE_WRITE: Duration = Duration::from_secs(1);
/// How often a failed save is retried while nothing changes.
const RETRY: Duration = Duration::from_secs(10);

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
    /// The state the background writer is saving; `Some` while a save is in flight.
    pub writing: Option<(Data, Settings)>,
    /// The state whose save failed. Not retried until something changes or `RetrySave`.
    pub failed: Option<(Data, Settings)>,
    /// The persistent bar: "Not saved: <path>: <OS error>". Cleared by a successful save.
    pub not_saved: Option<String>,
    pub closing: Closing,
}

/// The window close: the app waits for the save, asks when it fails, then exits.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Closing {
    #[default]
    No,
    /// Close requested; waiting for the save in flight.
    Waiting,
    /// The save failed: the dialog asks Retry / Save a copy as… / Quit without saving.
    Asking,
    /// `iced::exit()` returned.
    Exiting,
}

/// A save for the background writer. Its result comes back as `Message::Saved`.
#[derive(Debug)]
pub struct SaveJob {
    dir: PathBuf,
    data: Data,
    settings: Settings,
}

impl SaveJob {
    /// Writes the data file, blocking. The one place the app performs a save.
    pub fn run(self) -> Message {
        let saved = storage::save(&self.dir, &self.data, &self.settings);
        Message::Saved(saved.map_err(|e| not_saved(&storage::file_path(&self.dir), e)))
    }
}

fn not_saved(path: &std::path::Path, e: std::io::Error) -> String {
    format!("Not saved: {}: {e}", path.display())
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
    /// The background writer finished; `Err` holds the "Not saved" bar text.
    Saved(Result<(), String>),
    /// The 10 s timer while not saved, or Retry in the close dialog: try the failed save again.
    RetrySave,
    /// "Save a copy as…" in the close dialog: the native Save dialog, then the copy is written.
    SaveCopy,
    /// `None` when the Save dialog was cancelled; `Err` holds the "Not saved" bar text.
    CopySaved(Option<Result<(), String>>),
    QuitWithoutSaving,
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
            Message::Escape if self.closing == Closing::Asking => self.closing = Closing::No,
            Message::Escape => {
                self.board_name = None;
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
                self.failed = None; // closing is a save trigger: try again now
                self.closing = Closing::Waiting;
                return self.exit_if_saved();
            }
            Message::Saved(result) => {
                let attempted = self.writing.take();
                match result {
                    Ok(()) => {
                        self.saved = attempted.unwrap_or_default();
                        self.not_saved = None;
                        return self.exit_if_saved();
                    }
                    Err(e) => {
                        self.failed = attempted;
                        self.not_saved = Some(e);
                        if self.closing == Closing::Waiting {
                            self.closing = Closing::Asking;
                        }
                    }
                }
            }
            Message::RetrySave => {
                self.failed = None;
                if self.closing == Closing::Asking {
                    self.closing = Closing::Waiting;
                }
            }
            Message::SaveCopy => {
                let (data, settings) = (self.data.clone(), self.settings());
                let pick = rfd::AsyncFileDialog::new()
                    .set_file_name("kanvibe.json")
                    .add_filter("Kanvibe data", &["json"])
                    .save_file();
                return Task::perform(
                    async move {
                        let path = pick.await?.path().to_owned();
                        let saved = storage::save_copy(&path, &data, &settings);
                        Some(saved.map_err(|e| not_saved(&path, e)))
                    },
                    Message::CopySaved,
                );
            }
            Message::CopySaved(Some(Ok(()))) | Message::QuitWithoutSaving => {
                self.closing = Closing::Exiting;
                return iced::exit();
            }
            Message::CopySaved(Some(Err(e))) => self.not_saved = Some(e),
            Message::CopySaved(None) => {}
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
        Subscription::batch([keys, self.edit_subscription(), self.retry_subscription()])
    }

    /// Every 10 s while the last save failed.
    fn retry_subscription(&self) -> Subscription<Message> {
        match self.not_saved {
            Some(_) => time::every(RETRY).map(|_| Message::RetrySave),
            None => Subscription::none(),
        }
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
            _ => mouse_area(text(name).size(size))
                .on_double_click(Message::StartEdit(field))
                .into(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = match self
            .data
            .boards
            .iter()
            .find(|b| Some(b.id) == self.current_board)
        {
            Some(board) => self.view_board(board),
            None => self.view_empty(),
        };
        let content: Element<_> = match &self.not_saved {
            Some(error) => column![
                container(text(error))
                    .width(Fill)
                    .padding(8)
                    .style(container::danger),
                content
            ]
            .into(),
            None => content,
        };
        match self.closing {
            Closing::Asking | Closing::Waiting if self.not_saved.is_some() => {
                stack![content, self.view_close_dialog()].into()
            }
            _ => content,
        }
    }

    /// Retry / Save a copy as… / Quit without saving, over the dimmed app.
    /// The buttons wait while a save is in flight, so exit never cuts one off.
    fn view_close_dialog(&self) -> Element<'_, Message> {
        let idle = |message| self.writing.is_none().then_some(message);
        let buttons = row![
            button("Retry").on_press_maybe(idle(Message::RetrySave)),
            button("Save a copy as…").on_press_maybe(idle(Message::SaveCopy)),
            button("Quit without saving").on_press_maybe(idle(Message::QuitWithoutSaving)),
        ]
        .spacing(8);
        let panel = column![
            text("Your changes are not saved").size(20),
            text(self.not_saved.as_deref().unwrap_or_default()),
            buttons
        ]
        .spacing(12)
        .max_width(560);
        let dim = |_: &iced::Theme| container::Style {
            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
            ..Default::default()
        };
        opaque(center(container(panel).padding(20).style(container::rounded_box)).style(dim))
    }

    fn view_board<'a>(&'a self, board: &'a Board) -> Element<'a, Message> {
        let columns = board.columns.iter().map(|c| self.view_column(c));
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
        container(
            column![
                self.editable_name(Field::ColumnName(col.id), &col.name, 16),
                scrollable(column(cards).spacing(6)).height(Fill),
                footer
            ]
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

    /// A save of the current state for the background writer, if the Boards or settings differ
    /// from what the data file holds. Runs after every `update`, never inside it.
    /// `None` while a save is in flight: its `Saved` result runs this again, so changes made
    /// meanwhile merge into one save of the latest state.
    pub fn save_if_changed(&mut self) -> Option<SaveJob> {
        let dir = self.data_dir.clone()?;
        if self.writing.is_some() || self.editing.as_ref().is_some_and(|e| e.typed_at.is_some()) {
            return None; // live text waits for the idle write (`EditTick`)
        }
        let state = (self.data.clone(), self.settings());
        if state == self.saved {
            self.not_saved = None; // e.g. undone back to what the file holds
            return None;
        }
        if self.failed.as_ref() == Some(&state) {
            return None; // wait for a change or the retry timer
        }
        self.writing = Some(state.clone());
        let (data, settings) = state;
        Some(SaveJob {
            dir,
            data,
            settings,
        })
    }

    /// Exits once a requested close has nothing left to save and no save in flight;
    /// otherwise the next `save_if_changed` starts the save and its `Saved` comes back here.
    fn exit_if_saved(&mut self) -> Task<Message> {
        let clean = self.saved.0 == self.data && self.saved.1 == self.settings();
        if self.closing != Closing::Waiting || self.writing.is_some() || !clean {
            return Task::none();
        }
        self.closing = Closing::Exiting;
        iced::exit()
    }

    /// The settings saved beside the Boards.
    fn settings(&self) -> Settings {
        Settings {
            last_board: self.current_board,
        }
    }
}
