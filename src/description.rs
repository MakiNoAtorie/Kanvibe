//! The Card's Markdown description (ADR 0003): rendered view, highlighted editor, Ctrl+B/I/K, links.
use std::sync::Arc;
use std::time::Instant;

use iced::widget::text_editor::{Action, Binding, Edit, KeyPress, Motion, Status};
use iced::widget::{button, container, markdown, row, space, text, text_editor};
use iced::{Center, Element, Font};

use crate::app::{App, Field, Message};
use crate::domain::Card;

/// Widget Id of the description editor.
pub(crate) const EDITOR: &str = "description";

/// Markup the editor shortcuts wrap around the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Markup {
    Bold,
    Italic,
    Link,
}

impl Markup {
    fn markers(self) -> (&'static str, &'static str) {
        match self {
            Markup::Bold => ("**", "**"),
            Markup::Italic => ("*", "*"),
            Markup::Link => ("[", "](url)"),
        }
    }
}

/// Whether a clicked link may open in the browser: http, https and mailto only, so a file or
/// other scheme never launches a program.
pub fn opens_in_browser(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        ["http", "https", "mailto"]
            .iter()
            .any(|s| scheme.eq_ignore_ascii_case(s))
    })
}

/// Opens an allowed link; any other does nothing.
pub(crate) fn open_link(url: &str) {
    if opens_in_browser(url)
        && let Err(e) = webbrowser::open(url)
    {
        eprintln!("Link not opened: {e}");
    }
}

/// The description editor's keys: Ctrl+B / I / K format, everything else as `text_editor` does.
pub fn key_binding(key: KeyPress) -> Option<Binding<Message>> {
    if !matches!(key.status, Status::Focused { .. }) {
        return None;
    }
    if key.modifiers.command() {
        let markup = match key.key.to_latin(key.physical_key) {
            Some('b') => Some(Markup::Bold),
            Some('i') => Some(Markup::Italic),
            Some('k') => Some(Markup::Link),
            _ => None,
        };
        if let Some(markup) = markup {
            return Some(Binding::Custom(Message::Format(markup)));
        }
    }
    Binding::from_key_press(key)
}

/// The Card face's description preview: its first non-blank line, muted and small, clipped
/// to one line.
pub(crate) fn face_preview(card: &Card) -> Option<Element<'_, Message>> {
    let line = card
        .description
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?;
    let line = text(line)
        .size(crate::app::META_SIZE)
        .style(text::secondary)
        .wrapping(text::Wrapping::None);
    Some(container(line).width(iced::Fill).clip(true).into())
}

/// iced's Markdown style with inline code on the weak background in the theme's text colour,
/// instead of a black box (loud in the light theme).
fn markdown_settings(theme: &iced::Theme) -> markdown::Settings {
    let palette = theme.extended_palette();
    markdown::Settings::with_style(markdown::Style {
        inline_code_highlight: markdown::Highlight {
            background: palette.background.weak.color.into(),
            border: iced::Border::default().rounded(4),
        },
        inline_code_color: palette.background.weak.text,
        ..markdown::Style::from_palette(theme.palette())
    })
}

impl App {
    /// Re-parses the open Card's description for the rendered view when it changed.
    /// Runs after every `update`.
    pub(crate) fn refresh_rendered(&mut self) {
        let source = self.open_card().and_then(|c| self.data.description(c));
        let source = source.unwrap_or_default();
        if self.rendered.0 != source {
            self.rendered = (source.to_owned(), markdown::Content::parse(source));
        }
    }

    /// The open Card's description: rendered with an Edit button, or the highlighted editor.
    pub(crate) fn view_description<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let theme = self.theme();
        // The section heading, with Edit or Done at its right: the button swaps in place.
        let header = |button| {
            row![
                crate::app::section_heading("Description"),
                space::horizontal(),
                button
            ]
            .align_y(Center)
        };
        let editing =
            (self.editing.as_ref()).is_some_and(|e| e.field == Field::Description(card.id));
        let edit = match editing {
            true => button(text("Done").size(crate::app::META_SIZE))
                .padding([2, 8])
                .on_press(Message::CommitEdit)
                .into(),
            false => crate::chrome::tip(
                crate::app::small_button("Edit", Message::EditDescription),
                "Edit description (E)",
            ),
        };
        let body: Element<_> = match card.description.is_empty() {
            _ if editing => self.description_editor(),
            true => text("No description")
                .size(13)
                .style(text::secondary)
                .into(),
            false => markdown::view(self.rendered.1.items(), markdown_settings(&theme))
                .map(Message::OpenLink),
        };
        crate::app::section(header(edit), [body])
    }

    /// The highlighted Markdown editor of the description being edited.
    pub(crate) fn description_editor(&self) -> Element<'_, Message> {
        text_editor(&self.editor)
            .id(EDITOR)
            .placeholder("Markdown: **bold**, *italic*, [link](https://…)")
            .on_action(Message::DescriptionAction)
            .key_binding(key_binding)
            .font(Font::MONOSPACE)
            .size(13)
            .min_height(120)
            .highlight("md", self.highlighter_theme())
            .into()
    }

    /// Wraps the selection in `markup`, or inserts the markers with the cursor between them.
    pub(crate) fn format(&mut self, markup: Markup) {
        let (left, right) = markup.markers();
        let inner = self.editor.selection().unwrap_or_default();
        let text = format!("{left}{inner}{right}");
        self.description_action(Action::Edit(Edit::Paste(Arc::new(text))));
        if inner.is_empty() {
            for _ in right.chars() {
                self.description_action(Action::Move(Motion::Left));
            }
        }
    }

    /// Applies an editor action; a text change goes live into the Card, like a rename keystroke.
    pub(crate) fn description_action(&mut self, action: Action) {
        let Some(edit) = &mut self.editing else {
            return;
        };
        let Field::Description(card) = edit.field else {
            return;
        };
        let changes = action.is_edit();
        self.editor.perform(action);
        if changes {
            edit.text = self.editor.text();
            self.data.set_description(card, &edit.text);
            edit.typed_at = Some(Instant::now());
        }
    }
}
