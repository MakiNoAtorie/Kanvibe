//! Startup: the single-copy lock, then the app or a problem screen when the data file won't load.
use std::path::PathBuf;
use std::time::SystemTime;

use iced::widget::{button, center, column, container, row, text};
use iced::{Element, Subscription, Task};

use crate::app::{self, App};
use crate::storage::{self, LoadError};

/// What the window shows: the app, or a problem screen until the data file loads.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // one Screen per process
pub enum Screen {
    App(App),
    Problem(Problem),
}

/// A data file that didn't load, and the way out the screen offers.
#[derive(Debug)]
pub struct Problem {
    pub dir: PathBuf,
    pub error: LoadError,
    /// The newest readable backup and its date; "Restore from backup" is shown only with one.
    pub backup: Option<(PathBuf, SystemTime)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    App(app::Message),
    /// "Can't open": load again.
    Retry,
    /// Unreadable file: move it aside and copy the newest readable backup over it.
    Restore,
    /// Unreadable file: move it aside and write an empty one.
    StartEmpty,
    Quit,
}

impl Screen {
    /// Loads the data folder; any load error becomes its problem screen.
    pub fn open(dir: PathBuf) -> Screen {
        match App::open(dir.clone()) {
            Ok(app) => Screen::App(app),
            Err(error) => {
                let backup = match error {
                    LoadError::Unreadable(_) => storage::backup(&dir),
                    _ => None,
                };
                Screen::Problem(Problem { dir, error, backup })
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match (&mut *self, message) {
            (Screen::App(app), Message::App(message)) => {
                let task = app.update(message).map(Message::App);
                let Some(job) = app.save_if_changed() else {
                    return task;
                };
                // The background writer: a thread per save, one in flight at a time (see
                // `save_if_changed`), so the UI never waits on the disk.
                let (done, result) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let _ = done.send(job.run()); // receiver gone only if the app quit
                });
                let saved = Task::perform(result, |result| {
                    Message::App(result.unwrap_or(app::Message::Saved(Err(
                        "Not saved: the writer stopped".into(),
                    ))))
                });
                return Task::batch([task, saved]);
            }
            (_, Message::Quit) => return iced::exit(),
            (Screen::Problem(problem), Message::Retry) => {
                *self = Screen::open(problem.dir.clone());
            }
            (Screen::Problem(problem), message @ (Message::Restore | Message::StartEmpty)) => {
                let dir = problem.dir.clone();
                let fixed = match message {
                    Message::Restore => storage::restore(&dir),
                    _ => storage::start_empty(&dir),
                };
                *self = match fixed {
                    // No backup rotation this session: the backups are what's left of the old file.
                    Ok(()) => match Screen::open(dir) {
                        Screen::App(app) => Screen::App(App {
                            rotation: storage::Rotation::Off,
                            ..app
                        }),
                        screen => screen,
                    },
                    Err(e) => Screen::Problem(Problem {
                        dir,
                        error: LoadError::Io(e),
                        backup: None,
                    }),
                };
            }
            _ => {}
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self {
            Screen::App(app) => app.subscription().map(Message::App),
            // Closing is intercepted for the app (#6), so the problem screen quits on close itself.
            Screen::Problem(_) => iced::window::close_requests().map(|_| Message::Quit),
        }
    }

    pub fn theme(&self) -> iced::Theme {
        match self {
            Screen::App(app) => app.theme(),
            Screen::Problem(_) => iced::Theme::Light,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self {
            Screen::App(app) => app.view().map(Message::App),
            Screen::Problem(problem) => problem.view(),
        }
    }
}

impl Problem {
    // ponytail: a centred panel, not a `stack` overlay: nothing is loaded to show beneath it.
    fn view(&self) -> Element<'_, Message> {
        let path = storage::file_path(&self.dir);
        let quit = button("Quit").on_press(Message::Quit);
        let (title, detail, buttons) = match &self.error {
            LoadError::Newer => (
                "Install the newer version".to_owned(),
                format!(
                    "{} was saved by a newer version of Kanvibe. Install the newer version to open it.",
                    path.display()
                ),
                row![quit],
            ),
            LoadError::Io(e) => (
                "Can't open the data file".to_owned(),
                format!("Can't open {}: {e}", path.display()),
                row![button("Retry").on_press(Message::Retry), quit],
            ),
            LoadError::Unreadable(e) => {
                let mut buttons = row![];
                if let Some((_, modified)) = self.backup {
                    let label = format!("Restore from backup ({})", local_date(modified));
                    buttons = buttons.push(button(text(label)).on_press(Message::Restore));
                }
                (
                    "The data file can't be read".to_owned(),
                    format!("{}\n{e}", path.display()),
                    buttons
                        .push(button("Start empty").on_press(Message::StartEmpty))
                        .push(quit),
                )
            }
        };
        let panel = column![text(title).size(20), text(detail), buttons.spacing(8)]
            .spacing(12)
            .max_width(560);
        center(container(panel).padding(20).style(container::rounded_box)).into()
    }
}

/// `2026-10-27 14:03` in local time (UTC if the system time zone is unknown).
fn local_date(at: SystemTime) -> String {
    jiff::Timestamp::try_from(at)
        .map(|t| t.to_zoned(jiff::tz::TimeZone::system()))
        .map(|t| t.strftime("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// The program: lock first, before reading data or starting iced; then the app or a problem screen.
pub fn run() -> iced::Result {
    let Some(dir) = storage::data_dir(std::env::args().skip(1)) else {
        alert("No local app data folder; pass --data-dir <path>.");
        std::process::exit(1);
    };
    // Held for the life of the process; the OS releases it on exit or crash.
    let _lock = match storage::lock(&dir) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            alert("Kanvibe is already running. Look for it in the taskbar.");
            std::process::exit(0);
        }
        Err(e) => {
            alert(&format!("Can't open {}: {e}", dir.display()));
            std::process::exit(1);
        }
    };
    let mut screen = Screen::open(dir);
    let window = match &mut screen {
        Screen::App(app) => app.fit_window(&crate::chrome::monitors()),
        Screen::Problem(_) => App::default().fit_window(&[]),
    };
    let screen = std::cell::Cell::new(Some(screen));
    let boot = move || screen.take().expect("iced boots once");
    iced::application(boot, Screen::update, Screen::view)
        .title("Kanvibe")
        .window(window)
        .theme(Screen::theme)
        .subscription(Screen::subscription)
        .exit_on_close_request(false) // the app commits the pending edit on CloseRequested first
        .run()
}

/// A blocking native OK box, for before iced starts.
fn alert(message: &str) {
    rfd::MessageDialog::new()
        .set_title("Kanvibe")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
