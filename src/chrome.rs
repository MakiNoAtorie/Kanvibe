//! Window chrome (#23): theme, window geometry, the shortcuts overlay and the data-folder opener.
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use iced::theme::{Palette, palette};
use iced::widget::{button, column, row, text, tooltip};
use iced::{Color, Element, Point, Size, Task, Theme, color, window};

use crate::app::{App, Message, dialog};
use crate::storage::Geometry;

/// Every keyboard shortcut, for the F1 overlay. Add a row when `keys::map` learns a key.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("N", "New Card in the selected Column"),
    ("Arrow keys", "Move the selection"),
    ("Ctrl+Arrow keys", "Move the selected Card"),
    ("Home / End", "Select the first / last Card in the Column"),
    (
        "Ctrl+Home / Ctrl+End",
        "Move the selected Card to the top / bottom",
    ),
    ("Ctrl+Tab / Ctrl+Shift+Tab", "Next / previous Board"),
    ("Enter", "Open the selected Card (or click it)"),
    ("F2", "Rename the open or selected Card"),
    (
        "Tab / Shift+Tab",
        "Between a Card's title and its description",
    ),
    ("E", "Edit the open or selected Card's description"),
    (
        "Ctrl+B / Ctrl+I / Ctrl+K",
        "Bold, italic, link in the description",
    ),
    ("Del", "Archive the selected Card"),
    ("L", "Labels of the open or selected Card"),
    ("M", "Move the open or selected Card to another Board"),
    (
        "Ctrl+Up / Ctrl+Down",
        "Move the Checklist Item being edited",
    ),
    ("Ctrl+F", "Filter the Board (Enter selects the first match)"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+Y / Ctrl+Shift+Z", "Redo"),
    ("Esc", "Cancel a drag, close, cancel or deselect"),
    ("F1", "Keyboard shortcuts"),
];

/// Opens at this size, centred, when there is no usable saved rectangle.
const DEFAULT: Size = Size::new(1280.0, 720.0);
const MIN: Size = Size::new(960.0, 600.0);
/// A saved rectangle counts as on a monitor when this much of it is on one.
const VISIBLE: (f32, f32) = (100.0, 50.0);
/// Windows parks a minimised window at -32000,-32000 (physical); anything this far is that.
const MINIMISED_AT: f32 = -10_000.0;

/// The window as last reported. Geometry is committed only from a maximised answer of `false`,
/// so the rectangle a maximise or minimise passes through never lands in `geometry`.
#[derive(Debug, Default)]
pub struct WindowState {
    /// The restored rectangle that is saved with the next save.
    pub geometry: Option<Geometry>,
    pub maximized: bool,
    size: Option<Size>,
    pos: Option<Point>,
    /// The last report was a minimised window (0x0 size or parked position): its maximised
    /// answer is `false` even when it was maximised, so answers are ignored until it is back.
    minimised: bool,
}

impl WindowState {
    pub fn new(geometry: Option<Geometry>, maximized: bool) -> Self {
        let mut state = WindowState {
            maximized,
            ..Self::default()
        };
        state.seed(geometry);
        state
    }

    fn seed(&mut self, geometry: Option<Geometry>) {
        self.geometry = geometry;
        self.size = geometry.map(|g| Size::new(g.width, g.height));
        self.pos = geometry.map(|g| Point::new(g.x, g.y));
    }

    /// The window's size as last reported (or saved); `None` before any report.
    pub fn size(&self) -> Option<Size> {
        self.size
    }

    pub fn resized(&mut self, size: Size) {
        self.minimised = size.width < 1.0 || size.height < 1.0;
        if !self.minimised {
            self.size = Some(size);
        }
    }

    pub fn moved(&mut self, pos: Point) {
        self.minimised = pos.x <= MINIMISED_AT && pos.y <= MINIMISED_AT;
        if !self.minimised {
            self.pos = Some(pos);
        }
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        if self.minimised {
            return;
        }
        self.maximized = maximized;
        if let (false, Some(size), Some(pos)) = (maximized, self.size, self.pos) {
            self.geometry = Some(Geometry {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            });
        }
    }
}

impl Geometry {
    /// Enough of this rectangle is on a monitor to grab it.
    fn on_screen(&self, monitors: &[Geometry]) -> bool {
        monitors.iter().any(|m| {
            let w = (self.x + self.width).min(m.x + m.width) - self.x.max(m.x);
            let h = (self.y + self.height).min(m.y + m.height) - self.y.max(m.y);
            w >= VISIBLE.0 && h >= VISIBLE.1
        })
    }
}

/// A monitor as `display-info` reports it (physical pixels) in logical pixels.
pub fn logical(x: i32, y: i32, width: u32, height: u32, scale: f32) -> Geometry {
    Geometry {
        x: x as f32 / scale,
        y: y as f32 / scale,
        width: width as f32 / scale,
        height: height as f32 / scale,
    }
}

/// The light theme: paper greys on a warm `#f7f7f5` page. Surfaces read from the background
/// tiers in both themes: `weakest` is the raised surface (Columns, buttons),
/// which is white here; iced's dark theme already lifts it toward the text. The other tiers
/// come from a neutral page: iced's darkening multiplies a warm page's chroma into olive.
static LIGHT: LazyLock<Theme> = LazyLock::new(|| {
    let paper = Palette {
        background: color!(0xf7f7f5),
        text: color!(0x222222),
        ..Palette::LIGHT
    };
    Theme::custom_with_fn("Kanvibe Light", paper, |p| {
        let neutral = Palette {
            background: color!(0xf7f7f7),
            ..p
        };
        let mut extended = palette::Extended::generate(neutral);
        extended.background.base = palette::Pair::new(p.background, p.text);
        extended.background.weakest = palette::Pair::new(Color::WHITE, p.text);
        extended
    })
});

/// The connected monitors; empty if the system won't say (then nothing is dropped as off-screen).
pub fn monitors() -> Vec<Geometry> {
    display_info::DisplayInfo::all()
        .unwrap_or_default()
        .iter()
        .map(|d| logical(d.x, d.y, d.width, d.height, d.scale_factor.max(1.0)))
        .collect()
}

impl App {
    /// The light or dark iced theme.
    pub fn theme(&self) -> Theme {
        if self.dark {
            Theme::Dark
        } else {
            LIGHT.clone()
        }
    }

    /// The description highlighter's theme, following the app theme.
    pub fn highlighter_theme(&self) -> iced::highlighter::Theme {
        if self.dark {
            iced::highlighter::Theme::Base16Ocean
        } else {
            iced::highlighter::Theme::InspiredGitHub
        }
    }

    /// The window settings for launch. A saved rectangle on no monitor is dropped for good and
    /// the window opens centred at 1280x720. No monitors known keeps the rectangle.
    pub fn fit_window(&mut self, monitors: &[Geometry]) -> window::Settings {
        let saved = self.window.geometry;
        let kept = saved.filter(|g| monitors.is_empty() || g.on_screen(monitors));
        if kept.is_none() {
            self.window.seed(None);
        }
        let (size, position) = match kept {
            Some(g) => (
                Size::new(g.width, g.height),
                window::Position::Specific(Point::new(g.x, g.y)),
            ),
            None => (DEFAULT, window::Position::Centered),
        };
        window::Settings {
            size,
            position,
            maximized: self.window.maximized,
            min_size: Some(MIN),
            ..window::Settings::default()
        }
    }

    pub(crate) fn view_shortcuts(&self) -> Element<'_, Message> {
        let rows = SHORTCUTS.iter().map(|(keys, what)| {
            row![
                text(*keys).width(220).font(iced::Font::MONOSPACE),
                text(*what)
            ]
            .spacing(12)
            .into()
        });
        // The same ✕ (Esc) header as the other dialogs; a fixed width holds the right-aligned ✕.
        dialog(
            column![crate::app::dialog_header(
                "Keyboard shortcuts",
                Message::Escape
            )]
            .extend(rows)
            .spacing(8)
            .width(600),
        )
    }
}

/// Asked after every Resized/Moved report; the `Maximized` answer decides whether the report
/// was a restored rectangle.
pub fn query_maximized() -> Task<Message> {
    window::oldest()
        .and_then(window::is_maximized)
        .map(Message::Maximized)
}

/// A tooltip above-or-below `content`; used to name a button's shortcut.
pub fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    label: impl text::IntoFragment<'a>,
) -> Element<'a, Message> {
    tooltip(
        content,
        iced::widget::container(text(label).size(12))
            .padding(4)
            // Bordered, so a tooltip over a gray Column still reads as its own box.
            .style(iced::widget::container::bordered_box),
        tooltip::Position::Top,
    )
    .into()
}

/// The sidebar footer's last three buttons: theme, Keyboard shortcuts, Open data folder.
pub fn footer(app: &App) -> Element<'_, Message> {
    let (theme, switch) = if app.dark {
        ("Light theme", "Switch to the light theme")
    } else {
        ("Dark theme", "Switch to the dark theme")
    };
    column![
        tip(link(theme, Message::ToggleTheme), switch),
        tip(
            link("Keyboard shortcuts", Message::ShowShortcuts),
            "Keyboard shortcuts (F1)"
        ),
        tip(
            link("Open data folder", Message::OpenDataFolder),
            "Show the data folder in the file manager"
        ),
    ]
    .into()
}

/// A sidebar footer link: 13px, muted until hovered.
pub fn link(label: &str, message: Message) -> button::Button<'_, Message> {
    button(text(label).size(13))
        .style(crate::app::faded(0.7))
        .on_press(message)
}

/// The file manager command that shows `dir`.
pub fn open_command(dir: &Path) -> Command {
    let mut command = Command::new(if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    });
    command.arg(dir);
    command
}

/// Fire and forget; explorer exits with 1 even on success, so the result is not checked.
pub fn open_folder(dir: &Path) {
    let _ = open_command(dir).spawn();
}
