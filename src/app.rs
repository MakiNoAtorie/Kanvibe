//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input,
};
use iced::{Element, Event, Fill, Subscription, Task, event, keyboard};

use crate::domain::{Board, Column, Data, Id};
use crate::storage::{self, Settings};
use std::path::PathBuf;
use std::{fs, io};

const BOARD_NAME_INPUT: &str = "board-name";
const CAPTURE_INPUT: &str = "capture";

#[derive(Debug, Default)]
pub struct App {
    pub data: Data,
    pub current_board: Option<Id>,
    /// The new-Board name prompt; `Some` while it is open.
    pub board_name: Option<String>,
    /// The Capture box; at most one is open.
    pub capture: Option<Capture>,
    /// Where `kanvibe.json` lives; `None` (the default) saves nothing.
    pub data_dir: Option<PathBuf>,
    /// What the data file last saved holds, so only changes are saved.
    pub saved: (Data, Settings),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub column: Id,
    pub text: String,
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
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewBoard => {
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
                }
            }
            Message::OpenCapture(column) => {
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
                if let Some(capture) = &mut self.capture {
                    self.data.add_card(capture.column, &capture.text);
                    capture.text.clear();
                }
            }
            Message::Escape => {
                self.board_name = None;
                self.capture = None;
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // ponytail: Esc only, ignoring capture status (a focused text_input swallows Esc);
        // the key-mapping module replaces this.
        event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => Some(Message::Escape),
            _ => None,
        })
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
        let columns = board.columns.iter().map(|c| self.view_column(c));
        column![
            text(&board.name).size(20),
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
                text(&col.name),
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

impl App {
    /// Loads the data folder (created if missing) and reopens the last open Board, else the first.
    pub fn open(dir: PathBuf) -> io::Result<App> {
        fs::create_dir_all(&dir)?;
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

pub fn run() -> iced::Result {
    let Some(dir) = storage::data_dir(std::env::args().skip(1)) else {
        eprintln!("No local app data folder; pass --data-dir <path>");
        std::process::exit(1);
    };
    // ponytail: a load error exits so the bad file is never overwritten; #19 adds the problem screens.
    let app = match App::open(dir.clone()) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Can't open {}: {e}", dir.display());
            std::process::exit(1);
        }
    };
    let app = std::cell::Cell::new(Some(app));
    let boot = move || app.take().expect("iced boots once");
    let update = |app: &mut App, message| {
        let task = app.update(message);
        app.save_if_changed();
        task
    };
    iced::application(boot, update, App::view)
        .title("Kanvibe")
        .subscription(App::subscription)
        .run()
}
