//! Labels: the palette, Label operations on `Data`, the Label picker overlay and Card chips.
use iced::widget::{
    button, checkbox, column, container, operation, row, scrollable, space, text, text_input,
};
use iced::{Border, Center, Color, Element, Task};

use crate::app::{App, Confirm, EDIT_INPUT, Field, Message, dialog, dialog_header};
use crate::domain::{Board, Card, Data, Id, Label, cards, now};

/// The Label palette: stable keys (what files store) and colours that read in light and dark
/// themes (white text on a mid-dark fill). Reorder freely; files hold keys, not indexes.
pub const PALETTE: [(&str, Color); 10] = [
    ("gray", rgb(0x4B, 0x55, 0x63)),
    ("red", rgb(0xDC, 0x26, 0x26)),
    ("orange", rgb(0xC2, 0x41, 0x0C)),
    ("amber", rgb(0xB4, 0x53, 0x09)),
    ("green", rgb(0x15, 0x80, 0x3D)),
    ("teal", rgb(0x0F, 0x76, 0x6E)),
    ("blue", rgb(0x1D, 0x4E, 0xD8)),
    ("indigo", rgb(0x43, 0x38, 0xCA)),
    ("purple", rgb(0x7E, 0x22, 0xCE)),
    ("pink", rgb(0xBE, 0x18, 0x5D)),
];

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// The colour of a palette key; an unknown key shows as the first palette colour.
pub fn swatch(key: &str) -> Color {
    PALETTE
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(PALETTE[0].1, |(_, c)| *c)
}

/// A chip's text: the Label name; past 12 characters, its first 11 and "…".
pub fn chip_text(name: &str) -> String {
    match name.chars().nth(12) {
        Some(_) => name.chars().take(11).chain(['…']).collect(),
        None => name.into(),
    }
}

/// "Delete Label "X"? It is on N Cards." for a Label on at least one Card; `None` otherwise.
pub fn delete_prompt(data: &Data, label: Id) -> Option<String> {
    let name = &data.label(label)?.name;
    let n = data.label_use(label);
    (n > 0).then(|| format!("Delete Label \"{name}\"? It is on {}.", cards(n)))
}

impl Data {
    pub fn label(&self, id: Id) -> Option<&Label> {
        self.boards
            .iter()
            .flat_map(|b| &b.labels)
            .find(|l| l.id == id)
    }

    /// The Board that owns this Label.
    pub fn board_of_label(&self, label: Id) -> Option<Id> {
        let board = self
            .boards
            .iter()
            .find(|b| b.labels.iter().any(|l| l.id == label))?;
        Some(board.id)
    }

    /// Whether `name` (trimmed, case-insensitive) is taken by another Label of the Board.
    pub fn label_clash(&self, board: Id, name: &str, except: Option<Id>) -> bool {
        let name = name.trim().to_lowercase();
        self.boards.iter().filter(|b| b.id == board).any(|b| {
            let others = b.labels.iter().filter(|l| Some(l.id) != except);
            others
                .into_iter()
                .any(|l| l.name.trim().to_lowercase() == name)
        })
    }

    /// `id` is a Label and `name` would clash with another Label on its Board.
    pub(crate) fn is_label_clash(&self, id: Id, name: &str) -> bool {
        self.board_of_label(id)
            .is_some_and(|board| self.label_clash(board, name, Some(id)))
    }

    /// Appends a Label with a palette colour. A blank or clashing name, an unknown colour key
    /// or an unknown Board creates nothing.
    pub fn add_label(&mut self, board: Id, name: &str, color: &str) -> Option<Id> {
        let name = name.trim();
        if name.is_empty()
            || self.label_clash(board, name, None)
            || !PALETTE.iter().any(|(k, _)| *k == color)
        {
            return None;
        }
        self.boards.iter().position(|b| b.id == board)?;
        let id = self.new_id();
        let board = self.boards.iter_mut().find(|b| b.id == board)?;
        board.labels.push(Label {
            id,
            name: name.into(),
            color: color.into(),
        });
        Some(id)
    }

    /// Recolours a Label with a palette key. An unknown key or Label changes nothing.
    pub fn set_label_color(&mut self, label: Id, color: &str) -> Option<()> {
        PALETTE.iter().find(|(k, _)| *k == color)?;
        let labels = self.boards.iter_mut().flat_map(|b| &mut b.labels);
        labels.into_iter().find(|l| l.id == label)?.color = color.into();
        Some(())
    }

    /// The Card's Labels in its Board's Label order.
    pub fn card_labels(&self, card: Id) -> Vec<&Label> {
        let Some((b, _)) = self.holding(card) else {
            return Vec::new();
        };
        board_labels(&self.boards[b], self.card(card))
    }

    /// Attaches the Label to the Card, or detaches it when attached, and touches the Card.
    /// A Label of another Board, or an unknown Card, changes nothing.
    pub fn toggle_label(&mut self, card: Id, label: Id) -> Option<()> {
        let (b, _) = self.holding(card)?;
        let board = &mut self.boards[b];
        board.labels.iter().find(|l| l.id == label)?;
        let columns = board.columns.iter_mut();
        let card = columns.flat_map(|c| &mut c.cards).find(|c| c.id == card)?;
        match card.labels.iter().position(|l| *l == label) {
            Some(at) => card.labels.remove(at),
            None => {
                card.labels.push(label);
                label
            }
        };
        card.updated = now();
        Some(())
    }

    /// How many Cards carry the Label, archived Cards included.
    pub fn label_use(&self, label: Id) -> usize {
        let active = self
            .boards
            .iter()
            .flat_map(|b| &b.columns)
            .flat_map(|c| &c.cards);
        let archived = self
            .boards
            .iter()
            .flat_map(|b| &b.archived_cards)
            .map(|a| &a.card);
        let cards = active.chain(archived);
        cards.filter(|c| c.labels.contains(&label)).count()
    }

    /// Deletes the Label and strips it from every Card, touching those Cards.
    pub fn delete_label(&mut self, label: Id) -> Option<()> {
        let board = self
            .boards
            .iter_mut()
            .find(|b| b.labels.iter().any(|l| l.id == label))?;
        board.labels.retain(|l| l.id != label);
        let now = now();
        let active = board.columns.iter_mut().flat_map(|c| &mut c.cards);
        let archived = board.archived_cards.iter_mut().map(|a| &mut a.card);
        for card in active.chain(archived) {
            if card.labels.contains(&label) {
                card.labels.retain(|l| *l != label);
                card.updated = now.clone();
            }
        }
        Some(())
    }
}

/// `card`'s Labels on `board`, in Board order; Label IDs the Board doesn't have are skipped.
fn board_labels<'a>(board: &'a Board, card: Option<&Card>) -> Vec<&'a Label> {
    let Some(card) = card else { return Vec::new() };
    board
        .labels
        .iter()
        .filter(|l| card.labels.contains(&l.id))
        .collect()
}

pub const NEW_INPUT: &str = "new-label";

/// The Label picker for the open Card. `None` in `App::label_picker` = closed.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelPicker {
    pub card: Id,
    /// The Label whose editor (pencil) is open.
    pub expanded: Option<Id>,
    /// The "+ New Label" form.
    pub new: Option<NewLabel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewLabel {
    pub text: String,
    pub color: String,
}

impl App {
    pub(crate) fn update_labels(&mut self, message: Message) -> Task<Message> {
        self.commit_pending();
        let card = self.label_picker.as_ref().map(|p| p.card);
        match message {
            Message::OpenLabels => {
                if let Some(card) = self.open_selected() {
                    self.label_picker = Some(LabelPicker {
                        card,
                        expanded: None,
                        new: None,
                    });
                }
            }
            Message::CloseLabels => self.label_picker = None,
            Message::ToggleLabel(label) => {
                if let Some(card) = card
                    && self.data.toggle_label(card, label).is_some()
                {
                    self.record_step(self.data.board_of_card(card));
                }
            }
            Message::EditLabel(label) => {
                if let Some(picker) = &mut self.label_picker {
                    picker.new = None;
                    if picker.expanded.take() != Some(label) {
                        picker.expanded = Some(label);
                        return self.update(Message::StartEdit(Field::LabelName(label)));
                    }
                }
            }
            Message::SetLabelColor(label, color) => {
                if self.data.set_label_color(label, &color).is_some() {
                    self.record_step(self.data.board_of_label(label));
                }
            }
            Message::DeleteLabel(label) => match delete_prompt(&self.data, label) {
                Some(prompt) => {
                    let action = Message::RemoveLabel(label);
                    self.confirm = Some(Confirm { prompt, action });
                }
                None => self.delete_label(label),
            },
            Message::RemoveLabel(label) => self.delete_label(label),
            Message::NewLabel => {
                let used = |key: &str| {
                    let board = self.board();
                    board.is_some_and(|b| b.labels.iter().any(|l| l.color == key))
                };
                let color = PALETTE.iter().map(|(k, _)| *k).find(|k| !used(k));
                if let Some(picker) = &mut self.label_picker {
                    picker.expanded = None;
                    picker.new = Some(NewLabel {
                        text: String::new(),
                        color: color.unwrap_or(PALETTE[0].0).into(),
                    });
                    return operation::focus(NEW_INPUT);
                }
            }
            Message::NewLabelChanged(text) => {
                if let Some(new) = self.label_picker.as_mut().and_then(|p| p.new.as_mut()) {
                    new.text = text;
                }
            }
            Message::NewLabelColor(color) => {
                if let Some(new) = self.label_picker.as_mut().and_then(|p| p.new.as_mut()) {
                    new.color = color;
                }
            }
            Message::CreateLabel => {
                let new = self.label_picker.as_ref().and_then(|p| p.new.clone());
                let board = card.and_then(|card| self.data.board_of_card(card));
                if let Some((new, board)) = new.zip(board)
                    && let Some(label) = self.data.add_label(board, &new.text, &new.color)
                {
                    self.label_picker.as_mut().expect("open").new = None;
                    // Created from this Card's picker: it goes on the Card, same step.
                    card.map(|card| self.data.toggle_label(card, label));
                    self.record_step(Some(board));
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn delete_label(&mut self, label: Id) {
        let board = self.data.board_of_label(label);
        if self.data.delete_label(label).is_some() {
            self.record_step(board);
        }
    }

    /// Esc on the picker closes it (an open delete prompt is closed first by `App.confirm`).
    pub(crate) fn escape_labels(&mut self) {
        self.commit_pending();
        self.label_picker = None;
    }

    /// Run after every update: the picker lives only while its Card is open, and drops
    /// an editor whose Label is gone.
    pub(crate) fn validate_labels(&mut self) {
        if self
            .label_picker
            .as_ref()
            .is_some_and(|p| self.open_card() != Some(p.card))
        {
            self.label_picker = None;
        }
        if let Some(p) = &mut self.label_picker {
            let exists = |id: &Id| self.data.label(*id).is_some();
            p.expanded = p.expanded.filter(exists);
        }
    }

    /// The open Card's Labels row: the Card's chips (or "+ Add Label"); a click opens the picker.
    pub(crate) fn labels_row<'a>(&'a self, card: &'a Card) -> Element<'a, Message> {
        let chips = match self.board() {
            Some(board) => chips(board, card),
            None => None,
        };
        let shown: Element<_> = chips.unwrap_or_else(|| text("+ Add Label").size(12).into());
        row![
            text("Labels").size(13).font(crate::app::HEADING),
            crate::chrome::tip(
                button(shown)
                    .style(button::text)
                    .padding(2)
                    .on_press(Message::OpenLabels),
                "Labels (L)"
            )
        ]
        .spacing(8)
        .align_y(Center)
        .into()
    }

    /// The picker overlay over the dimmed app.
    pub(crate) fn view_label_picker<'a>(&'a self, picker: &'a LabelPicker) -> Element<'a, Message> {
        let Some((board, card)) = self.board().zip(self.data.card(picker.card)) else {
            return space().into();
        };
        let header = dialog_header("Labels", Message::CloseLabels);
        let rows = self.picker_rows(board, &card.labels, Message::ToggleLabel, Some(picker));
        // The list scrolls past 360px; the dialog wraps its content instead of filling the window.
        let list = container(scrollable(rows)).max_height(360);
        let body = column![header, list, self.new_label_form(board, picker)].spacing(12);
        dialog(container(body).width(360))
    }

    /// One row per Label of `board`: its colour dot and a checkbox that sends `toggle(id)`. With `editing` there is
    /// also a pencil per Label opening its rename / recolour / delete editor. The Filter reuses
    /// this with `None`.
    pub(crate) fn picker_rows<'a>(
        &'a self,
        board: &'a Board,
        checked: &[Id],
        toggle: fn(Id) -> Message,
        editing: Option<&'a LabelPicker>,
    ) -> Element<'a, Message> {
        let rows = board.labels.iter().map(|label| {
            let id = label.id;
            // A colour dot, not a chip: the checkbox already shows the full name.
            let color = swatch(&label.color);
            let dot = container(space().width(12).height(12)).style(move |_| container::Style {
                background: Some(color.into()),
                border: Border::default().rounded(6),
                ..Default::default()
            });
            let mut line = row![
                dot,
                checkbox(checked.contains(&id))
                    .label(label.name.as_str())
                    .on_toggle(move |_| toggle(id)),
                space::horizontal(),
            ]
            .spacing(8)
            .align_y(Center);
            let Some(picker) = editing else {
                return line.into();
            };
            line = line.push(crate::chrome::tip(
                button("✎")
                    .style(button::text)
                    .on_press(Message::EditLabel(id)),
                "Rename or recolour",
            ));
            if picker.expanded != Some(id) {
                return line.into();
            }
            let editor = match self
                .editing
                .as_ref()
                .filter(|e| e.field == Field::LabelName(id))
            {
                Some(edit) => {
                    let clash = self.data.is_label_clash(id, &edit.text);
                    column![
                        text_input("Name", &edit.text)
                            .id(EDIT_INPUT)
                            .on_input(Message::EditChanged)
                            .on_submit(Message::CommitEdit)
                    ]
                    .extend(clash.then(clash_text))
                }
                None => column![],
            };
            let editor = editor
                .push(swatches(&label.color, move |key| {
                    Message::SetLabelColor(id, key.into())
                }))
                .push(
                    button("Delete Label")
                        .style(button::danger)
                        .on_press(Message::DeleteLabel(id)),
                )
                .spacing(6);
            column![line, container(editor).padding([0, 12])]
                .spacing(6)
                .into()
        });
        column(rows).spacing(8).into()
    }

    /// "+ New Label", or its name box, colour swatches and Create button.
    fn new_label_form<'a>(
        &'a self,
        board: &'a Board,
        picker: &'a LabelPicker,
    ) -> Element<'a, Message> {
        let Some(new) = &picker.new else {
            return button("+ New Label")
                .style(crate::app::quiet)
                .on_press(Message::NewLabel)
                .into();
        };
        let clash = self.data.label_clash(board.id, &new.text, None);
        column![
            text_input("Label name", &new.text)
                .id(NEW_INPUT)
                .on_input(Message::NewLabelChanged)
                .on_submit(Message::CreateLabel)
        ]
        .extend(clash.then(clash_text))
        .push(swatches(&new.color, |key| {
            Message::NewLabelColor(key.into())
        }))
        .push(button("Create").on_press(Message::CreateLabel))
        .spacing(6)
        .into()
    }
}

fn clash_text<'a>() -> Element<'a, Message> {
    text("A Label with this name already exists")
        .size(12)
        .style(text::danger)
        .into()
}

/// The palette as round buttons; `current` has a ring.
fn swatches<'a>(
    current: &str,
    pick: impl Fn(&'static str) -> Message + 'a,
) -> Element<'a, Message> {
    let buttons = PALETTE.iter().map(|(key, color)| {
        let selected = *key == current;
        button(space().width(14).height(14))
            .padding(2)
            .style(move |theme: &iced::Theme, _| button::Style {
                background: Some((*color).into()),
                border: Border {
                    radius: 12.0.into(),
                    width: if selected { 3.0 } else { 0.0 },
                    color: theme.palette().text,
                },
                ..Default::default()
            })
            .on_press(pick(key))
            .into()
    });
    row(buttons).spacing(4).into()
}

/// One chip: the name cut at 12 characters on the Label's colour.
fn chip(label: &Label) -> Element<'_, Message> {
    let color = swatch(&label.color);
    container(text(chip_text(&label.name)).size(12).color(Color::WHITE))
        .padding([1, 6])
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// The Card's chips in Board Label order, wrapping; `None` when it has no Labels.
pub(crate) fn chips<'a>(board: &'a Board, card: &Card) -> Option<Element<'a, Message>> {
    let labels = board_labels(board, Some(card));
    (!labels.is_empty()).then(|| row(labels.into_iter().map(chip)).spacing(4).wrap().into())
}
