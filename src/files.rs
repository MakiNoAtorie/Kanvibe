//! Files on a Card: paths to files on this machine, chosen with the native Open dialog and
//! opened with their default program. Only the path is stored; nothing is copied.
use std::path::Path;

use iced::widget::{button, row, text};
use iced::{Center, Element, Task};

use crate::app::{App, Message};
use crate::chrome::tip;
use crate::domain::{Card, Data, Id};

impl Data {
    /// Adds a path to the Card's files and touches it; `None` when the Card is unknown or
    /// already holds that path.
    pub fn add_file(&mut self, card: Id, path: &str) -> Option<()> {
        let files = &mut self.card_mut(card)?.files;
        if files.iter().any(|f| f == path) {
            return None;
        }
        files.push(path.into());
        self.touch(card);
        Some(())
    }

    /// Removes the Card's file at `index` and touches it; `None` when there is none.
    pub fn remove_file(&mut self, card: Id, index: usize) -> Option<()> {
        let files = &mut self.card_mut(card)?.files;
        (index < files.len()).then(|| files.remove(index))?;
        self.touch(card);
        Some(())
    }
}

/// The face marker: "1 file", "n files"; `None` without files.
pub(crate) fn marker<'a>(card: &Card) -> Option<Element<'a, Message>> {
    let label = match card.files.len() {
        0 => return None,
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    };
    Some(
        text(label)
            .size(crate::app::META_SIZE)
            .style(text::secondary)
            .into(),
    )
}

impl App {
    pub(crate) fn update_files(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddFile(card) => {
                self.commit_pending();
                let pick = rfd::AsyncFileDialog::new().pick_file();
                return Task::perform(
                    async move { pick.await.map(|f| f.path().to_owned()) },
                    move |path| Message::FileChosen(card, path),
                );
            }
            Message::FileChosen(card, Some(path)) => {
                self.commit_pending();
                if self.data.add_file(card, &path.to_string_lossy()).is_some() {
                    self.record_step(self.data.board_of_card(card));
                }
            }
            Message::RemoveFile(card, index) => {
                self.commit_pending();
                if self.data.remove_file(card, index).is_some() {
                    self.record_step(self.data.board_of_card(card));
                }
            }
            Message::OpenFile(path) => crate::chrome::open_path(Path::new(&path)),
            _ => {}
        }
        Task::none()
    }

    /// The Files section of the open Card: one row per file, then "+ Add file…".
    pub(crate) fn view_files<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let rows = card.files.iter().enumerate().map(|(i, path)| {
            let name = Path::new(path)
                .file_name()
                .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
            // Checked at view time: a moved or renamed file shows as not found.
            let entry = match Path::new(path).exists() {
                true => button(text(name).size(13))
                    .style(button::text)
                    .on_press(Message::OpenFile(path.clone())),
                false => button(
                    text(format!("{name} (not found)"))
                        .size(13)
                        .style(text::secondary),
                )
                .style(button::text),
            };
            row![
                tip(entry, path.as_str()),
                tip(
                    button(text("✕").size(crate::app::META_SIZE).style(text::secondary))
                        .style(button::text)
                        .on_press(Message::RemoveFile(card.id, i)),
                    "Remove file"
                ),
            ]
            .spacing(6)
            .align_y(Center)
            .into()
        });
        let add = crate::app::small_button("+ Add file…", Message::AddFile(card.id));
        let body = rows.chain([add.into()]);
        crate::app::section(crate::app::section_heading("Files"), body)
    }
}
