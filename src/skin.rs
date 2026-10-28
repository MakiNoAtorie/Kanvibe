//! The themes: Light and Dark (three flat layers), and four with their own surfaces. A style
//! function asks [`Skin::of`] which theme it is drawing in; [`Skin::Light`] and [`Skin::Dark`]
//! get `None` from the overrides here and keep the plain styles in app.rs.
use std::sync::LazyLock;

use iced::theme::{Palette, palette};
use iced::widget::container;
use iced::{Background, Border, Color, Shadow, Theme, Vector, color, gradient};

/// A theme as saved: `"theme": "<key>"` in the settings. Shown by its [`label`](Skin::label).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Skin {
    #[default]
    Light,
    Dark,
    /// A purple-to-coral gradient, frosted Columns, dark glass Cards.
    Aurora,
    /// Cream paper, black 2px edges, hard offset shadows, square corners.
    Paper,
    /// A pastel per Column, a stronger band behind its header.
    Tinted,
    /// Navy with cyan accents; the selected Card glows.
    Midnight,
}

impl Skin {
    /// The sidebar's theme list, in order.
    pub const ALL: [Skin; 6] = [
        Skin::Light,
        Skin::Dark,
        Skin::Aurora,
        Skin::Paper,
        Skin::Tinted,
        Skin::Midnight,
    ];

    /// The settings key; "light" and "dark" are the keys v1 wrote.
    pub fn key(self) -> &'static str {
        match self {
            Skin::Light => "light",
            Skin::Dark => "dark",
            Skin::Aurora => "aurora",
            Skin::Paper => "paper",
            Skin::Tinted => "tinted",
            Skin::Midnight => "midnight",
        }
    }

    /// An unknown key (a newer version's theme) is Light.
    pub fn from_key(key: &str) -> Skin {
        Skin::ALL
            .into_iter()
            .find(|s| s.key() == key)
            .unwrap_or_default()
    }

    /// The name shown in the sidebar.
    pub fn label(self) -> &'static str {
        match self {
            Skin::Light => "Light",
            Skin::Dark => "Dark",
            Skin::Aurora => "Aurora",
            Skin::Paper => "Paper",
            Skin::Tinted => "Tinted",
            Skin::Midnight => "Midnight",
        }
    }

    pub fn theme(self) -> Theme {
        let theme: &LazyLock<Theme> = match self {
            Skin::Light => &LIGHT,
            Skin::Dark => &DARK,
            Skin::Aurora => &AURORA,
            Skin::Paper => &PAPER,
            Skin::Tinted => &TINTED,
            Skin::Midnight => &MIDNIGHT,
        };
        Theme::clone(theme)
    }

    /// The skin `theme` was made from (by its name); Light for any other theme.
    pub fn of(theme: &Theme) -> Skin {
        let name = iced::theme::Base::name(theme);
        Skin::ALL
            .into_iter()
            .find(|s| name.strip_prefix("Kanvibe ") == Some(s.label()))
            .unwrap_or_default()
    }
}

impl std::fmt::Display for Skin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The three layers every theme chooses rather than derives, stored in the background tiers
/// every style reads: `base` is the page, `weaker` the Column, `weakest` the Card (and raised
/// buttons), `weak` the hairline border.
fn layers(
    mut extended: palette::Extended,
    p: Palette,
    column: Color,
    card: Color,
    border: Color,
) -> palette::Extended {
    extended.background.base = palette::Pair::new(p.background, p.text);
    extended.background.weaker = palette::Pair::new(column, p.text);
    extended.background.weakest = palette::Pair::new(card, p.text);
    extended.background.weak = palette::Pair::new(border, p.text);
    extended
}

/// A theme from its palette and its three layers.
fn themed(name: &'static str, p: Palette, column: Color, card: Color, border: Color) -> Theme {
    Theme::custom_with_fn(name, p, move |p| {
        layers(palette::Extended::generate(p), p, column, card, border)
    })
}

/// The light theme: white Cards on a grey Column on a warm `#f7f7f5` page. The other tiers
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
        let extended = palette::Extended::generate(neutral);
        layers(
            extended,
            p,
            color!(0xecece9),
            Color::WHITE,
            color!(0xdcdcd8),
        )
    })
});

/// The dark theme: the same three steps, each lighter than the one below, with iced's dark
/// primary and danger.
static DARK: LazyLock<Theme> = LazyLock::new(|| {
    let night = Palette {
        background: color!(0x1b1b1d),
        text: color!(0xe6e6e6),
        ..Palette::DARK
    };
    themed(
        "Kanvibe Dark",
        night,
        color!(0x242428),
        color!(0x2e2e33),
        color!(0x3a3a40),
    )
});

/// Dark glass over a purple-to-coral gradient.
static AURORA: LazyLock<Theme> = LazyLock::new(|| {
    let p = Palette {
        background: color!(0x24123f),
        text: color!(0xf7f2ff),
        primary: color!(0xff7ac6),
        ..Palette::DARK
    };
    themed(
        "Kanvibe Aurora",
        p,
        color!(0x3b2466),
        color!(0x452c75),
        color!(0x5b3d8f),
    )
});

static PAPER: LazyLock<Theme> = LazyLock::new(|| {
    let p = Palette {
        background: color!(0xf3ecdc),
        text: color!(0x111111),
        primary: color!(0xff4f1f),
        warning: color!(0xffc700),
        ..Palette::LIGHT
    };
    // Secondary text (Column names, hints) in ink brown: iced's grey is faint on the yellow bar.
    Theme::custom_with_fn("Kanvibe Paper", p, |p| {
        let mut extended = layers(
            palette::Extended::generate(p),
            p,
            color!(0xe7dcc4),
            color!(0xfffdf6),
            color!(0x111111),
        );
        extended.secondary.base.color = color!(0x4a4032);
        extended
    })
});

/// Paper's sidebar and selected Column header.
const PAPER_YELLOW: Color = color!(0xffd84d);

static TINTED: LazyLock<Theme> = LazyLock::new(|| {
    let p = Palette {
        background: color!(0xf6f7fb),
        text: color!(0x1d2433),
        primary: color!(0x5b6cff),
        ..Palette::LIGHT
    };
    themed(
        "Kanvibe Tinted",
        p,
        color!(0xeceff7),
        Color::WHITE,
        color!(0xdde1ec),
    )
});

static MIDNIGHT: LazyLock<Theme> = LazyLock::new(|| {
    let p = Palette {
        background: color!(0x0a0d1a),
        text: color!(0xdfe6ff),
        primary: color!(0x22d3ee),
        success: color!(0x34f5a4),
        warning: color!(0xfacc15),
        danger: color!(0xff4d8d),
    };
    themed(
        "Kanvibe Midnight",
        p,
        color!(0x121730),
        color!(0x1a2042),
        color!(0x2a3366),
    )
});

/// Behind the whole app: a gradient for Aurora and Midnight, else the plain page.
pub fn page(theme: &Theme) -> container::Style {
    let g = match Skin::of(theme) {
        Skin::Aurora => gradient::Linear::new(2.4)
            .add_stop(0.0, color!(0x2b1055))
            .add_stop(0.55, color!(0x7a2a7f))
            .add_stop(1.0, color!(0xe0736b)),
        Skin::Midnight => gradient::Linear::new(2.8)
            .add_stop(0.0, color!(0x0a0d1a))
            .add_stop(1.0, color!(0x1a0f33)),
        _ => return container::Style::default(),
    };
    container::Style {
        background: Some(Background::Gradient(g.into())),
        ..container::Style::default()
    }
}

/// A Column's pastel hue, from its Id: the golden ratio spreads neighbours apart.
fn hue(id: u64) -> f32 {
    (id as f32 * 0.618_034).fract() * 360.0
}

fn hsl(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Color::from_rgb(r + m, g + m, b + m)
}

/// A Column's tray (expanded or a strip); `id` picks Tinted's pastel.
pub fn column(theme: &Theme, id: u64) -> Option<container::Style> {
    let text_color = Some(theme.palette().text);
    let tray = |background: Color, border: Border| container::Style {
        background: Some(background.into()),
        text_color,
        border,
        ..container::Style::default()
    };
    Some(match Skin::of(theme) {
        Skin::Aurora => tray(
            Color::WHITE.scale_alpha(0.10),
            Border::default()
                .color(Color::WHITE.scale_alpha(0.22))
                .width(1)
                .rounded(18),
        ),
        Skin::Paper => tray(
            color!(0xe7dcc4),
            Border::default().color(color!(0x111111)).width(2),
        ),
        Skin::Tinted => tray(hsl(hue(id), 0.65, 0.93), Border::default().rounded(16)),
        Skin::Midnight => tray(
            color!(0x121730).scale_alpha(0.75),
            Border::default()
                .color(color!(0x232a52))
                .width(1)
                .rounded(12),
        ),
        Skin::Light | Skin::Dark => return None,
    })
}

/// A Column header: Tinted's band, Paper's yellow bar when selected; `None` keeps the tint.
pub fn header(theme: &Theme, id: u64, selected: bool) -> Option<container::Style> {
    let band = |c: Color| container::Style {
        background: Some(c.into()),
        border: Border::default().rounded(10),
        ..container::Style::default()
    };
    match Skin::of(theme) {
        Skin::Tinted => Some(band(hsl(hue(id), 0.6, if selected { 0.78 } else { 0.85 }))),
        Skin::Paper if selected => Some(container::Style {
            border: Border::default().color(color!(0x111111)).width(2),
            ..band(PAPER_YELLOW)
        }),
        Skin::Paper => Some(container::Style::default()),
        _ => None,
    }
}

pub fn card(theme: &Theme) -> Option<container::Style> {
    let face = |background: Color, text: Color, border: Border, shadow: Shadow| container::Style {
        background: Some(background.into()),
        text_color: Some(text),
        border,
        shadow,
        ..container::Style::default()
    };
    let soft = |alpha: f32, y: f32, blur: f32| Shadow {
        color: Color::BLACK.scale_alpha(alpha),
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    };
    let text = theme.palette().text;
    Some(match Skin::of(theme) {
        Skin::Aurora => face(
            color!(0x1f0f3a).scale_alpha(0.82),
            text,
            Border::default()
                .color(Color::WHITE.scale_alpha(0.14))
                .width(1)
                .rounded(12),
            soft(0.35, 6.0, 18.0),
        ),
        Skin::Paper => face(
            color!(0xfffdf6),
            text,
            Border::default().color(text).width(2),
            Shadow {
                color: text,
                offset: Vector::new(4.0, 4.0),
                blur_radius: 0.0,
            },
        ),
        Skin::Tinted => face(
            Color::WHITE,
            text,
            Border::default().rounded(10),
            soft(0.08, 2.0, 6.0),
        ),
        Skin::Midnight => face(
            color!(0x181e3d),
            text,
            Border::default()
                .color(color!(0x2c3670))
                .width(1)
                .rounded(10),
            soft(0.5, 4.0, 12.0),
        ),
        Skin::Light | Skin::Dark => return None,
    })
}

/// The selected Card: Paper's shadow turns the accent, Midnight glows; `None` keeps the
/// plain outline.
pub fn selected(theme: &Theme, style: container::Style) -> Option<container::Style> {
    let primary = theme.palette().primary;
    match Skin::of(theme) {
        Skin::Paper => Some(container::Style {
            shadow: Shadow {
                color: primary,
                ..style.shadow
            },
            border: style.border.width(3),
            ..style
        }),
        Skin::Midnight => Some(container::Style {
            shadow: Shadow {
                color: primary.scale_alpha(0.55),
                offset: Vector::ZERO,
                blur_radius: 16.0,
            },
            border: style.border.color(primary).width(1.5),
            ..style
        }),
        _ => None,
    }
}

pub fn sidebar(theme: &Theme) -> Option<container::Style> {
    let flat = |background: Color, edge: Color, width: f32| container::Style {
        background: Some(background.into()),
        border: Border::default().color(edge).width(width),
        ..container::Style::default()
    };
    Some(match Skin::of(theme) {
        Skin::Aurora => flat(color!(0x12062a).scale_alpha(0.35), Color::TRANSPARENT, 0.0),
        Skin::Paper => flat(PAPER_YELLOW, color!(0x111111), 2.0),
        Skin::Tinted => flat(Color::WHITE, color!(0xe4e7f0), 1.0),
        Skin::Midnight => flat(color!(0x070914).scale_alpha(0.8), color!(0x1d2342), 1.0),
        Skin::Light | Skin::Dark => return None,
    })
}

/// Badge corners: square on Paper.
pub fn pill_radius(theme: &Theme) -> f32 {
    match Skin::of(theme) {
        Skin::Paper => 0.0,
        _ => 10.0,
    }
}
