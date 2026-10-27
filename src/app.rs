//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::mouse_area;
use iced::widget::{self, selector};
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input, tooltip,
};
use iced::widget::{opaque, stack};
use iced::{Center, Element, Event, Fill, Subscription, Task, event, keyboard};
use iced::{Theme, time, window};

use crate::domain::{Board, Column, Data, Id};
use crate::history::History;
use crate::keys::{self, Context, Input};
use crate::labels::{self, LabelPicker};
use crate::storage::{self, Settings};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const BOARD_NAME_INPUT: &str = "board-name";
const CAPTURE_INPUT: &str = "capture";
pub(crate) const EDIT_INPUT: &str = "edit";
const LIMIT_INPUT: &str = "limit";
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
    /// The Column whose header ⋯ menu is open.
    pub column_menu: Option<Id>,
    /// The "Set limit…" prompt; stays open until a valid value is submitted or Esc.
    pub limit_input: Option<LimitInput>,
    /// The selected Column and, optionally, a Card in it; both outlined.
    pub selection: Option<Selection>,
    /// The state the background writer is saving; `Some` while a save is in flight.
    pub writing: Option<(Data, Settings)>,
    /// The state whose save failed. Not retried until something changes or `RetrySave`.
    pub failed: Option<(Data, Settings)>,
    /// The persistent bar: "Not saved: <path>: <OS error>". Cleared by a successful save.
    pub not_saved: Option<String>,
    pub closing: Closing,
    /// The Card the Card panel shows. It follows the selected Card; see `validate_ui`.
    pub panel: Option<Id>,
    /// Backup rotation this session; startup sets `Off` after Restore or Start empty.
    pub rotation: storage::Rotation,
    /// The Label picker over the open Card's panel; see `labels`.
    pub label_picker: Option<LabelPicker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub column: Id,
    pub card: Option<Id>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitInput {
    pub column: Id,
    pub text: String,
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
    CardTitle(Id),
    LabelName(Id),
}

impl Field {
    fn id(self) -> Id {
        match self {
            Field::BoardName(id)
            | Field::ColumnName(id)
            | Field::CardTitle(id)
            | Field::LabelName(id) => id,
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

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A click on a Card, or on a Column's empty area (`card: None`).
    Select(Selection),
    /// An arrow key: select the neighbouring Card or Column.
    MoveSelection(Direction),
    /// Ctrl+arrow: move the selected Card to the neighbouring Column, or up/down within its own.
    MoveCard(Direction),
    /// N: open the Capture box in the selected Column, else the first.
    NewCard,
    Undo,
    Redo,
    NewBoard,
    BoardNameChanged(String),
    CreateBoard,
    OpenCapture(Id),
    CaptureChanged(String),
    SubmitCapture,
    Escape,
    /// A key press no widget captured; `update` asks which field has focus, then maps it.
    Key(keyboard::Event),
    /// A key press plus the focused widget, if any; mapped by [`keys::map`].
    KeyWithFocus(keyboard::Event, Option<selector::Target>),
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
    /// The background writer finished; `Err` holds the "Not saved" bar text.
    Saved(Result<(), String>),
    /// The 10 s timer while not saved, or Retry in the close dialog: try the failed save again.
    RetrySave,
    /// "Save a copy as…" in the close dialog: the native Save dialog, then the copy is written.
    SaveCopy,
    /// `None` when the Save dialog was cancelled; `Err` holds the "Not saved" bar text.
    CopySaved(Option<Result<(), String>>),
    QuitWithoutSaving,
    /// Enter: open the Card panel on the selected Card.
    OpenPanel,
    /// The panel's X button.
    ClosePanel,
    /// F2: edit the open Card's title.
    EditTitle,
    /// L, or a click on the panel's Labels row: open the Label picker.
    OpenLabels,
    CloseLabels,
    /// A picker checkbox: attach or detach the Label on the picker's Card.
    ToggleLabel(Id),
    /// A picker pencil: open (or close) the Label's editor and start renaming it.
    EditLabel(Id),
    /// A palette key for the Label.
    SetLabelColor(Id, String),
    /// "Delete Label": asks first when the Label is on Cards.
    DeleteLabel(Id),
    ConfirmDeleteLabel,
    CancelDeleteLabel,
    /// "+ New Label".
    NewLabel,
    NewLabelChanged(String),
    NewLabelColor(String),
    CreateLabel,
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.apply(message);
        self.validate_ui();
        task
    }

    fn apply(&mut self, message: Message) -> Task<Message> {
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
                    self.validate_selection();
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
                    return self.reveal_selection();
                }
            }
            Message::Escape if self.closing == Closing::Asking => self.closing = Closing::No,
            Message::Escape if self.label_picker.is_some() => self.escape_labels(),
            Message::Escape => {
                // One layer per press: the text field, the Card panel, then the selection.
                if self.board_name.is_none()
                    && self.capture.is_none()
                    && self.editing.is_none()
                    && self.column_menu.is_none()
                    && self.limit_input.is_none()
                    && self.panel.take().is_none()
                {
                    self.selection = None;
                }
                self.board_name = None;
                self.column_menu = None;
                self.limit_input = None;
                self.capture = None; // Esc discards the capture box; other fields commit.
                self.commit_pending();
            }
            Message::NewCard => {
                let column = self
                    .selection
                    .map(|s| s.column)
                    .or_else(|| Some(self.board()?.columns.first()?.id));
                if let Some(column) = column {
                    let reveal = reveal(BOARD_SCROLL.into(), column_widget(column), true);
                    return Task::batch([self.update(Message::OpenCapture(column)), reveal]);
                }
            }
            Message::Select(selection) => {
                self.commit_pending();
                self.selection = Some(selection);
                if selection.card.is_some() {
                    self.panel = selection.card; // clicking a Card opens or switches the panel
                }
                return self.reveal_selection();
            }
            Message::OpenPanel => {
                self.commit_pending();
                self.panel = self.selection.and_then(|s| s.card).or(self.panel);
            }
            Message::ClosePanel => {
                self.commit_pending();
                self.panel = None;
            }
            Message::EditTitle => {
                if let Some(card) = self.panel {
                    return self.update(Message::StartEdit(Field::CardTitle(card)));
                }
            }
            Message::OpenLabels
            | Message::CloseLabels
            | Message::ToggleLabel(_)
            | Message::EditLabel(_)
            | Message::SetLabelColor(..)
            | Message::DeleteLabel(_)
            | Message::ConfirmDeleteLabel
            | Message::CancelDeleteLabel
            | Message::NewLabel
            | Message::NewLabelChanged(_)
            | Message::NewLabelColor(_)
            | Message::CreateLabel => return self.update_labels(message),
            Message::MoveSelection(direction) => {
                self.commit_pending();
                if let Some(board) = self.board() {
                    self.selection = moved(board, self.selection, direction).or(self.selection);
                }
                return self.reveal_selection();
            }
            Message::MoveCard(direction) => {
                self.commit_pending();
                let target = self.board().zip(self.selection);
                if let Some((card, column, index)) =
                    target.and_then(|(board, selection)| card_move(board, selection, direction))
                {
                    self.data.move_card(card, column, index);
                    self.selection = Some(Selection {
                        column,
                        card: Some(card),
                    });
                    let board = self.current_board;
                    self.history.record(&self.data.boards, board, board);
                    return self.reveal_selection();
                }
            }
            Message::Undo | Message::Redo => {
                self.commit_pending();
                let step = match message {
                    Message::Undo => self.history.undo(),
                    _ => self.history.redo(),
                };
                if let Some((boards, show)) = step {
                    self.data.boards = boards;
                    if show.is_some() {
                        self.current_board = show;
                    }
                    self.validate_selection();
                }
            }
            Message::Key(event) => {
                return selector::find(selector::is_focused())
                    .map(move |focused| Message::KeyWithFocus(event.clone(), focused));
            }
            Message::KeyWithFocus(event, focused) => {
                let input = |target: selector::Target| {
                    let selector::Target::Focusable { id: Some(id), .. } = target else {
                        return None;
                    };
                    [
                        (BOARD_NAME_INPUT, Input::BoardName),
                        (CAPTURE_INPUT, Input::Capture),
                        (EDIT_INPUT, Input::Edit),
                        (LIMIT_INPUT, Input::Edit), // a text box: keys go to it
                        (labels::NEW_INPUT, Input::Edit),
                    ]
                    .into_iter()
                    .find_map(|(name, input)| (id == widget::Id::new(name)).then_some(input))
                };
                let focused = focused.and_then(input);
                if let Some(message) = keys::map(&event, &Context { focused }) {
                    return self.update(message);
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
                    let clash = self.data.is_label_clash(edit.field.id(), &text);
                    let live = if text.trim().is_empty() || clash {
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
            if let Some(card) = self.data.add_card(capture.column, &capture.text) {
                self.selection = Some(Selection {
                    column: capture.column,
                    card: Some(card),
                }); // new Cards become the selected Card
            }
            let board = self.board_of_column(capture.column);
            self.history.record(&self.data.boards, board, board);
        }
        if let Some(edit) = self.editing.take() {
            let board = match edit.field {
                Field::BoardName(_) => None,
                Field::ColumnName(column) => self.board_of_column(column),
                Field::LabelName(label) => self.data.board_of_label(label),
                Field::CardTitle(card) => {
                    if self.data.name(card) != Some(edit.start.as_str()) {
                        self.data.touch(card); // a changed title is the Card's own change
                    }
                    self.board_of_card(card)
                }
            };
            self.history.record(&self.data.boards, board, board);
        }
    }

    /// Scrolls the selected Column fully into view, and its Card into view within it.
    fn reveal_selection(&self) -> Task<Message> {
        let Some(s) = self.selection else {
            return Task::none();
        };
        let column = reveal(BOARD_SCROLL.into(), column_widget(s.column), true);
        let card = s.card.map_or(Task::none(), |card| {
            reveal(cards_widget(s.column), card_widget(card), false)
        });
        Task::batch([column, card])
    }

    /// Keeps the selection on the current Board: it follows its Card to another Column and is
    /// cleared when its Card or Column is gone.
    // ponytail: called where Boards can change under the selection (undo/redo, Board switch);
    // the spec's "after every update" check can replace these calls.
    fn validate_selection(&mut self) {
        let Some(board) = self.board() else {
            self.selection = None;
            return;
        };
        self.selection = self.selection.and_then(|s| match s.card {
            Some(card) => board
                .columns
                .iter()
                .find(|c| c.cards.iter().any(|x| x.id == card))
                .map(|c| Selection {
                    column: c.id,
                    card: Some(card),
                }),
            None => board.columns.iter().any(|c| c.id == s.column).then_some(s),
        });
    }

    /// Runs at the end of every `update`: Selection and panel are cleared when their Card is no
    /// longer an active Card on the current Board, and an open panel follows the selected Card.
    fn validate_ui(&mut self) {
        self.validate_selection();
        self.validate_labels();
        let on_board = |card: Id| {
            self.board().is_some_and(|b| {
                b.columns
                    .iter()
                    .any(|c| c.cards.iter().any(|x| x.id == card))
            })
        };
        self.panel = self.panel.filter(|card| on_board(*card));
        if self.panel.is_some() {
            self.panel = self.selection.and_then(|s| s.card).or(self.panel);
        }
    }

    pub(crate) fn board_of_card(&self, card: Id) -> Option<Id> {
        let board = self.data.boards.iter().find(|b| {
            b.columns
                .iter()
                .any(|c| c.cards.iter().any(|x| x.id == card))
        })?;
        Some(board.id)
    }

    /// The current Board.
    pub(crate) fn board(&self) -> Option<&Board> {
        let id = self.current_board?;
        self.data.boards.iter().find(|b| b.id == id)
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
        let keys = event::listen_with(|event, status, _window| match (event, status) {
            (Event::Keyboard(key @ keyboard::Event::KeyPressed { .. }), event::Status::Ignored) => {
                Some(Message::Key(key))
            }
            // A focused text_input captures Esc and only unfocuses; sending Escape for it is the
            // "leave the text field" layer, so a field closes (or commits) on one press.
            (
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }),
                event::Status::Captured,
            ) => Some(Message::Escape),
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
    pub(crate) fn editable_name<'a>(
        &'a self,
        field: Field,
        name: &'a str,
        size: u32,
    ) -> Element<'a, Message> {
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
        let content = match self
            .data
            .boards
            .iter()
            .find(|b| Some(b.id) == self.current_board)
        {
            Some(board) => self.view_board(board),
            None => self.view_empty(),
        };
        // The panel pushes the Board narrower; it never overlays it.
        let content: Element<_> = match self.panel.and_then(|id| self.data.card(id)) {
            Some(card) => row![container(content).width(Fill), self.view_panel(card)].into(),
            None => content,
        };
        let content: Element<_> = match &self.label_picker {
            Some(picker) => stack![content, self.view_label_picker(picker)].into(),
            None => content,
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
                .id(BOARD_SCROLL)
                .horizontal()
                .height(Fill),
        ]
        .spacing(12)
        .padding(12)
        .into()
    }

    fn view_column<'a>(&'a self, col: &'a Column) -> Element<'a, Message> {
        let selection = self.selection.filter(|s| s.column == col.id);
        let cards = col.cards.iter().map(|card| {
            let select = Message::Select(Selection {
                column: col.id,
                card: Some(card.id),
            });
            let title = text(&card.title);
            let chips = self.board().and_then(|b| labels::chips(b, card));
            let card_box = container(column![title].spacing(4).extend(chips))
                .id(card_widget(card.id))
                .width(Fill)
                .padding(8)
                .style(outlined(
                    container::bordered_box,
                    selection.is_some_and(|s| s.card == Some(card.id)),
                ));
            mouse_area(card_box).on_press(select).into()
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
        let column_box = container(
            column![header]
                .extend(menu)
                .push(
                    scrollable(column(cards).spacing(6))
                        .id(cards_widget(col.id))
                        .height(Fill),
                )
                .push(footer)
                .spacing(8),
        )
        .id(column_widget(col.id))
        .width(280)
        .height(Fill)
        .padding(8)
        .style(outlined(container::rounded_box, selection.is_some()));
        // Cards and buttons capture their clicks, so this only fires on the Column's empty area.
        mouse_area(column_box)
            .on_press(Message::Select(Selection {
                column: col.id,
                card: None,
            }))
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

/// The scroll offset that brings `start..start + len` into a `viewport`-long view currently
/// scrolled to `offset`, moving as little as possible; `None` when it is already fully visible.
/// Something longer than the viewport shows its start.
pub fn scroll_to_show(start: f32, len: f32, offset: f32, viewport: f32) -> Option<f32> {
    if start < offset {
        Some(start)
    } else if start + len > offset + viewport {
        Some((start + len - viewport).min(start))
    } else {
        None
    }
}

/// Scrolls `scrollable` so its descendant `child` is fully visible along one axis.
fn reveal(scrollable: widget::Id, child: widget::Id, horizontal: bool) -> Task<Message> {
    selector::find(scrollable.clone()).and_then(move |area| {
        let scrollable = scrollable.clone();
        selector::find(child.clone()).and_then(move |child| {
            let selector::Target::Scrollable {
                bounds,
                translation,
                ..
            } = &area
            else {
                return Task::none();
            };
            // Layout bounds are unscrolled, so the child's offset from the scrollable is its
            // position in the content.
            let c = child.bounds();
            let to = if horizontal {
                scroll_to_show(c.x - bounds.x, c.width, translation.x, bounds.width).map(|x| {
                    operation::AbsoluteOffset {
                        x: Some(x),
                        y: None,
                    }
                })
            } else {
                scroll_to_show(c.y - bounds.y, c.height, translation.y, bounds.height).map(|y| {
                    operation::AbsoluteOffset {
                        x: None,
                        y: Some(y),
                    }
                })
            };
            to.map_or(Task::none(), |to| {
                operation::scroll_to(scrollable.clone(), to)
            })
        })
    })
}

/// `base`, with a 2px primary-colour border when `selected`.
fn outlined(
    base: fn(&Theme) -> container::Style,
    selected: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let style = base(theme);
        if !selected {
            return style;
        }
        container::Style {
            border: style.border.color(theme.palette().primary).width(2),
            ..style
        }
    }
}

const BOARD_SCROLL: &str = "board";

fn column_widget(column: Id) -> widget::Id {
    widget::Id::from(format!("column-{column}"))
}

fn cards_widget(column: Id) -> widget::Id {
    widget::Id::from(format!("cards-{column}"))
}

fn card_widget(card: Id) -> widget::Id {
    widget::Id::from(format!("card-{card}"))
}

/// Where an arrow key takes the selection on `board`; `None` = stay put.
fn moved(board: &Board, selection: Option<Selection>, direction: Direction) -> Option<Selection> {
    let at = |column: &Column, index: Option<usize>| Selection {
        column: column.id,
        card: index.and_then(|i| column.cards.get(i)).map(|card| card.id),
    };
    let Some(selection) = selection else {
        return board.columns.first().map(|c| at(c, Some(0)));
    };
    let (ci, index) = position(board, selection)?;
    let column = &board.columns[ci];
    match direction {
        Direction::Up => Some(at(column, Some(index?.checked_sub(1)?))),
        Direction::Down => {
            let next = index.map_or(0, |i| i + 1);
            (next < column.cards.len()).then(|| at(column, Some(next)))
        }
        Direction::Left | Direction::Right => {
            let next = match direction {
                Direction::Left => board.columns.get(ci.checked_sub(1)?)?,
                _ => board.columns.get(ci + 1)?,
            };
            let i = index.unwrap_or(0).min(next.cards.len().saturating_sub(1));
            Some(at(next, Some(i)))
        }
    }
}

/// Where Ctrl+arrow moves the selected Card: (Card, Column, index). `None` = no Card or at an end.
fn card_move(board: &Board, selection: Selection, direction: Direction) -> Option<(Id, Id, usize)> {
    let (ci, index) = position(board, selection)?;
    let (card, i) = (selection.card?, index?);
    let column = &board.columns[ci];
    match direction {
        Direction::Up => Some((card, column.id, i.checked_sub(1)?)),
        Direction::Down => (i + 1 < column.cards.len()).then_some((card, column.id, i + 1)),
        Direction::Left => Some((card, board.columns.get(ci.checked_sub(1)?)?.id, usize::MAX)),
        Direction::Right => Some((card, board.columns.get(ci + 1)?.id, usize::MAX)),
    }
}

/// The selection's Column index on `board` and its Card's index in that Column.
fn position(board: &Board, selection: Selection) -> Option<(usize, Option<usize>)> {
    let ci = board
        .columns
        .iter()
        .position(|c| c.id == selection.column)?;
    let cards = &board.columns[ci].cards;
    let index = selection
        .card
        .and_then(|id| cards.iter().position(|c| c.id == id));
    Some((ci, index))
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
        // ponytail: rotation runs here on the UI thread (a file copy, at most once per session or
        // per 24 h, never while a write is in flight); move it into `SaveJob::run` if it ever stalls.
        let changed = self.saved.0 != state.0;
        if let Err(e) = storage::rotate(&dir, &mut self.rotation, changed) {
            eprintln!("Backups not rotated: {e}");
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
