//! "Save as image…": the whole Board, Filter applied strictly, rendered off-screen to a PNG
//! by `iced_test::screenshot` on a second iced program whose view is [`view`]. No sidebar,
//! the open Card as a plain face, no scrolling; current theme, scale 1.0, on a worker thread.
use std::fs;
use std::time::Duration;

use iced::widget::{column, container, row, text};
use iced::{Element, Fill, Padding, Task, Theme};

use crate::app::aged;
use crate::app::{App, HEADING, Message, card_face, card_style, column_style, count_badge};
use crate::domain::Board;
use crate::filter::Filter;

/// Pixels per side: wgpu's default texture limit (rendering past it panics).
pub const LIMIT: u32 = 8192;
const PAD: u32 = 12;
const COLUMN: u32 = 280;
const SPACING: u32 = 12;

fn too_large(width: u32, height: &str) -> String {
    format!(
        "Board too large for one image ({width}×{height} px; limit {LIMIT}). \
         Use the filter or archive old Cards, then try again."
    )
}

/// The image width, from the Column count; the refusal when it is over the cap.
/// The height is unknown before rendering, so it reads "…".
fn width(board: &Board) -> Result<u32, String> {
    let n = board.columns.len().max(1) as u32;
    let width = 2 * PAD + n * COLUMN + (n - 1) * SPACING;
    match width > LIMIT {
        true => Err(too_large(width, "…")),
        false => Ok(width),
    }
}

/// The export program's state: the Board as it was when "Save as image…" was clicked.
#[derive(Clone)]
struct Snapshot {
    board: Board,
    filter: Filter,
}

/// The Board as PNG bytes, or the refusal to show. Blocks for a second or more: run it on a
/// worker thread, never on iced's executor.
pub fn render(board: &Board, filter: &Filter, theme: &Theme) -> Result<Vec<u8>, String> {
    let width = width(board)?;
    let snapshot = Snapshot {
        board: board.clone(),
        filter: filter.clone(),
    };
    let program = iced::application(
        move || snapshot.clone(),
        |_: &mut Snapshot, _: Message| {},
        view,
    );
    // The height can't be known up front: render at the cap and crop the background below.
    let shot = iced_test::screenshot(
        &program,
        theme,
        (width as f32, LIMIT as f32),
        1.0,
        Duration::ZERO,
    );
    let width = shot.size.width;
    let bg = iced::theme::Base::base(theme).background_color.into_rgba8();
    let rows: Vec<&[u8]> = shot.rgba.chunks_exact(width as usize * 4).collect();
    let is_bg = |r: &[u8]| r.as_chunks::<4>().0.iter().all(|p| *p == bg);
    // The view has no bottom padding, so content reaching the last row means it went past it.
    if rows.last().is_some_and(|r| !is_bg(r)) {
        return Err(too_large(width, &format!(">{LIMIT}")));
    }
    let content = rows
        .iter()
        .rposition(|r| !is_bg(r))
        .map_or(0, |i| i as u32 + 1);
    let height = (content + PAD).min(rows.len() as u32);
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let image = &shot.rgba[..(width * height * 4) as usize];
    let written = encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(image).and_then(|()| w.finish()));
    written.map_err(|e| format!("Image not saved: {e}"))?;
    Ok(png)
}

/// The export view: Board name, then every Column with the Cards the Filter matches.
/// Bottom padding is left off (the crop adds it back), so an overflow reaches the last row.
fn view(s: &Snapshot) -> Element<'_, Message> {
    let now = jiff::Timestamp::now();
    let columns = s.board.columns.iter().map(|col| {
        let shown = |c: &_| s.filter.matches(c, &s.board.labels);
        let count = count_badge(s.filter.count_label(col, shown), col.over_limit());
        let header = row![text(&col.name).size(14).font(HEADING).width(Fill), count].spacing(8);
        let cards = col
            .cards
            .iter()
            .filter(|c| s.filter.matches(c, &s.board.labels));
        let cards = cards.map(|c| {
            let stale = c.is_stale(now); // no selection here: stale simply fades
            card_face(Some(&s.board), c, text(&c.title).width(Fill).into())
                .style(move |theme| aged(card_style(theme), stale))
                .into()
        });
        let cards = column(cards).spacing(6);
        container(column![header, cards].spacing(8))
            .width(COLUMN as f32)
            .padding(8)
            .style(column_style)
            .into()
    });
    column![
        text(&s.board.name).size(20).font(HEADING),
        row(columns).spacing(SPACING as f32)
    ]
    .spacing(12)
    .padding(Padding::new(PAD as f32).bottom(0))
    .into()
}

impl App {
    pub(crate) fn update_image(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SaveImage(id) if !self.saving_image => {
                self.commit_pending(); // a Card being typed goes into the image
                let Some(board) = self.data.active_board(id).cloned() else {
                    return Task::none();
                };
                // Refused before the dialog, so no file name is picked for nothing.
                if let Err(refusal) = width(&board) {
                    self.notice = Some(refusal);
                    return Task::none();
                }
                self.saving_image = true;
                let (filter, theme) = (self.filter.clone(), self.theme());
                let stem = crate::transfer::stem(&board.name);
                let pick = rfd::AsyncFileDialog::new()
                    .set_file_name(format!("{stem}.png"))
                    .add_filter("PNG image", &["png"])
                    .save_file();
                return Task::perform(
                    async move {
                        let path = pick.await?.path().to_owned();
                        let (tx, rx) = iced::futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let saved = render(&board, &filter, &theme).and_then(|png| {
                                fs::write(&path, png).map_err(|e| {
                                    format!("Image not saved: {}: {e}", path.display())
                                })
                            });
                            let _ = tx.send(saved);
                        });
                        // A panic in the renderer drops the sender.
                        let failed = || Err("Image not saved: rendering failed.".to_string());
                        Some(rx.await.unwrap_or_else(|_| failed()))
                    },
                    Message::ImageSaved,
                );
            }
            Message::ImageSaved(saved) => {
                self.saving_image = false;
                match saved {
                    Some(Err(e)) => self.notice = Some(e),
                    Some(Ok(())) => self.board_menu = false,
                    None => {}
                }
            }
            _ => {}
        }
        Task::none()
    }
}
