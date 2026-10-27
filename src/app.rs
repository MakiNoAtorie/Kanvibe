//! The iced program: state, Message, `update` (the only place state changes) and `view`.
use iced::widget::{
    button, center, column, container, operation, row, scrollable, text, text_input,
};
use iced::{Element, Event, Fill, Subscription, Task, event, keyboard};

use crate::domain::{Board, Column, Data, Id};

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

pub fn run() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Kanvibe")
        .subscription(App::subscription)
        .run()
}
