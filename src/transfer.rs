//! Export and import of one Board as a `.kanvibe.json` file.
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use iced::widget::{button, column, text};
use iced::{Element, Task};
use serde::{Deserialize, Serialize};

use crate::app::{App, Message, dialog};
use crate::domain::{Board, Data, Id};

const FORMAT: &str = "kanvibe-board";
use crate::storage::VERSION;

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    version: u32,
    board: &'a Board,
}

/// Writes `{ "format": "kanvibe-board", "version": 2, "board": ... }`, pretty-printed.
/// The Board carries its Labels, Columns, Cards, Checklists and archived Cards; no settings.
pub fn export(path: &Path, board: &Board) -> io::Result<()> {
    let file = FileOut {
        format: FORMAT,
        version: VERSION,
        board,
    };
    fs::write(path, serde_json::to_vec_pretty(&file)?)
}

/// The Board in an export file, decided by content; `Err` is the refusal to show.
fn read(bytes: &[u8]) -> Result<Board, String> {
    #[derive(Deserialize)]
    struct Head {
        boards: Option<serde::de::IgnoredAny>,
        format: Option<String>,
        version: Option<u32>,
    }
    #[derive(Deserialize)]
    struct FileIn {
        board: Board,
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let not_export = || "This file is not a Kanvibe Board export.".to_string();
    let head: Head = serde_json::from_slice(bytes).map_err(|_| not_export())?;
    if head.boards.is_some() {
        return Err("This is a Kanvibe data file, not a Board export.".into());
    }
    if head.format.as_deref() != Some(FORMAT) {
        return Err(not_export());
    }
    if head.version.is_some_and(|v| v > VERSION) {
        return Err(
            "This Board was exported by a newer version of Kanvibe. Update Kanvibe to import it."
                .into(),
        );
    }
    let file: FileIn = serde_json::from_slice(bytes)
        .map_err(|e| format!("This Board export can't be read: {e}"))?;
    Ok(file.board)
}

impl Data {
    /// Adds an imported Board, active, at the bottom of the sidebar, with every ID reassigned.
    /// Card Label references and archived Cards' `column_id` go through the same old-to-new
    /// map, after the load repairs (`repair::repair`). Timestamps are kept.
    pub fn import_board(&mut self, board: Board) -> Id {
        let boards = vec![board];
        let (repaired, _) = crate::repair::repair(Data { next_id: 0, boards });
        let mut board = repaired
            .boards
            .into_iter()
            .next()
            .expect("one Board in, one out");
        let mut map: HashMap<Id, Id> = HashMap::new();
        let mut fresh = |old: &mut Id| {
            let id = self.new_id();
            map.entry(*old).or_insert(id);
            *old = id;
        };
        fresh(&mut board.id);
        board.labels.iter_mut().for_each(|l| fresh(&mut l.id));
        board.columns.iter_mut().for_each(|c| fresh(&mut c.id));
        let active = board.columns.iter_mut().flat_map(|c| &mut c.cards);
        let archived = board.archived_cards.iter_mut().map(|a| &mut a.card);
        for card in active.chain(archived) {
            fresh(&mut card.id);
            card.checklist.iter_mut().for_each(|i| fresh(&mut i.id));
        }
        let active = board.columns.iter_mut().flat_map(|c| &mut c.cards);
        let archived = board.archived_cards.iter_mut().map(|a| &mut a.card);
        for card in active.chain(archived) {
            // The repair left only the Board's own Labels, so every one is in the map.
            card.labels = card
                .labels
                .iter()
                .filter_map(|l| map.get(l).copied())
                .collect();
        }
        for archived in &mut board.archived_cards {
            // A removed Column gets an ID no Column has, shared by all its archived Cards.
            let column = map
                .entry(archived.column_id)
                .or_insert_with(|| self.new_id());
            archived.column_id = *column;
        }
        board.archived = false;
        board.name = self.free_name(&board.name);
        let id = board.id;
        self.boards.push(board);
        id
    }

    /// `name`, or on a clash (ignoring case and outer spaces, against every Board including
    /// archived ones) its base, without any trailing " (n)", plus the first free " (2)", " (3)", ….
    fn free_name(&self, name: &str) -> String {
        let taken = |n: &str| {
            let n = n.to_lowercase();
            self.boards
                .iter()
                .any(|b| b.name.trim().to_lowercase() == n)
        };
        let name = name.trim();
        if !taken(name) {
            return name.into();
        }
        let number = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
        let base = match name.strip_suffix(')').and_then(|n| n.rsplit_once(" (")) {
            Some((base, n)) if number(n) => base.trim_end(),
            _ => name,
        };
        let mut names = (2..).map(|n| format!("{base} ({n})"));
        names.find(|n| !taken(n)).expect("some number is free")
    }
}

impl App {
    pub(crate) fn update_transfer(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ExportBoard(id) => {
                self.commit_pending(); // a Card being typed goes into the export
                self.board_menu = false;
                // Archived Boards can't be exported: restore first.
                let Some(board) = self.data.active_board(id).cloned() else {
                    return Task::none();
                };
                let pick = rfd::AsyncFileDialog::new()
                    .set_file_name(file_name(&board.name))
                    .add_filter("Kanvibe Board", &["json"])
                    .save_file();
                return Task::perform(
                    async move {
                        let path = pick.await?.path().to_owned();
                        let written = export(&path, &board);
                        Some(written.map_err(|e| format!("Not exported: {}: {e}", path.display())))
                    },
                    Message::Exported,
                );
            }
            Message::ImportBoard => {
                let pick = rfd::AsyncFileDialog::new()
                    .add_filter("Kanvibe Board", &["json"])
                    .pick_file();
                return Task::perform(
                    async move {
                        let path = pick.await?.path().to_owned();
                        let bytes = fs::read(&path);
                        Some(bytes.map_err(|e| format!("Can't open {}: {e}", path.display())))
                    },
                    Message::Imported,
                );
            }
            Message::Exported(Some(Err(e))) | Message::Imported(Some(Err(e))) => {
                self.notice = Some(e)
            }
            Message::Imported(Some(Ok(bytes))) => match read(&bytes) {
                Ok(board) => {
                    self.commit_pending();
                    self.drag = None;
                    let id = self.data.import_board(board);
                    self.current_board = Some(id);
                    self.record_step(None);
                }
                Err(refusal) => self.notice = Some(refusal),
            },
            _ => {}
        }
        Task::none()
    }

    /// The open notice over the dimmed app; OK (or Esc) closes it.
    pub(crate) fn view_notice(&self) -> Option<Element<'_, Message>> {
        let notice = self.notice.as_deref()?;
        let ok = button("OK").on_press(Message::Escape);
        Some(dialog(column![text(notice), ok].spacing(12).max_width(480)))
    }
}

/// The suggested export file name: [`stem`] plus `.kanvibe.json`.
pub fn file_name(board_name: &str) -> String {
    format!("{}.kanvibe.json", stem(board_name))
}

/// The Board name made safe as a Windows file name stem; "Board" when nothing is left.
pub fn stem(board_name: &str) -> String {
    let trim = |s: &str| s.trim_end_matches(['.', ' ']).to_string();
    let safe: String = board_name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let safe = trim(&trim(&safe).chars().take(100).collect::<String>());
    // Windows reserves these device names, with any extension.
    let stem = safe.split('.').next().unwrap_or_default().to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    match safe.as_str() {
        "" => "Board".into(),
        _ if reserved => format!("_{safe}"),
        _ => safe,
    }
}
