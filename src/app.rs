//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::mouse_area;
use iced::widget::{self, selector};
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input, tooltip,
};
use iced::widget::{opaque, stack};
use iced::{Border, Center, Element, Event, Fill, Padding, Subscription, Task};
use iced::{Theme, time, window};
use iced::{event, keyboard};

use crate::description;
use crate::domain::{Board, Card, Column, Data, Id};
use crate::drag::{self, Drag, Item, Target};
use crate::filter::{self, Filter};
use crate::history::History;
use crate::keys::{self, Context, Input};
use crate::labels::{self, LabelPicker};
use crate::storage::{self, Settings};
use iced::Point;
use iced::widget::text_editor;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const BOARD_NAME_INPUT: &str = "board-name";
const CAPTURE_INPUT: &str = "capture";
const CAPTURE_DESCRIPTION: &str = "capture-description";
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
    /// The state the background writer is saving; `Some` while a save is in flight. Shared
    /// with the `SaveJob`, so a save copies the Boards once.
    pub writing: Option<Arc<(Data, Settings)>>,
    /// The state whose save failed. Not retried until something changes or `RetrySave`.
    pub failed: Option<(Data, Settings)>,
    /// The persistent bar: "Not saved: <path>: <OS error>". Cleared by a successful save.
    pub not_saved: Option<String>,
    pub closing: Closing,
    /// The open confirmation overlay, if any.
    pub confirm: Option<Confirm>,
    /// The current Board's header ⋯ menu is open.
    pub board_menu: bool,
    /// The sidebar is a 40px rail. Saved in the settings.
    pub sidebar_collapsed: bool,
    /// The sidebar's "Archived (n)" section is expanded.
    pub archived_open: bool,
    /// The press or drag in progress; see `drag`.
    pub drag: Option<Drag>,
    /// The selected Card is the open Card, expanded in place in its Column. Cleared when the
    /// Selection has no Card; see `validate_ui`.
    pub open: bool,
    /// The open Card's highlighted part: Up/Down move it, F2 edits it.
    pub part: Part,
    /// Backup rotation this session; startup sets `Off` after Restore or Start empty.
    pub rotation: storage::Rotation,
    /// The Board whose Archive view replaces its Columns; `None` = the Columns show.
    pub archive: Option<Id>,
    /// The Label picker over the open Card; see `labels`.
    pub label_picker: Option<LabelPicker>,
    /// The "Move to Board…" picker for this Card; see `move_board`.
    pub move_picker: Option<Id>,
    /// The theme. Saved in the settings.
    pub skin: crate::skin::Skin,
    /// Window geometry and the maximised flag. Saved with the next save, never a save on its own.
    pub window: crate::chrome::WindowState,
    /// The F1 shortcuts overlay is open.
    pub shortcuts: bool,
    /// The description editor's text and cursor while `Field::Description` is pending.
    pub editor: text_editor::Content,
    /// The open Card's description as last parsed for the rendered view: (source, parsed).
    pub rendered: (String, iced::widget::markdown::Content),
    /// A message over the dimmed app with only OK (an import refused, an export failed).
    pub notice: Option<String>,
    /// "Save as image…" is running (Save dialog or rendering); the menu item reads "Saving image…".
    pub saving_image: bool,
    /// The Filter on the current Board; never saved. See `filter`.
    pub filter: Filter,
    /// Cards created while the Filter is on: shown even when they don't match.
    pub filter_kept: Vec<Id>,
    /// The Labels filter list under the Board header is open.
    pub filter_menu: bool,
    /// The Search view replacing the Columns; `None` = closed. See `search`.
    pub search: Option<crate::search::Search>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub column: Id,
    pub card: Option<Id>,
}

/// A stop for Up/Down inside the open Card, in the order the Card shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Part {
    #[default]
    Title,
    Labels,
    Description,
    Item(Id),
    /// "+ Add Checklist Item".
    Checklist,
    Files,
    Due,
}

/// The open Card's parts, top to bottom.
pub fn parts(card: &Card) -> Vec<Part> {
    let items = card.checklist.iter().map(|i| Part::Item(i.id));
    [Part::Title, Part::Labels, Part::Description]
        .into_iter()
        .chain(items)
        .chain([Part::Checklist, Part::Files, Part::Due])
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
    /// Home: the first Card of the Column.
    Top,
    /// End: the last Card of the Column.
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitInput {
    pub column: Id,
    pub text: String,
    /// The last submit was refused; the prompt says why until the text changes.
    pub refused: bool,
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
    state: Arc<(Data, Settings)>,
    /// `App.rotation` when the job was made; `run` rotates and hands the new value back.
    rotation: storage::Rotation,
    /// The save changes Board data (see `storage::rotate`).
    data_changed: bool,
}

impl SaveJob {
    /// Rotates the backups if due, then writes the data file, blocking. The one place the app
    /// performs a save; off the UI thread, since rotating reads, copies and parses the file.
    pub fn run(mut self) -> Message {
        if let Err(e) = storage::rotate(&self.dir, &mut self.rotation, self.data_changed) {
            eprintln!("Backups not rotated: {e}");
        }
        let saved = storage::save(&self.dir, &self.state.0, &self.state.1);
        let saved = saved.map_err(|e| not_saved(&storage::file_path(&self.dir), e));
        Message::Saved(saved, self.rotation)
    }
}

impl Capture {
    fn new(column: Id) -> Capture {
        Capture {
            column,
            text: String::new(),
            description: String::new(),
        }
    }
}

fn not_saved(path: &std::path::Path, e: std::io::Error) -> String {
    format!("Not saved: {}: {e}", path.display())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub column: Id,
    pub text: String,
    /// The new Card's description; blank adds none.
    pub description: String,
}

/// A text field whose edits go straight into the Boards and become one undo step on commit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Field {
    BoardName(Id),
    ColumnName(Id),
    CardTitle(Id),
    ChecklistItem(Id),
    LabelName(Id),
    /// The Card's Markdown description, in the open Card's description editor.
    Description(Id),
    /// The open Card's due date, as typed; written to the Card only on commit.
    Due(Id),
}

impl Field {
    fn id(self) -> Id {
        match self {
            Field::BoardName(id)
            | Field::ColumnName(id)
            | Field::CardTitle(id)
            | Field::ChecklistItem(id)
            | Field::LabelName(id)
            | Field::Due(id)
            | Field::Description(id) => id,
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
    /// Ctrl+arrow: move the selected Card to the neighbouring Column, or up/down within its own;
    /// with a Column selected and no Card, Ctrl+Left/Right moves the Column one place.
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
    CaptureDescriptionChanged(String),
    /// Tab (`true`) / Shift+Tab (`false`) in the Capture box: to its description / title.
    FocusCapture(bool),
    SubmitCapture,
    /// A paste into the Capture box (its text after the paste); the clipboard is re-read.
    CapturePasted(String),
    /// The clipboard after a paste: two or more non-blank lines become one Card each.
    CaptureLines(Option<String>),
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
    /// Typing in the open Card's Due field: starts `Field::Due` if needed, then `EditChanged`.
    DueChanged(Id, String),
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
    /// The background writer finished; `Err` holds the "Not saved" bar text. Also the backup
    /// rotation state after the job's rotation.
    Saved(Result<(), String>, storage::Rotation),
    /// The 10 s timer while not saved, or Retry in the close dialog: try the failed save again.
    RetrySave,
    /// "Save a copy as…" in the close dialog: the native Save dialog, then the copy is written.
    SaveCopy,
    /// `None` when the Save dialog was cancelled; `Err` holds the "Not saved" bar text.
    CopySaved(Option<Result<(), String>>),
    QuitWithoutSaving,
    /// A Board row in the sidebar: switch to it. Archived Boards cannot be opened.
    OpenBoard(Id),
    /// Ctrl+Tab (`true`) / Ctrl+Shift+Tab (`false`): the next / previous active Board in
    /// sidebar order, wrapping around.
    CycleBoard(bool),
    /// "Archive Board" in the Board's ⋯ menu.
    ArchiveBoard(Id),
    /// Restore on an archived Board's sidebar row.
    RestoreBoard(Id),
    /// Delete on an archived Board's sidebar row: asks first.
    AskDeleteBoard(Id),
    /// Permanently deletes an archived Board.
    DeleteBoard(Id),
    /// The confirm button of the open [`Confirm`]: sends its action.
    Confirm,
    /// The ⋯ button in the Board header.
    ToggleBoardMenu,
    /// Collapse the sidebar to a rail, or expand it.
    ToggleSidebar,
    /// The "Archived (n)" heading in the sidebar.
    ToggleArchivedBoards,
    /// Left press on a draggable item: a drag after 5px of movement, else a click on release.
    DragPress(Item),
    /// The cursor moved (window coordinates) while a press or drag is in progress.
    DragMoved(Point),
    /// The cursor is over a drop target.
    DragHover(Target),
    /// The cursor left a drop target.
    DragLeave(Target),
    /// The left button was released anywhere: drop, click, or cancel when over no target.
    DragEnd,
    /// Esc during a drag: the item stays where it was.
    DragCancel,
    /// Autoscroll timer while dragging.
    DragTick,
    /// Enter: open the selected Card (expand it in place).
    OpenCard,
    /// Double-click on a Card: open it and edit its title.
    EditCard(Id),
    /// The open Card's ✕ button.
    CloseCard,
    /// Enter or F2: edit the open or selected Card's title.
    EditTitle,
    /// Up/Down/Home/End in the open Card: highlight another part.
    MovePart(Direction),
    /// F2 in the open Card: edit its highlighted part.
    EditPart,
    /// Del: archive the selected Card.
    ArchiveSelected,
    /// The open Card's "Archive" button.
    ArchiveCard(Id),
    /// "Remove Column (archives N Cards)" in the Column menu.
    RemoveColumn(Id),
    /// "Archive (n)" in the Board header.
    OpenArchive,
    /// "Back to Board".
    CloseArchive,
    Restore(Id),
    /// An archived Card's Delete button: opens the question.
    AskDelete(Id),
    /// Permanently deletes an archived Card (the action of its [`Confirm`]).
    DeleteCard(Id),
    /// "+ Add Checklist Item" in the Card's Checklist: a blank Item at the end, in edit.
    AddItem(Id),
    /// Enter in an Item: commit it and, unless it was blank, add one below.
    SubmitItem(Id),
    ToggleItem(Id),
    /// The ▲ / ▼ buttons (`Up` or `Down`).
    MoveItem(Id, Direction),
    /// Ctrl+Up / Ctrl+Down in the Item being edited.
    MoveEditedItem(Direction),
    RemoveItem(Id),
    /// L, or a click on the open Card's Labels: open the Label picker.
    OpenLabels,
    CloseLabels,
    /// M, or "Move to Board…" in the open Card: open the Board picker.
    OpenMove,
    CloseMove,
    /// A Board in the picker, or a drop on a sidebar Board: move the Card there.
    MoveToBoard(Id, Id),
    /// A picker checkbox: attach or detach the Label on the picker's Card.
    ToggleLabel(Id),
    /// A picker pencil: open (or close) the Label's editor and start renaming it.
    EditLabel(Id),
    /// A palette key for the Label.
    SetLabelColor(Id, String),
    /// "Delete Label": asks first when the Label is on Cards.
    DeleteLabel(Id),
    /// The action of the delete prompt: delete the Label and strip it from every Card.
    RemoveLabel(Id),
    /// "+ New Label".
    NewLabel,
    NewLabelChanged(String),
    NewLabelColor(String),
    CreateLabel,
    /// A pick from the sidebar footer's theme list.
    SetTheme(crate::skin::Skin),
    /// The window was resized (logical pixels).
    Resized(iced::Size),
    /// The window was moved (logical pixels).
    Moved(iced::Point),
    /// Whether the window is maximised: the answer to the query each report starts.
    Maximized(bool),
    /// F1 or the footer's "Keyboard shortcuts".
    ShowShortcuts,
    /// The footer's "Open data folder".
    OpenDataFolder,
    /// E or the open Card's Edit button: open the description editor.
    EditDescription,
    /// Typing, cursor moves and clicks in the description editor.
    DescriptionAction(text_editor::Action),
    /// Ctrl+B / Ctrl+I / Ctrl+K in the description editor.
    Format(description::Markup),
    /// A link clicked in the rendered description; only http, https and mailto open.
    OpenLink(String),
    /// A click no widget took: leave the description editor, else close the open Card.
    ClickOutside,
    /// "Export Board…" in the Board's ⋯ menu: the native Save dialog, then the file is written.
    ExportBoard(Id),
    /// `None` when the Save dialog was cancelled; `Err` holds the message to show.
    Exported(Option<Result<(), String>>),
    /// "Save as image…" in the Board's ⋯ menu: the size check, the Save dialog, then the PNG.
    SaveImage(Id),
    /// `None` when the Save dialog was cancelled; `Err` holds the message to show.
    ImageSaved(Option<Result<(), String>>),
    /// "Import Board…" in the sidebar footer: the native Open dialog, then the file is read.
    ImportBoard,
    /// The picked file's bytes; `None` when the dialog was cancelled, `Err` when it can't be read.
    Imported(Option<Result<Vec<u8>, String>>),
    /// Ctrl+F: focus the filter box.
    FocusFilter,
    /// Typing in the filter box.
    FilterChanged(String),
    /// Enter in the filter box: select the first visible Card.
    SubmitFilter,
    /// The filter box's × button, or "Clear filter".
    ClearFilter,
    /// The Labels filter button: open or close its list.
    ToggleFilterMenu,
    /// A checkbox in the Labels filter list.
    FilterLabel(Id),
    /// Esc captured by a focused text box (it has already unfocused): the "leave the text
    /// field" layer. Only the filter box keeps no state, so then nothing else happens.
    LeaveField,
    /// Ctrl+Shift+F or "Search" in the sidebar: open the Search view.
    OpenSearch,
    SearchChanged(String),
    /// Up/Down in the search box: move the highlighted hit.
    SearchMove(Direction),
    /// A click on a hit, or Enter on the highlighted one: open its Board on the Card.
    SearchPick(Id),
    /// "Back to Board".
    CloseSearch,
    /// Ctrl+D, or "Duplicate" in the open Card: copy the selected Card right below it.
    DuplicateCard,
    /// "+ Add file…" in the open Card: the native Open dialog for this Card.
    AddFile(Id),
    /// The path picked for the Card; `None` when the dialog was cancelled.
    FileChosen(Id, Option<std::path::PathBuf>),
    /// The ✕ on the Card's file at this index.
    RemoveFile(Id, usize),
    /// A click on a file: open it with its default program.
    OpenFile(String),
    /// "Collapse" in the Column menu, or a click on a collapsed Column's strip.
    ToggleCollapse(Id),
}

/// A question over the dimmed app (`App.confirm`). Confirm sends `action`; Esc or Cancel drops it.
#[derive(Debug, Clone, PartialEq)]
pub struct Confirm {
    pub prompt: String,
    pub action: Message,
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::SearchPick(card) = message {
            return self.pick(card); // nested updates; see `pick`
        }
        let (board, order): (_, Vec<Id>) = (
            self.current_board,
            self.data.boards.iter().map(|b| b.id).collect(),
        );
        let task = self.apply(message);
        self.validate_ui(board, &order);
        self.refresh_rendered();
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
                self.commit_pending(); // a Capture box opened under the prompt keeps its Card
                let name = self.board_name.as_deref().unwrap_or_default();
                if let Some(id) = self.data.create_board(name) {
                    self.current_board = Some(id);
                    self.board_name = None;
                    self.record_step(None);
                }
            }
            Message::OpenCapture(column) => {
                self.commit_pending();
                self.expand(column); // a strip has no room for the box
                self.open = false; // new Cards become selected; they shouldn't open one by one
                self.capture = Some(Capture::new(column));
                return focus_capture(column);
            }
            Message::CaptureChanged(text) => {
                if let Some(capture) = &mut self.capture {
                    capture.text = text;
                }
            }
            Message::CaptureDescriptionChanged(text) => {
                if let Some(capture) = &mut self.capture {
                    capture.description = text;
                }
            }
            Message::FocusCapture(description) => {
                return operation::focus(match description {
                    true => CAPTURE_DESCRIPTION,
                    false => CAPTURE_INPUT,
                });
            }
            Message::SubmitCapture => {
                if let Some(column) = self.capture.as_ref().map(|c| c.column) {
                    self.commit_pending();
                    self.capture = Some(Capture::new(column));
                    // The new Card shifts the box in the tree, which drops its focus.
                    return focus_capture(column);
                }
            }
            Message::CapturePasted(text) => {
                if let Some(capture) = &mut self.capture {
                    capture.text = text;
                    return iced::clipboard::read().map(Message::CaptureLines);
                }
            }
            Message::CaptureLines(text) => return self.capture_lines(text.unwrap_or_default()),
            Message::Escape if self.drag.is_some() => self.drag = None, // first Esc layer
            Message::Escape if self.shortcuts => self.shortcuts = false,
            Message::Escape if self.notice.is_some() => self.notice = None,
            Message::Escape if self.closing == Closing::Asking => self.closing = Closing::No,
            Message::Escape if self.confirm.is_some() => self.confirm = None,
            Message::Escape if self.label_picker.is_some() => self.escape_labels(),
            Message::Escape if self.move_picker.is_some() => self.move_picker = None,
            Message::Escape if self.search.is_some() && !self.field_open() => self.search = None,
            Message::Escape => {
                // One layer per press: the text field, the Archive view, the open Card, the
                // Filter, then the selection.
                if !self.field_open()
                    && self.column_menu.is_none()
                    && !self.board_menu
                    && !self.filter_menu
                    && self.archive.take().is_none()
                    && !std::mem::take(&mut self.open)
                    && !self.clear_filter()
                {
                    self.selection = None;
                }
                self.board_name = None;
                self.column_menu = None;
                self.limit_input = None;
                self.board_menu = false;
                self.filter_menu = false;
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
                // A click selects; it opens nothing. A click inside the open Card keeps it open.
                self.open &= self.open_card().is_some_and(|c| selection.card == Some(c));
                self.selection = Some(selection);
                return self.reveal_selection();
            }
            Message::OpenCard => {
                self.commit_pending();
                self.part = Part::Title;
                self.open_selected();
                return self.reveal_selection(); // the whole open Card, footer included
            }
            Message::CloseCard => {
                self.commit_pending();
                self.open = false;
            }
            Message::EditDescription => {
                if let Some(card) = self.open_selected() {
                    return self.update(Message::StartEdit(Field::Description(card)));
                }
            }
            Message::DescriptionAction(action) => self.description_action(action),
            Message::Format(markup) => self.format(markup),
            Message::OpenLink(url) => description::open_link(&url),
            // One layer per click, as Esc: the description editor, then the open Card. A
            // rename keeps its own commit points.
            Message::ClickOutside => match self.editing.as_ref().map(|e| e.field) {
                Some(Field::Description(_)) => _ = self.commit_pending(),
                Some(_) => {}
                None => self.open = false,
            },
            Message::EditCard(card) => {
                self.drag = None; // the double-click's own release is no click
                return self.update(Message::StartEdit(Field::CardTitle(card)));
            }
            Message::EditTitle => {
                if let Some(card) = self.open_selected() {
                    return self.update(Message::StartEdit(Field::CardTitle(card)));
                }
            }
            Message::MovePart(direction) => {
                if let Some(card) = self.open_card().and_then(|c| self.data.card(c)) {
                    let parts = parts(card);
                    let at = parts.iter().position(|p| *p == self.part).unwrap_or(0);
                    let last = parts.len() - 1;
                    self.part = parts[match direction {
                        Direction::Up => at.saturating_sub(1),
                        Direction::Down => (at + 1).min(last),
                        Direction::Top => 0,
                        Direction::Bottom => last,
                        Direction::Left | Direction::Right => at,
                    }];
                }
            }
            Message::EditPart => {
                if let Some(card) = self.open_card() {
                    return self.update(match self.part {
                        Part::Title => Message::EditTitle,
                        Part::Labels => Message::OpenLabels,
                        Part::Description => Message::EditDescription,
                        Part::Item(item) => Message::StartEdit(Field::ChecklistItem(item)),
                        Part::Checklist => Message::AddItem(card),
                        Part::Files => Message::AddFile(card),
                        Part::Due => Message::StartEdit(Field::Due(card)),
                    });
                }
            }
            Message::ArchiveSelected => {
                if let Some(card) = self.selection.and_then(|s| s.card) {
                    return self.apply(Message::ArchiveCard(card));
                }
            }
            Message::DuplicateCard => {
                self.commit_pending();
                let selection = self.selection.and_then(|s| Some((s.column, s.card?)));
                if let Some((column, card)) = selection
                    && let Some(copy) = self.data.duplicate_card(card)
                {
                    self.selection = Some(Selection {
                        column,
                        card: Some(copy),
                    });
                    self.record_step(self.current_board);
                    return self.reveal_selection();
                }
            }
            Message::ArchiveCard(card) => {
                self.archive_card(card);
                return self.reveal_selection();
            }
            Message::RemoveColumn(column) => self.remove_column(column),
            Message::OpenArchive => self.open_archive(),
            Message::CloseArchive => self.archive = None,
            Message::Restore(card) => self.restore_card(card),
            Message::AskDelete(card) => self.ask_delete(card),
            Message::DeleteCard(card) => self.delete_card(card),
            Message::AddItem(card) => {
                self.commit_pending();
                return self.add_item(card, None);
            }
            Message::SubmitItem(item) => {
                let card = self.data.card_of_item(item);
                let blank = self
                    .editing
                    .as_ref()
                    .is_none_or(|e| e.text.trim().is_empty());
                self.commit_pending();
                if let Some(card) = card.filter(|_| !blank && self.data.item(item).is_some()) {
                    return self.add_item(card, Some(item));
                }
            }
            Message::ToggleItem(item) => {
                self.commit_pending();
                self.change_checklist(item, |data| data.toggle_item(item));
            }
            Message::MoveItem(item, direction) => {
                let field = Field::ChecklistItem(item);
                let was_editing = self.editing.as_ref().is_some_and(|e| e.field == field);
                self.commit_pending();
                let by = if direction == Direction::Up { -1 } else { 1 };
                self.change_checklist(item, |data| data.move_item(item, by));
                if was_editing && self.data.item(item).is_some() {
                    return self.update(Message::StartEdit(field));
                }
            }
            Message::MoveEditedItem(direction) => {
                if let Some(Field::ChecklistItem(item)) = self.editing.as_ref().map(|e| e.field) {
                    return self.update(Message::MoveItem(item, direction));
                }
            }
            Message::RemoveItem(item) => {
                self.commit_pending();
                self.change_checklist(item, |data| data.remove_item(item));
            }
            Message::OpenLabels
            | Message::CloseLabels
            | Message::ToggleLabel(_)
            | Message::EditLabel(_)
            | Message::SetLabelColor(..)
            | Message::DeleteLabel(_)
            | Message::RemoveLabel(_)
            | Message::NewLabel
            | Message::NewLabelChanged(_)
            | Message::NewLabelColor(_)
            | Message::CreateLabel => return self.update_labels(message),
            Message::FocusFilter
            | Message::FilterChanged(_)
            | Message::SubmitFilter
            | Message::ClearFilter
            | Message::ToggleFilterMenu
            | Message::FilterLabel(_)
            | Message::LeaveField => return self.update_filter(message),
            Message::OpenSearch
            | Message::SearchChanged(_)
            | Message::SearchMove(_)
            | Message::SearchPick(_)
            | Message::CloseSearch => return self.update_search(message),
            Message::OpenMove => self.open_move(),
            Message::CloseMove => self.move_picker = None,
            Message::MoveToBoard(card, board) => {
                self.move_to_board(card, board);
                return self.reveal_selection();
            }
            Message::ExportBoard(_)
            | Message::Exported(_)
            | Message::ImportBoard
            | Message::Imported(_) => return self.update_transfer(message),
            Message::SaveImage(_) | Message::ImageSaved(_) => return self.update_image(message),
            Message::AddFile(_)
            | Message::FileChosen(..)
            | Message::RemoveFile(..)
            | Message::OpenFile(_) => return self.update_files(message),
            Message::MoveSelection(direction) => {
                self.commit_pending();
                if let Some(board) = self.board() {
                    let shown = |card: &Card| self.shown(card);
                    let to = moved(board, self.selection, direction, shown);
                    // Arrows browse faces: a move closes the open Card, Enter opens one.
                    self.open &= to.is_none() || to == self.selection;
                    self.selection = to.or(self.selection);
                }
                return self.reveal_selection();
            }
            Message::MoveCard(direction) => {
                self.commit_pending();
                if let Some(gap) = self.column_move(direction) {
                    let column = self.selection.expect("a selected Column").column;
                    self.data.move_column(column, gap);
                    self.record_step(self.current_board);
                    return self.reveal_selection();
                }
                let target = self.board().zip(self.selection);
                let shown = |card: &Card| self.shown(card);
                if let Some((card, column, index)) = target
                    .and_then(|(board, selection)| card_move(board, selection, direction, shown))
                {
                    self.data.move_card(card, column, index);
                    self.selection = Some(Selection {
                        column,
                        card: Some(card),
                    });
                    self.record_step(self.current_board);
                    return self.reveal_selection();
                }
            }
            Message::Undo | Message::Redo => {
                // An unchanged edit just closes; an open, empty Capture box holds no edit, so
                // Ctrl+Z there undoes the last Card as usual.
                let editing = self.editing.is_some();
                if !self.commit_pending() && editing && message == Message::Undo {
                    return Task::none();
                }
                self.drag = None;
                let step = match message {
                    Message::Undo => self.history.undo(&mut self.data.boards),
                    _ => self.history.redo(&mut self.data.boards),
                };
                if let Some(Some(show)) = step {
                    self.current_board = Some(show);
                }
            }
            Message::Key(event) => {
                return selector::find(selector::is_focused())
                    .map(move |focused| Message::KeyWithFocus(event.clone(), focused));
            }
            Message::KeyWithFocus(event, focused) => {
                let item_or_edit = match self.editing.as_ref().map(|e| e.field) {
                    Some(Field::ChecklistItem(_)) => Input::Item,
                    Some(Field::CardTitle(_)) => Input::CardTitle,
                    _ => Input::Edit,
                };
                let input = |target: selector::Target| {
                    let selector::Target::Focusable { id: Some(id), .. } = target else {
                        return None;
                    };
                    [
                        (BOARD_NAME_INPUT, Input::BoardName),
                        (CAPTURE_INPUT, Input::Capture),
                        (CAPTURE_DESCRIPTION, Input::CaptureDescription),
                        (EDIT_INPUT, item_or_edit),
                        (description::EDITOR, Input::CardDescription),
                        (LIMIT_INPUT, Input::Edit), // a text box: keys go to it
                        (labels::NEW_INPUT, Input::Edit),
                        (filter::INPUT, Input::Filter),
                        (crate::search::INPUT, Input::Search),
                        (crate::due::INPUT, Input::Due),
                    ]
                    .into_iter()
                    .find_map(|(name, input)| (id == widget::Id::new(name)).then_some(input))
                };
                let focused = focused.and_then(input);
                let dragging = self.drag.is_some();
                let message = keys::map(&event, &Context { focused, dragging });
                // Inside the open Card the arrows, Enter and F2 act on its parts, not the Board.
                let message = match message {
                    Some(m) if self.open_card().is_none() => Some(m),
                    Some(Message::MoveSelection(Direction::Left | Direction::Right)) => None,
                    Some(Message::MoveSelection(direction)) => Some(Message::MovePart(direction)),
                    Some(Message::OpenCard) => Some(Message::CloseCard),
                    Some(Message::EditTitle) => Some(Message::EditPart),
                    m => m,
                };
                // A dialog leaves only Esc; the Archive view also Undo/Redo and F1.
                let allowed = |m: &Message| match m {
                    Message::Escape => true,
                    _ if self.dialog_open() => false,
                    Message::Undo | Message::Redo | Message::ShowShortcuts => true,
                    Message::OpenSearch | Message::SearchMove(_) => true,
                    _ => self.archive.is_none() && self.search.is_none(),
                };
                if let Some(message) = message.filter(allowed) {
                    return self.update(message);
                }
            }
            Message::StartEdit(field) => {
                self.commit_pending();
                self.board_menu = false;
                self.column_menu = None;
                let id = field.id();
                let start = match field {
                    Field::Description(card) => self.data.description(card),
                    Field::Due(card) => {
                        self.data.card(card).map(|c| c.due.as_deref().unwrap_or(""))
                    }
                    _ => self.data.name(id),
                };
                if let Some(start) = start.map(String::from) {
                    let focus = match field {
                        Field::Description(_) => {
                            self.editor = text_editor::Content::with_text(&start);
                            description::EDITOR
                        }
                        Field::Due(_) => crate::due::INPUT,
                        _ => EDIT_INPUT,
                    };
                    self.editing = Some(PendingEdit {
                        field,
                        text: start.clone(),
                        start,
                        typed_at: None,
                    });
                    self.part = match field {
                        Field::CardTitle(_) => Part::Title,
                        Field::Description(_) => Part::Description,
                        Field::ChecklistItem(item) => Part::Item(item),
                        Field::Due(_) => Part::Due,
                        _ => self.part,
                    };
                    // A Card's title and description are edited in the open Card: select and
                    // open it, so it shows and is in view.
                    if let Field::CardTitle(card) | Field::Description(card) = field
                        && let Some(column) = self.data.column_of_card(card)
                    {
                        let card = Some(card);
                        self.selection = Some(Selection { column, card });
                        self.open = true;
                        return Task::batch([operation::focus(focus), self.reveal_selection()]);
                    }
                    return operation::focus(focus);
                }
            }
            Message::EditChanged(text) => {
                if let Some(edit) = &mut self.editing {
                    if let Field::Due(_) = edit.field {
                        // The stored form is ISO: the typed text waits in the edit for the commit.
                        edit.text = text;
                        return Task::none();
                    }
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
            Message::CommitEdit => {
                self.commit_pending();
                // The Due box stays on screen: let go of it, so the next Enter closes the Card.
                // Focusing an Id no widget has unfocuses every widget.
                return operation::focus("none");
            }
            Message::DueChanged(card, text) => {
                if self.editing.as_ref().map(|e| e.field) != Some(Field::Due(card)) {
                    // Drop StartEdit's focus task: refocusing the typed-in box moves its cursor
                    // to the end.
                    let _ = self.update(Message::StartEdit(Field::Due(card)));
                }
                return self.update(Message::EditChanged(text));
            }
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
            Message::Saved(result, rotation) => {
                self.rotation = rotation;
                // The job has dropped its share by now, so this moves rather than copies.
                let attempted = (self.writing.take())
                    .map(|s| Arc::try_unwrap(s).unwrap_or_else(|s| (*s).clone()));
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
                    self.record_step(Some(board));
                }
            }
            Message::ToggleColumnMenu(column) => {
                self.commit_pending();
                self.limit_input = None;
                self.column_menu = (self.column_menu != Some(column)).then_some(column);
            }
            Message::ToggleCollapse(column) => {
                self.commit_pending();
                self.column_menu = None;
                if self.data.toggle_collapse(column).is_some() {
                    self.record_step(self.data.board_of_column(column));
                }
            }
            Message::OpenLimit(column) => {
                self.commit_pending();
                self.column_menu = None;
                if let Some(col) = self.data.column(column) {
                    let text = col.limit.map(|l| l.to_string()).unwrap_or_default();
                    self.limit_input = Some(LimitInput {
                        column,
                        text,
                        refused: false,
                    });
                    return operation::focus(LIMIT_INPUT);
                }
            }
            Message::LimitChanged(text) => {
                if let Some(input) = &mut self.limit_input {
                    input.text = text;
                    input.refused = false;
                }
            }
            Message::SubmitLimit => {
                if let Some(LimitInput { column, text, .. }) = self.limit_input.clone() {
                    match self.data.set_limit(column, &text) {
                        Some(()) => {
                            self.limit_input = None;
                            self.record_step(self.data.board_of_column(column));
                        }
                        None => self.limit_input.as_mut().expect("open").refused = true,
                    }
                }
            }
            Message::CopySaved(Some(Err(e))) => self.not_saved = Some(e),
            Message::CopySaved(None) => {}
            Message::OpenBoard(board) => {
                self.commit_pending();
                self.search = None;
                if self.data.active_board(board).is_some() {
                    self.current_board = Some(board);
                }
            }
            Message::CycleBoard(forward) => {
                let active: Vec<Id> = (self.data.boards.iter())
                    .filter(|b| !b.archived)
                    .map(|b| b.id)
                    .collect();
                let n = active.len();
                let at = self
                    .current_board
                    .and_then(|c| active.iter().position(|id| *id == c));
                let next = at.map_or(0, |i| if forward { i + 1 } else { i + n - 1 });
                if let Some(board) = (n > 0).then(|| active[next % n]) {
                    return self.update(Message::OpenBoard(board));
                }
            }
            Message::ArchiveBoard(board) => {
                self.commit_pending();
                if self.data.archive_board(board).is_some() {
                    self.archived_open = true; // show where it went
                    self.record_step(None);
                }
            }
            Message::AskDeleteBoard(id) => {
                if let Some(board) = self.data.boards.iter().find(|b| b.id == id && b.archived) {
                    let active: usize = board.columns.iter().map(|c| c.cards.len()).sum();
                    let archived = board.archived_cards.len();
                    let prompt = format!(
                        "Permanently delete Board \"{}\" with {} ({archived} archived)? \
                         You can undo this until Kanvibe closes.",
                        board.name,
                        crate::domain::cards(active + archived)
                    );
                    let action = Message::DeleteBoard(id);
                    self.confirm = Some(Confirm { prompt, action });
                }
            }
            Message::DeleteBoard(board) => {
                self.commit_pending();
                if self.data.delete_board(board).is_some() {
                    self.record_step(None);
                }
            }
            Message::ToggleSidebar => self.sidebar_collapsed = !self.sidebar_collapsed,
            Message::SetTheme(skin) => self.skin = skin,
            Message::ShowShortcuts => self.shortcuts = true,
            Message::OpenDataFolder => {
                if let Some(dir) = &self.data_dir {
                    crate::chrome::open_path(dir);
                }
            }
            Message::Maximized(maximized) => self.window.set_maximized(maximized),
            Message::Resized(size) => {
                self.window.resized(size);
                return crate::chrome::query_maximized();
            }
            Message::Moved(pos) => {
                self.window.moved(pos);
                return crate::chrome::query_maximized();
            }
            Message::ToggleArchivedBoards => self.archived_open = !self.archived_open,
            Message::ToggleBoardMenu => {
                self.commit_pending();
                self.board_menu = !self.board_menu;
            }
            Message::Confirm => {
                if let Some(confirm) = self.confirm.take() {
                    return self.update(confirm.action);
                }
            }
            Message::RestoreBoard(board) => {
                self.commit_pending();
                if self.data.restore_board(board).is_some() {
                    self.current_board = Some(board);
                    self.record_step(None);
                }
            }
            Message::DragPress(item) => {
                self.commit_pending();
                self.drag = Some(Drag::new(item));
            }
            Message::DragMoved(to) => {
                if let Some(drag) = &mut self.drag {
                    drag.moved(to);
                }
            }
            Message::DragHover(target) => {
                if let Some(drag) = &mut self.drag {
                    drag.hover(target);
                }
            }
            Message::DragLeave(target) => {
                if let Some(drag) = &mut self.drag {
                    drag.leave(target);
                }
            }
            Message::DragEnd => {
                let Some(drag) = self.drag.take() else {
                    return Task::none(); // the second of the local and global release
                };
                match (drag.item, drag.target()) {
                    (Item::Card(card), _) if !drag.started => {
                        if let Some(column) = self.data.column_of_card(card) {
                            let card = Some(card);
                            return self.update(Message::Select(Selection { column, card }));
                        }
                    }
                    (Item::Board(board), _) if !drag.started => {
                        return self.update(Message::OpenBoard(board));
                    }
                    (Item::Column(column), _) if !drag.started => {
                        // A click on a collapsed Column's strip expands it.
                        if self.data.column(column).is_some_and(|c| c.collapsed) {
                            return self.update(Message::ToggleCollapse(column));
                        }
                    }
                    (Item::Card(card), Some(Target::Board(board))) => {
                        return self.update(Message::MoveToBoard(card, board));
                    }
                    (Item::Card(card), Some(target)) => self.drop_card(card, target),
                    (Item::Column(column), Some(Target::ColumnGap(gap))) => {
                        self.data.move_column(column, gap);
                        self.record_step(self.data.board_of_column(column));
                    }
                    (Item::Board(board), Some(Target::BoardGap(gap))) => {
                        self.data.move_board(board, gap);
                        self.record_step(None);
                    }
                    _ => {} // released outside any target, or on the wrong kind: cancel
                }
            }
            Message::DragCancel => self.drag = None,
            Message::DragTick => {
                if let Some(drag) = self.drag.as_ref().filter(|d| d.started)
                    && let Some(cursor) = drag.cursor
                {
                    let column = match drag.target() {
                        Some(Target::Card { column, .. } | Target::Column(column)) => Some(column),
                        _ => None,
                    };
                    return drag::autoscroll(cursor, BOARD_SCROLL.into(), column.map(cards_widget));
                }
            }
        }
        Task::none()
    }

    /// Moves `card` into the slot `target`; dropping where it was records no step.
    fn drop_card(&mut self, card: Id, target: Target) {
        let (column, gap) = match target {
            Target::Card { column, index } => (column, index),
            Target::Column(column) => (column, usize::MAX),
            _ => return,
        };
        let col = self.data.column(column);
        let cards = col.map_or(&[][..], |c| &c.cards);
        let gap = match col.is_some_and(|c| c.collapsed) {
            true => cards.len(), // a collapsed Column's strip means "append"
            false => filter::placement(cards, gap, |c| self.shown(c)),
        };
        // The Card leaves its own slot first, so a later gap in its Column moves up by one.
        let from = cards.iter().position(|x| x.id == card);
        let index = match from {
            Some(i) if i < gap => gap - 1,
            _ => gap,
        };
        self.data.move_card(card, column, index);
        self.record_step(self.data.board_of_column(column));
    }

    /// A multi-line paste into the Capture box: one Card per non-blank line, as one undo step,
    /// the last one selected. The box's text (typed and pasted) is replaced by the Cards and the
    /// box stays open, empty. A single line changes nothing: the box already holds it.
    fn capture_lines(&mut self, text: String) -> Task<Message> {
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let Some(capture) = self.capture.as_mut().filter(|_| lines.len() > 1) else {
            return Task::none();
        };
        capture.text.clear();
        let column = capture.column;
        for line in lines {
            if let Some(card) = self.data.add_captured(column, line) {
                let card = Some(card);
                self.selection = Some(Selection { column, card });
                if self.filter.is_on() {
                    self.filter_kept.extend(card);
                }
            }
        }
        self.record_step(self.data.board_of_column(column));
        focus_capture(column)
    }

    /// Opens the selected Card, if any, and returns it.
    pub(crate) fn open_selected(&mut self) -> Option<Id> {
        let card = self.selection.and_then(|s| s.card);
        self.open |= card.is_some();
        card
    }

    /// The open Card: the selected Card while `open` is set.
    pub fn open_card(&self) -> Option<Id> {
        self.selection.and_then(|s| s.card).filter(|_| self.open)
    }

    /// Records an undo step (if anything changed) that shows `board` on undo and on redo;
    /// `None` keeps the current Board. `true` when a step was recorded.
    pub(crate) fn record_step(&mut self, board: Option<Id>) -> bool {
        self.history.record(&self.data.boards, board, board)
    }

    /// Expands `column` if it is collapsed, as an undo step on its Board.
    pub(crate) fn expand(&mut self, column: Id) {
        if self.data.column(column).is_some_and(|c| c.collapsed) {
            self.data.toggle_collapse(column);
            self.record_step(self.data.board_of_column(column));
        }
    }

    /// A text box with state is open: the Board name prompt, Capture box, pending edit or limit.
    pub(crate) fn field_open(&self) -> bool {
        self.board_name.is_some()
            || self.capture.is_some()
            || self.editing.is_some()
            || self.limit_input.is_some()
    }

    /// An overlay that leaves only Esc to the keyboard: shortcuts, notice, close dialog,
    /// confirmation, Label picker or Move list.
    fn dialog_open(&self) -> bool {
        self.shortcuts
            || self.notice.is_some()
            || self.closing_dialog()
            || self.confirm.is_some()
            || self.label_picker.is_some()
            || self.move_picker.is_some()
    }

    /// The close dialog (Retry / Save a copy as… / Quit without saving) shows.
    fn closing_dialog(&self) -> bool {
        matches!(self.closing, Closing::Asking | Closing::Waiting) && self.not_saved.is_some()
    }

    /// Commits the text edit in progress, if any, as one undo step (none if nothing changed),
    /// and closes its field. A Capture box adds its Card. Call it first in every arm that
    /// changes domain data or UI context. `true` when a step was recorded.
    pub fn commit_pending(&mut self) -> bool {
        let mut recorded = false;
        if let Some(capture) = self.capture.take() {
            if let Some(card) = self.data.add_captured(capture.column, &capture.text) {
                let description = capture.description.trim();
                if !description.is_empty() {
                    self.data.set_description(card, description);
                }
                self.selection = Some(Selection {
                    column: capture.column,
                    card: Some(card),
                }); // new Cards become the selected Card
                if self.filter.is_on() {
                    self.filter_kept.push(card); // a Card just created stays visible
                }
            }
            recorded |= self.record_step(self.data.board_of_column(capture.column));
        }
        if let Some(edit) = self.editing.take() {
            let board = match edit.field {
                Field::BoardName(_) => None,
                Field::ColumnName(column) => self.data.board_of_column(column),
                Field::ChecklistItem(item) => {
                    let card = self.data.card_of_item(item);
                    if edit.text.trim().is_empty() {
                        // A blank Item is removed; one that was never non-blank leaves no trace.
                        self.data.remove_item(item);
                        if !edit.start.is_empty() {
                            card.inspect(|c| self.data.touch(*c));
                        }
                    } else if self.data.name(item) != Some(edit.start.as_str()) {
                        card.inspect(|c| self.data.touch(*c));
                    }
                    card.and_then(|c| self.data.board_of_card(c))
                }
                Field::LabelName(label) => self.data.board_of_label(label),
                Field::Description(card) => {
                    if edit.text.trim().is_empty() {
                        // Whitespace only is no description: "" is the one rule everywhere.
                        self.data.set_description(card, "");
                    }
                    if self.data.description(card) != Some(edit.start.as_str()) {
                        self.data.touch(card);
                    }
                    self.data.board_of_card(card)
                }
                Field::Due(card) => {
                    // Blank clears; a date that doesn't parse leaves the Card as it was.
                    let due = match edit.text.trim() {
                        "" => Some(None),
                        typed => crate::due::parse(typed).map(Some),
                    };
                    if let Some(due) = due {
                        self.data.set_due(card, due); // touches only on a change
                    }
                    self.data.board_of_card(card)
                }
                Field::CardTitle(card) => {
                    if self.data.name(card) != Some(edit.start.as_str()) {
                        self.data.touch(card); // a changed title is the Card's own change
                    }
                    self.data.board_of_card(card)
                }
            };
            recorded |= self.record_step(board);
        }
        recorded
    }

    /// Adds a blank Item to the Card and opens it for typing; the add is recorded on commit.
    fn add_item(&mut self, card: Id, after: Option<Id>) -> Task<Message> {
        match self.data.add_item(card, after) {
            Some(item) => self.update(Message::StartEdit(Field::ChecklistItem(item))),
            None => Task::none(),
        }
    }

    /// Applies a Checklist change to the Item's Card as one undo step.
    fn change_checklist(&mut self, item: Id, change: impl FnOnce(&mut Data) -> Option<()>) {
        let Some(card) = self.data.card_of_item(item) else {
            return;
        };
        if change(&mut self.data).is_some() {
            self.data.touch(card);
            self.record_step(self.data.board_of_card(card));
        }
    }

    /// The gap Ctrl+Left/Right moves the selected Column to, when the Selection is a Column with
    /// no Card; `None` otherwise or at the first/last Column.
    fn column_move(&self, direction: Direction) -> Option<usize> {
        let selection = self.selection.filter(|s| s.card.is_none())?;
        let columns = &self.board()?.columns;
        let at = columns.iter().position(|c| c.id == selection.column)?;
        match direction {
            Direction::Left => at.checked_sub(1),
            Direction::Right => (at + 1 < columns.len()).then_some(at + 2),
            _ => None,
        }
    }

    /// Scrolls the selected Column fully into view, and its Card into view within it.
    pub(crate) fn reveal_selection(&self) -> Task<Message> {
        let Some(s) = self.selection else {
            return Task::none();
        };
        let column = reveal(BOARD_SCROLL.into(), column_widget(s.column), true);
        let card = s.card.map_or(Task::none(), |card| {
            reveal(cards_widget(s.column), card_widget(card), false)
        });
        Task::batch([column, card])
    }

    /// Part of `validate_ui`: keeps the selection on the current Board. It follows its Card to
    /// another Column and is cleared when its Card or Column is gone. A Card in a collapsed
    /// Column is hidden, so then only its Column stays selected.
    fn validate_selection(&mut self) {
        let here = |column: &Id| {
            let board = self.data.board_of_column(*column);
            board.is_some() && board == self.current_board
        };
        let collapsed = |column: Id| self.data.column(column).is_some_and(|c| c.collapsed);
        self.selection = self.selection.and_then(|s| match s.card {
            Some(card) => self
                .data
                .column_of_card(card)
                .filter(here)
                .map(|column| Selection {
                    column,
                    card: (!collapsed(column)).then_some(card),
                }),
            None => here(&s.column).then_some(s),
        });
    }

    /// After any Board switch: each Board opens clean.
    fn left_board(&mut self) {
        self.selection = None;
        self.open = false;
        self.capture = None;
        self.column_menu = None;
        self.limit_input = None;
        self.board_menu = false;
        self.archive = None;
        self.search = None;
        self.clear_filter();
        self.filter_menu = false;
    }

    /// Runs at the end of every `update`: the Selection is cleared when its Card is no longer an
    /// active Card on the current Board, and the open Card closes when no Card is selected.
    /// The current Board is never archived or gone: it falls back to the next active Board
    /// below, else above (positions from `order`, the Board IDs before the update).
    fn validate_ui(&mut self, board: Option<Id>, order: &[Id]) {
        self.current_board = self.data.fallback(self.current_board, order);
        if self.current_board != board {
            self.left_board();
        }
        self.validate_selection();
        // Undo or redo can take away the Column a ⋯ menu or limit prompt is open on.
        let here = |c: &Id| {
            self.data
                .board_of_column(*c)
                .is_some_and(|b| self.current_board == Some(b))
        };
        self.column_menu = self.column_menu.filter(here);
        self.limit_input = self.limit_input.take().filter(|l| here(&l.column));
        self.validate_archive();
        self.validate_labels();
        self.validate_filter();
        self.validate_move();
        self.open &= self.open_card().is_some();
        if let Part::Item(item) = self.part
            && self.data.item(item).is_none()
        {
            self.part = Part::Checklist; // a blank Item left on Enter is removed
        }
    }

    /// The Card is an active Card of the current Board.
    pub(crate) fn on_board(&self, card: Id) -> bool {
        let board = self.data.board_of_card(card);
        board.is_some() && board == self.current_board
    }

    /// The current Board.
    pub(crate) fn board(&self) -> Option<&Board> {
        let id = self.current_board?;
        self.data.boards.iter().find(|b| b.id == id)
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
            ) => Some(Message::LeaveField),
            _ => None,
        });
        let window = event::listen_with(|event, _status, _window| match event {
            Event::Window(window::Event::Resized(size)) => Some(Message::Resized(size)),
            Event::Window(window::Event::Moved(pos)) => Some(Message::Moved(pos)),
            _ => None,
        });
        Subscription::batch([
            keys,
            window,
            self.edit_subscription(),
            self.retry_subscription(),
            drag::subscription(self.drag.as_ref()),
        ])
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
        self.name_widget(field, name, size, None)
    }

    /// [`Self::editable_name`], whose label also starts a drag of `drag` on press.
    fn name_widget<'a>(
        &'a self,
        field: Field,
        name: &'a str,
        size: u32,
        drag: Option<Item>,
    ) -> Element<'a, Message> {
        // Board and Column names are headings; Card titles stay body text.
        let font = match field {
            Field::BoardName(_) | Field::ColumnName(_) => HEADING,
            _ => iced::Font::DEFAULT,
        };
        // Column names are muted so the Cards lead.
        let tone = match field {
            Field::ColumnName(_) => text::secondary,
            _ => text::default,
        };
        match &self.editing {
            Some(edit) if edit.field == field && !matches!(field, Field::CardTitle(_)) => {
                text_input("Name", &edit.text)
                    .id(EDIT_INPUT)
                    .size(size)
                    .font(font)
                    .on_input(Message::EditChanged)
                    .on_submit(Message::CommitEdit)
                    .into()
            }
            _ => {
                // A Card's title wraps: the open Card is where it is read in full.
                let wrapping = match field {
                    Field::CardTitle(_) => text::Wrapping::Word,
                    _ => text::Wrapping::None,
                };
                let label = mouse_area(
                    text(name)
                        .size(size)
                        .font(font)
                        .wrapping(wrapping)
                        .style(tone),
                )
                .on_double_click(Message::StartEdit(field));
                let label = match drag {
                    Some(item) => label
                        .on_press(Message::DragPress(item))
                        .on_release(Message::DragEnd),
                    None => label,
                };
                // The open Card's title names its shortcut too.
                let hint = matches!(field, Field::CardTitle(_))
                    .then(|| text("Rename (F2)").size(META_SIZE));
                let full = column![text(name)].extend(hint.map(Into::into));
                clipped(label, full).width(Fill).into()
            }
        }
    }

    /// The app over the theme's page (a gradient on some themes).
    pub fn view(&self) -> Element<'_, Message> {
        container(self.view_app())
            .width(Fill)
            .height(Fill)
            .style(crate::skin::page)
            .into()
    }

    fn view_app(&self) -> Element<'_, Message> {
        let content = match self.board() {
            _ if self.search.is_some() => self.view_search(),
            Some(board) if self.archive == Some(board.id) => self.view_archive(board),
            Some(board) => self.view_board(board),
            None => self.view_empty(),
        };
        let content: Element<_> = match self.data.boards.is_empty() {
            true => content, // first run: only "Create your first Board"
            false => row![crate::sidebar::view(self), content].into(),
        };
        // Clicks no widget took: leaving the description editor by clicking outside it.
        let content: Element<_> = mouse_area(content).on_press(Message::ClickOutside).into();
        let content: Element<_> = match &self.label_picker {
            Some(picker) => stack![content, self.view_label_picker(picker)].into(),
            None => content,
        };
        let content: Element<_> = match self.move_picker {
            Some(card) => stack![content, self.view_move_picker(card)].into(),
            None => content,
        };
        let content: Element<_> = match &self.not_saved {
            Some(error) => column![
                container(
                    row![
                        text(error).width(Fill),
                        button("Retry now")
                            .style(quiet)
                            .on_press_maybe(self.writing.is_none().then_some(Message::RetrySave)),
                    ]
                    .spacing(8)
                    .align_y(Center)
                )
                .width(Fill)
                .padding(8)
                .style(container::danger),
                content
            ]
            .into(),
            None => content,
        };
        let content = match self.ghost() {
            Some(ghost) => stack![content, ghost].into(),
            None => content,
        };
        if self.shortcuts {
            return stack![content, self.view_shortcuts()].into();
        }
        if let Some(notice) = self.view_notice() {
            return stack![content, notice].into();
        }
        if self.closing_dialog() {
            return stack![content, self.view_close_dialog()].into();
        }
        match &self.confirm {
            Some(confirm) => stack![content, view_confirm(confirm)].into(),
            None => content,
        }
    }

    /// The dragged Card under the cursor, once the drag has started.
    fn ghost(&self) -> Option<Element<'_, Message>> {
        let drag = self.drag.as_ref().filter(|d| d.started)?;
        let (Item::Card(id) | Item::Column(id) | Item::Board(id)) = drag.item;
        Some(drag::ghost(self.data.name(id)?, drag.cursor?))
    }

    /// Retry / Save a copy as… / Quit without saving, over the dimmed app.
    /// The buttons wait while a save is in flight, so exit never cuts one off.
    fn view_close_dialog(&self) -> Element<'_, Message> {
        let idle = |message| self.writing.is_none().then_some(message);
        let buttons = row![
            iced::widget::space::horizontal(),
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
        .width(560);
        dialog(panel)
    }

    fn view_board<'a>(&'a self, board: &'a Board) -> Element<'a, Message> {
        // Gaps sit between Columns (and at both ends); only a Column drag registers them.
        let dragged = matches!(self.drag.as_ref().map(|d| d.item), Some(Item::Column(_)));
        let hovered = self.drag.as_ref().and_then(Drag::target);
        let gap = |i: usize| {
            let line = drag::indicator_v(hovered == Some(Target::ColumnGap(i)));
            drag::target(mouse_area(line), Target::ColumnGap(i), dragged).into()
        };
        let mut columns: Vec<Element<_>> = Vec::new();
        let now = jiff::Timestamp::now(); // ageing and due badges: one clock read per view
        let today = crate::due::today(now);
        for (i, c) in board.columns.iter().enumerate() {
            columns.extend([gap(i), self.view_column(c, now, today)]);
        }
        columns.extend([
            gap(board.columns.len()),
            // A ghost of a Column: its width, a hairline edge, muted text.
            button(text("+ Add Column").size(COLUMN_NAME_SIZE))
                .width(COLUMN_WIDTH)
                .height(Fill)
                .padding([10, 12])
                .style(|theme: &Theme, status| button::Style {
                    border: Border::default()
                        .color(theme.extended_palette().background.weak.color)
                        .width(1)
                        .rounded(COLUMN_RADIUS),
                    ..faded(0.7)(theme, status)
                })
                .on_press(Message::AddColumn(board.id))
                .into(),
        ]);
        let name = self.editable_name(Field::BoardName(board.id), &board.name, BOARD_TITLE_SIZE);
        let archive = button(
            text(format!("Archive ({})", board.archived_cards.len()))
                .wrapping(text::Wrapping::None),
        )
        .style(quiet)
        .on_press(Message::OpenArchive);
        let menu_button = crate::chrome::tip(
            button("⋯")
                .style(button::text)
                .on_press(Message::ToggleBoardMenu),
            "Board menu",
        );
        // A narrow Board area puts the filter on its own line, so the name, Archive and ⋯
        // keep their room.
        let header: Element<_> = if self.board_width() < NARROW {
            column![
                row![name, archive, menu_button].spacing(8).align_y(Center),
                self.filter_bar(Fill),
            ]
            .spacing(8)
            .into()
        } else {
            row![name, self.filter_bar(220.into()), archive, menu_button]
                .spacing(8)
                .align_y(Center)
                .into()
        };
        let menu = self.board_menu.then(|| {
            row![
                button("Rename")
                    .style(quiet)
                    .on_press(Message::StartEdit(Field::BoardName(board.id))),
                button("Export Board…")
                    .style(quiet)
                    .on_press(Message::ExportBoard(board.id)),
                button(match self.saving_image {
                    true => "Saving image…",
                    false => "Save as image…",
                })
                .style(quiet)
                .on_press_maybe((!self.saving_image).then_some(Message::SaveImage(board.id))),
                button("Archive Board")
                    .style(quiet)
                    .on_press(Message::ArchiveBoard(board.id)),
            ]
            .spacing(8)
        });
        // Under the ⋯ that opened it.
        let menu = menu.map(|m| container(m).align_right(Fill).into());
        // The outer gaps are drop targets too: the header is inset by one so it lines up
        // with the first Column.
        let top = column![header]
            .extend(menu)
            .extend(self.filter_lines(board))
            .spacing(12)
            .padding([0.0, drag::GAP]);
        column![top]
            .push(
                // Embedded scrollbars take their own lane instead of covering the content.
                scrollable(row(columns).height(Fill))
                    .id(BOARD_SCROLL)
                    .direction(widget::scrollable::Direction::Horizontal(scrollbar(8)))
                    .height(Fill),
            )
            .spacing(12)
            .padding(Padding::new(6.0).top(14).bottom(12))
            .into()
    }

    /// The Board area's width (window less sidebar); unbounded before the window
    /// reports a size.
    fn board_width(&self) -> f32 {
        let Some(size) = self.window.size() else {
            return f32::INFINITY;
        };
        let sidebar = match (self.data.boards.is_empty(), self.sidebar_collapsed) {
            (true, _) => 0,
            (false, true) => crate::sidebar::RAIL,
            (false, false) => crate::sidebar::WIDTH,
        };
        size.width - sidebar as f32
    }

    fn view_column<'a>(
        &'a self,
        col: &'a Column,
        now: jiff::Timestamp,
        today: jiff::civil::Date,
    ) -> Element<'a, Message> {
        let selection = self.selection.filter(|s| s.column == col.id);
        // Only a Card drag registers Card slots and Column bodies; Column drags use gaps.
        let dragging = matches!(self.drag.as_ref().map(|d| d.item), Some(Item::Card(_)));
        if col.collapsed {
            return self.view_strip(col, selection.is_some(), dragging);
        }
        // The slot the drop indicator shows: before Card `gap`, or after the last.
        let gap = match self.drag.as_ref().and_then(Drag::target) {
            Some(Target::Card { column, index }) if column == col.id => Some(index),
            Some(Target::Column(column)) if column == col.id => Some(col.cards.len()),
            _ => None,
        };
        // Hidden Cards are skipped but keep their index: drop slots are positions in all Cards.
        let shown = col.cards.iter().enumerate().filter(|(_, c)| self.shown(c));
        let cards = shown.map(|(i, card)| {
            let selected = selection.is_some_and(|s| s.card == Some(card.id));
            let outline = outlined(card_style, selected);
            // The selected Card (the open one included) never fades.
            let stale = !selected && card.is_stale(now);
            // The Card being dragged stays in place, its text faded under the ghost.
            let lifted = self
                .drag
                .as_ref()
                .is_some_and(|d| d.started && d.item == Item::Card(card.id));
            // A title is edited in place (only on the open Card: editing opens it).
            let title: Element<_> = match self.editing.as_ref().map(|e| (e.field, &e.text)) {
                Some((Field::CardTitle(id), edit)) if id == card.id => text_input("Title", edit)
                    .id(EDIT_INPUT)
                    .size(CARD_TITLE_SIZE)
                    .on_input(Message::EditChanged)
                    .on_submit(Message::CommitEdit)
                    .into(),
                _ => text(&card.title).size(CARD_TITLE_SIZE).width(Fill).into(),
            };
            let card_box = match self.open_card() == Some(card.id) {
                true => self.view_open_card(card, title),
                false => card_face(self.board(), card, title, today),
            };
            let card_box = card_box.id(card_widget(card.id)).style(move |theme| {
                let style = fade_text(outline(theme), if stale { STALE_ALPHA } else { 1.0 });
                fade_text(style, if lifted { 0.4 } else { 1.0 })
            });
            // Indicator above, padding below: Card slots touch, so no gap between them
            // falls through to the Column body.
            let slot = container(column![drag::indicator(gap == Some(i)), card_box].spacing(2))
                .padding(iced::Padding::ZERO.bottom(2));
            let target = Target::Card {
                column: col.id,
                index: i,
            };
            let source =
                drag::source(slot, Item::Card(card.id)).on_double_click(Message::EditCard(card.id));
            drag::target(source, target, dragging).into()
        });
        let cards = cards.chain([drag::indicator(gap == Some(col.cards.len()))]);
        // An empty Column says what it takes.
        let hint = col.cards.is_empty().then(|| {
            container(
                column![
                    text("Drop a Card here").size(13),
                    text("or press N").size(13)
                ]
                .spacing(2)
                .align_x(Center),
            )
            .center_x(Fill)
            .padding([20, 12])
            .style(|theme: &Theme| container::Style {
                text_color: Some(theme.palette().text.scale_alpha(0.45)),
                ..container::Style::default()
            })
            .into()
        });
        let footer: Element<_> = match &self.capture {
            Some(capture) if capture.column == col.id => column![
                text_input("Card title (Enter adds, Esc cancels)", &capture.text)
                    .id(CAPTURE_INPUT)
                    .on_input(Message::CaptureChanged)
                    .on_paste(Message::CapturePasted)
                    .on_submit(Message::SubmitCapture),
                text_input("Description (Tab, optional)", &capture.description)
                    .id(CAPTURE_DESCRIPTION)
                    .size(13)
                    .on_input(Message::CaptureDescriptionChanged)
                    .on_submit(Message::SubmitCapture),
            ]
            .spacing(4)
            .into(),
            _ => crate::chrome::tip(
                button("+ Add Card")
                    .style(faded(0.7))
                    .on_press(Message::OpenCapture(col.id)),
                "Add Card (N)",
            ),
        };
        let count = count_badge(self.count_label(col), col.over_limit());
        let count = match col.limit_tip() {
            Some(tip) => crate::chrome::tip(count, tip),
            None => count,
        };
        // Dragged by the header only; the name label forwards the press (it captures its own).
        let item = Item::Column(col.id);
        let header = row![
            self.name_widget(
                Field::ColumnName(col.id),
                &col.name,
                COLUMN_NAME_SIZE,
                Some(item)
            ),
            count,
            crate::chrome::tip(
                button("⋯")
                    .style(faded(0.5))
                    .on_press(Message::ToggleColumnMenu(col.id)),
                "Column menu"
            ),
        ]
        .spacing(8)
        .align_y(Center);
        // A selected Column tints its header instead of outlining the whole Column.
        let header = container(header).padding([2, 4]).style({
            let (id, selected) = (col.id, selection.is_some());
            move |theme: &Theme| {
                crate::skin::header(theme, id, selected)
                    .unwrap_or_else(|| header_tint(selected)(theme))
            }
        });
        let header = drag::source(header, item);
        let menu: Option<Element<_>> = match &self.limit_input {
            Some(input) if input.column == col.id => Some(
                column![
                    text_input("Limit (blank removes)", &input.text)
                        .id(LIMIT_INPUT)
                        .on_input(Message::LimitChanged)
                        .on_submit(Message::SubmitLimit)
                ]
                .extend(input.refused.then(|| {
                    text("Enter a whole number of 1 or more, or leave blank to remove")
                        .size(META_SIZE)
                        .style(text::danger)
                        .into()
                }))
                .spacing(4)
                .into(),
            ),
            _ if self.column_menu == Some(col.id) => Some(
                // Full-width 14px items, so the menu reads as one block and Remove fits one line.
                column![
                    button(text("Rename").size(14))
                        .style(quiet)
                        .width(Fill)
                        .on_press(Message::StartEdit(Field::ColumnName(col.id))),
                    button(text("Set limit…").size(14))
                        .style(quiet)
                        .width(Fill)
                        .on_press(Message::OpenLimit(col.id)),
                    button(text("Collapse").size(14))
                        .style(quiet)
                        .width(Fill)
                        .on_press(Message::ToggleCollapse(col.id)),
                    button(
                        text(crate::archive::remove_column_text(col))
                            .size(14)
                            .style(text::danger),
                    )
                    .style(quiet)
                    .width(Fill)
                    .on_press(Message::RemoveColumn(col.id)),
                ]
                .spacing(4)
                .into(),
            ),
            _ => None,
        };
        // "+ Add Card" scrolls with the Cards, right after the last one, not pinned to the
        // bottom of the full-height Column.
        let footer = container(footer).padding(Padding::ZERO.top(4));
        let column_box = container(
            column![header]
                .extend(menu)
                .push(
                    scrollable(column(cards).extend(hint).push(footer))
                        .id(cards_widget(col.id))
                        .direction(widget::scrollable::Direction::Vertical(scrollbar(6)))
                        .height(Fill),
                )
                .spacing(8),
        )
        .id(column_widget(col.id))
        .width(COLUMN_WIDTH)
        .height(Fill)
        .padding(8)
        .style(column_style(col.id));
        // Cards and buttons capture their clicks, so this only fires on the Column's empty area.
        let area = mouse_area(column_box).on_press(Message::Select(Selection {
            column: col.id,
            card: None,
        }));
        drag::target(area, Target::Column(col.id), dragging).into()
    }

    /// A collapsed Column: a [`STRIP_WIDTH`] strip with the count over the clipped name (full
    /// name in a tooltip). A click expands it, a drag moves the Column, a Card dropped on it
    /// goes to its bottom.
    fn view_strip<'a>(
        &'a self,
        col: &'a Column,
        selected: bool,
        dragging: bool,
    ) -> Element<'a, Message> {
        let hovered = self.drag.as_ref().and_then(Drag::target) == Some(Target::Column(col.id));
        let count = count_badge(self.count_label(col), col.over_limit());
        let strip = strip_face(col, count)
            .id(column_widget(col.id))
            .height(Fill)
            .style(outlined(column_style(col.id), selected || hovered));
        // A press without a drag is a click: `DragEnd` expands the Column.
        let area = drag::source(strip, Item::Column(col.id));
        drag::target(area, Target::Column(col.id), dragging).into()
    }

    fn view_empty(&self) -> Element<'_, Message> {
        let content: Element<_> = match &self.board_name {
            Some(name) => column![text("Name your Board"), board_name_input(name).width(320)]
                .spacing(8)
                .into(),
            None if self.data.boards.is_empty() => button("Create your first Board")
                .on_press(Message::NewBoard)
                .into(),
            None => column![
                text("No open Boards").size(20),
                button("+ New Board").on_press(Message::NewBoard)
            ]
            .spacing(12)
            .align_x(Center)
            .into(),
        };
        // First run: the app's name above the one thing to do.
        let content: Element<_> = match self.data.boards.is_empty() {
            true => column![text("Kanvibe").size(28).font(HEADING), content]
                .spacing(12)
                .align_x(Center)
                .into(),
            false => content,
        };
        center(content).into()
    }
}

impl App {
    /// The open Card: the selected Card expanded in place, unstyled. Its face's title, then
    /// Labels, description, Checklist, and a footer with the Card's actions and times.
    fn view_open_card<'a>(
        &'a self,
        card: &'a Card,
        title: Element<'a, Message>,
    ) -> container::Container<'a, Message> {
        let tz = jiff::tz::TimeZone::system();
        let header = row![
            title,
            crate::chrome::tip(
                small_button("✕", Message::CloseCard).padding([0, 2]),
                "Close (Esc)"
            )
        ]
        .spacing(8);
        let header = self.part_box(Part::Title, header);
        let actions = row![
            crate::chrome::tip(
                small_button("Move to Board…", Message::OpenMove),
                "Move to Board (M)"
            ),
            crate::chrome::tip(
                small_button("Duplicate", Message::DuplicateCard),
                "Duplicate (Ctrl+D)"
            ),
            crate::chrome::tip(
                small_button("Archive", Message::ArchiveCard(card.id)),
                "Archive Card (Del)"
            ),
        ]
        .spacing(14)
        .wrap();
        // One time per line: both on one line overflow a 280px Column.
        let time = |label, at| {
            let at = local_time(at, &tz);
            text(format!("{label} {at}")).size(META_SIZE)
        };
        let times = column![
            time("Created", &card.created),
            time("Updated", &card.updated)
        ];
        container(
            column![
                header,
                widget::rule::horizontal(1),
                self.part_box(Part::Labels, self.view_labels(card)),
                self.part_box(Part::Description, self.view_description(card)),
                self.view_checklist(card),
                self.part_box(Part::Files, self.view_files(card)),
                self.part_box(Part::Due, self.view_due(card)),
                widget::rule::horizontal(1),
                column![actions, times].spacing(6),
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding([9, 11])
    }

    /// A part of the open Card, tinted while it is the highlighted one.
    pub(crate) fn part_box<'a>(
        &self,
        part: Part,
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        container(content)
            .padding([2, 4])
            .style(header_tint(self.part == part))
            .into()
    }
}

/// An RFC 3339 timestamp as "YYYY-MM-DD HH:MM" in `tz`; text that doesn't parse is shown as is.
pub fn local_time(timestamp: &str, tz: &jiff::tz::TimeZone) -> String {
    match timestamp.parse::<jiff::Timestamp>() {
        Ok(t) => t
            .to_zoned(tz.clone())
            .strftime("%Y-%m-%d %H:%M")
            .to_string(),
        Err(_) => timestamp.into(),
    }
}

/// A Card's face, unstyled: title, the description's first line, then one metadata row
/// (due badge, Label chips, Checklist progress, files marker).
/// Shared by the Board view and the image export.
pub(crate) fn card_face<'a>(
    board: Option<&'a Board>,
    card: &'a Card,
    title: Element<'a, Message>,
    today: jiff::civil::Date,
) -> container::Container<'a, Message> {
    let mut meta: Vec<Element<_>> = Vec::new();
    meta.extend(crate::due::badge(card, today));
    meta.extend(board.and_then(|b| labels::chips(b, card)));
    meta.extend(
        card.progress()
            // ✓, not ☑: that renders as a colour emoji competing with the chips.
            .map(|p| text(format!("✓ {p}")).size(META_SIZE).into()),
    );
    meta.extend(crate::files::marker(card));
    let meta = (!meta.is_empty()).then(|| metadata_row(meta).into());
    container(
        column![title]
            .extend(description::face_preview(card))
            .extend(meta)
            .spacing(4),
    )
    .width(Fill)
    .padding([9, 11])
}

/// A collapsed Column's strip, unstyled: `count` over the clipped name (full name in a
/// tooltip). Shared by the Board view and the image export.
pub(crate) fn strip_face<'a>(
    col: &'a Column,
    count: Element<'a, Message>,
) -> container::Container<'a, Message> {
    let name = text(&col.name)
        .size(COLUMN_NAME_SIZE)
        .font(HEADING)
        .style(text::secondary);
    let name = clipped(name.wrapping(text::Wrapping::None), text(&col.name)).width(Fill);
    container(column![count, name].spacing(8).align_x(Center))
        .width(STRIP_WIDTH)
        .padding([8, 4])
        .clip(true)
}

/// An open Card section's heading: small, so the Card's content leads.
pub(crate) fn section_heading<'a>(label: impl text::IntoFragment<'a>) -> widget::Text<'a> {
    text(label).size(META_SIZE).font(HEADING)
}

/// The open Card's small text buttons: "+ Add …", Edit and the footer's actions.
pub(crate) fn small_button<'a>(label: &'a str, message: Message) -> widget::Button<'a, Message> {
    button(text(label).size(META_SIZE))
        .style(faded(0.7))
        .padding([2, 0])
        .on_press(message)
}

/// An open Card section: its heading over its body, close together.
pub(crate) fn section<'a>(
    heading: impl Into<Element<'a, Message>>,
    body: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    column![heading.into()].extend(body).spacing(4).into()
}

/// The face's metadata (chips, badges, progress) as one row: one size, one spacing.
pub(crate) fn metadata_row<'a>(items: Vec<Element<'a, Message>>) -> widget::Row<'a, Message> {
    row(items).spacing(6).align_y(Center)
}

/// A Column's count as a small grey pill; over its limit, a danger-coloured one.
/// Shared by the Board view and the image export.
pub(crate) fn count_badge<'a>(label: String, over: bool) -> Element<'a, Message> {
    pill(label, if over { Tone::Danger } else { Tone::Neutral })
}

/// A [`pill`]'s colour.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Tone {
    Neutral,
    Warning,
    Danger,
}

/// A small rounded badge at [`META_SIZE`]: the count badge and the due badge.
pub(crate) fn pill<'a>(label: String, tone: Tone) -> Element<'a, Message> {
    container(text(label).size(META_SIZE))
        .padding([1, 7])
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            let (background, text) = match tone {
                Tone::Neutral => (
                    palette.background.weak.color,
                    palette.background.weak.text.scale_alpha(0.75),
                ),
                Tone::Warning => (palette.warning.base.color, palette.warning.base.text),
                Tone::Danger => (palette.danger.base.color, palette.danger.base.text),
            };
            container::Style {
                background: Some(background.into()),
                text_color: Some(text),
                border: Border::default().rounded(crate::skin::pill_radius(theme)),
                ..container::Style::default()
            }
        })
        .into()
}

/// A confirmation: the question, Cancel (as Esc) and the confirm button.
fn view_confirm(confirm: &Confirm) -> Element<'_, Message> {
    let buttons = row![
        iced::widget::space::horizontal(),
        button("Cancel").style(quiet).on_press(Message::Escape),
        button("Delete")
            .style(button::danger)
            .on_press(Message::Confirm),
    ]
    .spacing(8);
    dialog(
        column![text(&confirm.prompt), buttons]
            .spacing(12)
            .width(480),
    )
}

/// One line, clipped (iced has no ellipsis), with `full` in a tooltip below it.
pub(crate) fn clipped<'a>(
    line: impl Into<Element<'a, Message>>,
    full: impl Into<Element<'a, Message>>,
) -> container::Container<'a, Message> {
    let full = container(full).padding(4).style(container::bordered_box);
    container(tooltip(line, full, tooltip::Position::Bottom)).clip(true)
}

/// [`clipped`] for plain text, the full text in the tooltip.
pub(crate) fn clipped_text<'a>(
    s: impl text::IntoFragment<'a> + Clone,
) -> container::Container<'a, Message> {
    clipped(text(s.clone()).wrapping(text::Wrapping::None), text(s))
}

/// The new-Board name prompt: its text box and Create (disabled while blank).
pub(crate) fn board_name_input(name: &str) -> widget::Row<'_, Message> {
    let create = (!name.trim().is_empty()).then_some(Message::CreateBoard);
    row![
        text_input("Board name", name)
            .id(BOARD_NAME_INPUT)
            .on_input(Message::BoardNameChanged)
            .on_submit(Message::CreateBoard),
        button("Create").on_press_maybe(create),
    ]
    .spacing(4)
    .align_y(Center)
}

/// A dialog's title row with its ✕ (Esc) button sending `close`.
pub(crate) fn dialog_header(title: &str, close: Message) -> Element<'_, Message> {
    row![
        text(title).size(20),
        iced::widget::space::horizontal(),
        crate::chrome::tip(
            button("✕").style(button::text).on_press(close),
            "Close (Esc)"
        ),
    ]
    .align_y(Center)
    .into()
}

/// An overlay panel over the dimmed app, blocking clicks to what is below.
pub(crate) fn dialog<'a>(panel: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let dim = |_: &iced::Theme| container::Style {
        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..Default::default()
    };
    let panel = container(panel).padding(20).style(container::rounded_box);
    opaque(center(panel).style(dim))
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
pub(crate) fn reveal(scrollable: widget::Id, child: widget::Id, horizontal: bool) -> Task<Message> {
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

/// A `width`-px scrollbar embedded 4px from the content it scrolls.
fn scrollbar(width: u32) -> widget::scrollable::Scrollbar {
    widget::scrollable::Scrollbar::new()
        .width(width)
        .scroller_width(width)
        .spacing(4)
}

/// The secondary button: raised (weakest) background, 1px weak border, weaker background on
/// hover.
pub(crate) fn quiet(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base = button::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: palette.background.weakest.text,
        border: Border {
            color: palette.background.weak.color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    };
    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(palette.background.weaker.color.into()),
            ..base
        },
        // As iced's own disabled buttons: everything at half strength.
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.scale_alpha(0.5).into()),
            text_color: base.text_color.scale_alpha(0.5),
            border: base
                .border
                .color(palette.background.weak.color.scale_alpha(0.5)),
            ..base
        },
    }
}

/// A text button whose text is at `alpha` until hovered: occasional links and "+ Add Card".
pub(crate) fn faded(alpha: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let style = button::text(theme, status);
        match status {
            button::Status::Active => button::Style {
                text_color: style.text_color.scale_alpha(alpha),
                ..style
            },
            _ => style,
        }
    }
}

/// Text alpha on a stale Card's face (ageing).
pub(crate) const STALE_ALPHA: f32 = 0.55;

/// `style` with its text at `alpha`: a stale Card ([`STALE_ALPHA`]) or a lifted one.
/// Shared by the Board view and the image export.
pub(crate) fn fade_text(style: container::Style, alpha: f32) -> container::Style {
    container::Style {
        text_color: style.text_color.map(|c| c.scale_alpha(alpha)),
        ..style
    }
}

/// A Column (expanded or a strip): a tray (the weaker tier, a shade off the page), rounded,
/// flat (the image export's too). The Column picks its tint on the Tinted theme.
pub(crate) fn column_style(column: Id) -> impl Fn(&Theme) -> container::Style + Copy {
    move |theme| {
        if let Some(style) = crate::skin::column(theme, column) {
            return style;
        }
        let palette = theme.extended_palette();
        container::Style {
            background: Some(palette.background.weaker.color.into()),
            text_color: Some(palette.background.weaker.text),
            border: Border::default().rounded(COLUMN_RADIUS),
            ..container::Style::default()
        }
    }
}

/// A Card: the lightest tier with a hairline edge and a one-pixel shadow (the
/// image export's too).
pub(crate) fn card_style(theme: &Theme) -> container::Style {
    if let Some(style) = crate::skin::card(theme) {
        return style;
    }
    let bg = theme.extended_palette().background;
    container::Style {
        background: Some(bg.weakest.color.into()),
        text_color: Some(bg.weakest.text),
        border: Border::default()
            .color(bg.weak.color)
            .width(1)
            .rounded(CARD_RADIUS),
        shadow: iced::Shadow {
            color: iced::Color::BLACK.scale_alpha(0.06),
            offset: iced::Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ..container::Style::default()
    }
}

/// `base`, with a 2px primary-colour border when `selected`.
pub(crate) fn outlined(
    base: impl Fn(&Theme) -> container::Style,
    selected: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let style = base(theme);
        if !selected {
            return style;
        }
        if let Some(style) = crate::skin::selected(theme, style) {
            return style;
        }
        container::Style {
            border: style.border.color(theme.palette().primary).width(2),
            ..style
        }
    }
}

/// A Column header's selection: a low-alpha primary tint behind it.
pub(crate) fn header_tint(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: selected.then(|| theme.palette().primary.scale_alpha(0.12).into()),
        border: Border::default().rounded(CARD_RADIUS),
        ..container::Style::default()
    }
}

const BOARD_SCROLL: &str = "board";
/// A Column's width (the "+ Add Column" ghost's too).
pub(crate) const COLUMN_WIDTH: f32 = 280.0;
/// Text sizes: the Board title, Column names, Card titles, and face metadata (count badge,
/// Label chips, Checklist progress, small print).
pub(crate) const BOARD_TITLE_SIZE: u32 = 18;
pub(crate) const COLUMN_NAME_SIZE: u32 = 13;
pub(crate) const CARD_TITLE_SIZE: u32 = 14;
pub(crate) const META_SIZE: u32 = 12;
/// Text in the open Card's sections: Checklist Items, the empty description, the Due field.
pub(crate) const SECTION_TEXT_SIZE: u32 = 13;
/// Corner radii: Columns (and the "+ Add Column" ghost), Cards (and the header tint).
pub(crate) const COLUMN_RADIUS: f32 = 10.0;
pub(crate) const CARD_RADIUS: f32 = 6.0;
/// Gaps between Columns and between Cards. The live Board's Card gap is its drop slot: 2 px
/// padding below a Card, the 2 px indicator and 2 px spacing; the image export uses this.
pub(crate) const COLUMN_GAP: f32 = 12.0;
pub(crate) const CARD_GAP: f32 = 6.0;
/// A collapsed Column's strip (the image export's too).
pub(crate) const STRIP_WIDTH: f32 = 48.0;
/// Board and Column names (the image export's too).
pub(crate) const HEADING: iced::Font = iced::Font {
    weight: iced::font::Weight::Semibold,
    ..iced::Font::DEFAULT
};
/// Below this Board-area width the header's filter moves to its own line.
const NARROW: f32 = 640.0;

fn column_widget(column: Id) -> widget::Id {
    widget::Id::from(format!("column-{column}"))
}

/// Focuses the Capture box's title, at the end of the Column's scrolling list.
fn focus_capture(column: Id) -> Task<Message> {
    Task::batch([
        operation::focus(CAPTURE_INPUT),
        operation::snap_to_end(cards_widget(column)),
    ])
}

fn cards_widget(column: Id) -> widget::Id {
    widget::Id::from(format!("cards-{column}"))
}

fn card_widget(card: Id) -> widget::Id {
    widget::Id::from(format!("card-{card}"))
}

/// Where an arrow key takes the selection on `board`, counting only Cards `shown` by the
/// Filter; `None` = stay put.
fn moved(
    board: &Board,
    selection: Option<Selection>,
    direction: Direction,
    shown: impl Fn(&Card) -> bool,
) -> Option<Selection> {
    // A collapsed Column shows no Cards: the Selection lands on the Column itself.
    let visible = |column: &Column| -> Vec<Id> {
        column
            .cards
            .iter()
            .filter(|c| !column.collapsed && shown(c))
            .map(|c| c.id)
            .collect()
    };
    let at = |column: &Column, index: Option<usize>| Selection {
        column: column.id,
        card: index.and_then(|i| visible(column).get(i).copied()),
    };
    let Some(selection) = selection else {
        return board.columns.first().map(|c| at(c, Some(0)));
    };
    let (ci, _) = position(board, selection)?;
    let column = &board.columns[ci];
    let cards = visible(column);
    let index = selection
        .card
        .and_then(|id| cards.iter().position(|c| *c == id));
    match direction {
        Direction::Up => Some(at(column, Some(index?.checked_sub(1)?))),
        Direction::Down => {
            let next = index.map_or(0, |i| i + 1);
            (next < cards.len()).then(|| at(column, Some(next)))
        }
        Direction::Top => Some(at(column, Some(0))),
        Direction::Bottom => Some(at(column, Some(cards.len().saturating_sub(1)))),
        Direction::Left | Direction::Right => {
            let next = match direction {
                Direction::Left => board.columns.get(ci.checked_sub(1)?)?,
                _ => board.columns.get(ci + 1)?,
            };
            let i = index
                .unwrap_or(0)
                .min(visible(next).len().saturating_sub(1));
            Some(at(next, Some(i)))
        }
    }
}

/// Where Ctrl+arrow moves the selected Card: (Card, Column, index). `None` = no Card or at an end.
/// Up/Down go just before the previous / just after the next Card `shown` by the Filter;
/// Top/Bottom (Home/End) just before the first / just after the last other shown Card;
/// Left/Right go after the last shown Card of the neighbouring Column.
fn card_move(
    board: &Board,
    selection: Selection,
    direction: Direction,
    shown: impl Fn(&Card) -> bool,
) -> Option<(Id, Id, usize)> {
    let (ci, index) = position(board, selection)?;
    let (card, i) = (selection.card?, index?);
    let column = &board.columns[ci];
    let cards = &column.cards;
    match direction {
        Direction::Up => Some((card, column.id, cards[..i].iter().rposition(&shown)?)),
        Direction::Down => {
            let next = i + 1 + cards[i + 1..].iter().position(&shown)?;
            Some((card, column.id, next))
        }
        Direction::Top => {
            let first = cards.iter().position(&shown)?;
            (first < i).then_some((card, column.id, first))
        }
        Direction::Bottom => {
            let last = cards.iter().rposition(&shown)?;
            (last > i).then_some((card, column.id, last))
        }
        Direction::Left | Direction::Right => {
            let next = match direction {
                Direction::Left => board.columns.get(ci.checked_sub(1)?)?,
                _ => board.columns.get(ci + 1)?,
            };
            // After the last visible Card, as a drop on the Column body (the bottom unfiltered);
            // a collapsed Column shows none, so at its bottom.
            let at = match next.collapsed {
                true => next.cards.len(),
                false => filter::placement(&next.cards, next.cards.len(), &shown),
            };
            Some((card, next.id, at))
        }
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
        let storage::Loaded {
            data,
            settings,
            repaired,
        } = storage::open(&dir)?.unwrap_or_default();
        // The last open Board, else (archived, gone or unset) the first active one.
        let last = settings
            .last_board
            .filter(|b| data.active_board(*b).is_some());
        let current_board = last.or(data.boards.iter().find(|b| !b.archived).map(|b| b.id));
        Ok(App {
            sidebar_collapsed: settings.sidebar_collapsed,
            skin: settings.skin,
            window: crate::chrome::WindowState::new(settings.window, settings.maximized),
            saved: (data.clone(), settings),
            data,
            current_board,
            data_dir: Some(dir),
            // A repaired file didn't load cleanly: rotating would push a good backup out.
            rotation: if repaired {
                storage::Rotation::Off
            } else {
                storage::Rotation::Due
            },
            ..App::default()
        })
    }

    /// A save of the current state for the background writer, if the Boards or settings differ
    /// from what the data file holds. Runs after every `update`, never inside it.
    /// `None` while a save is in flight: its `Saved` result runs this again, so changes made
    /// meanwhile merge into one save of the latest state.
    pub fn save_if_changed(&mut self) -> Option<SaveJob> {
        let dir = self.data_dir.as_ref()?;
        if self.writing.is_some() || self.editing.as_ref().is_some_and(|e| e.typed_at.is_some()) {
            return None; // live text waits for the idle write (`EditTick`)
        }
        // Compared in place: this runs after every message (drag moves, ticks, resizes), so the
        // state is cloned only once a save is going to happen.
        let settings = self.settings();
        let now = (&self.data, &settings);
        if self.same(now, (&self.saved.0, &self.saved.1)) {
            self.not_saved = None; // e.g. undone back to what the file holds
            return None;
        }
        if (self.failed.as_ref()).is_some_and(|(d, s)| self.same((d, s), now)) {
            return None; // wait for a change or the retry timer
        }
        // Only a due rotation looks at whether Board data changed.
        let data_changed = self.rotation == storage::Rotation::Due && self.saved.0 != self.data;
        let state = Arc::new((self.data.clone(), settings));
        self.writing = Some(Arc::clone(&state));
        Some(SaveJob {
            dir: dir.clone(),
            state,
            rotation: self.rotation,
            data_changed,
        })
    }

    /// Exits once a requested close has nothing left to save and no save in flight;
    /// otherwise the next `save_if_changed` starts the save and its `Saved` comes back here.
    fn exit_if_saved(&mut self) -> Task<Message> {
        let clean = self.same(
            (&self.saved.0, &self.saved.1),
            (&self.data, &self.settings()),
        );
        if self.closing != Closing::Waiting || self.writing.is_some() || !clean {
            return Task::none();
        }
        self.closing = Closing::Exiting;
        iced::exit()
    }

    /// Whether two saved states match. The window is ignored (geometry never triggers a save on
    /// its own) except while closing, when it is saved too.
    fn same(&self, a: (&Data, &Settings), b: (&Data, &Settings)) -> bool {
        if self.closing != Closing::No {
            return a == b;
        }
        let bare = |s: &Settings| Settings {
            window: None,
            maximized: false,
            ..s.clone()
        };
        a.0 == b.0 && bare(a.1) == bare(b.1)
    }

    /// The settings saved beside the Boards.
    fn settings(&self) -> Settings {
        Settings {
            last_board: self.current_board,
            sidebar_collapsed: self.sidebar_collapsed,
            skin: self.skin,
            window: self.window.geometry,
            maximized: self.window.maximized,
        }
    }
}
