//! Plain data and pure operations on Boards, Columns and Cards. No iced types.

use serde::{Deserialize, Serialize};

pub type Id = u64;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Data {
    /// Monotonic ID counter, shared by every entity type, never reused.
    pub next_id: Id,
    pub boards: Vec<Board>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub columns: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub id: Id,
    pub name: String,
    /// Column Limit (>= 1). Exceeding it only flags the Column; Cards are still accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl Column {
    /// More Cards than the Column Limit.
    pub fn over_limit(&self) -> bool {
        self.limit.is_some_and(|l| self.cards.len() > l as usize)
    }

    /// The header count: "n", or "n/limit" with a Column Limit.
    pub fn count_label(&self) -> String {
        match self.limit {
            Some(limit) => format!("{}/{limit}", self.cards.len()),
            None => self.cards.len().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: Id,
    pub title: String,
    /// RFC 3339 UTC, second precision. Blank in files from before timestamps existed.
    #[serde(default)]
    pub created: String,
    /// When the Card's own fields last changed (not when neighbouring Cards shift).
    #[serde(default)]
    pub updated: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklist: Vec<Item>,
}

/// A Checklist Item. Edits target it by `id`, never by position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: Id,
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

impl Card {
    /// "done/total" when the Checklist has Items.
    pub fn progress(&self) -> Option<String> {
        let done = self.checklist.iter().filter(|i| i.done).count();
        (!self.checklist.is_empty()).then(|| format!("{done}/{}", self.checklist.len()))
    }
}

/// The current time as an RFC 3339 UTC string with second precision.
pub fn now() -> String {
    jiff::Timestamp::from_second(jiff::Timestamp::now().as_second())
        .expect("now is in range")
        .to_string()
}

impl Data {
    fn new_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    /// Appends a Board with To do / Doing / Done Columns. A blank name creates nothing.
    pub fn create_board(&mut self, name: &str) -> Option<Id> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let id = self.new_id();
        let columns = ["To do", "Doing", "Done"]
            .map(|name| Column {
                id: self.new_id(),
                name: name.into(),
                limit: None,
                cards: Vec::new(),
            })
            .into();
        self.boards.push(Board {
            id,
            name: name.into(),
            columns,
        });
        Some(id)
    }

    /// Appends an empty Column after the Board's last one. An unknown Board creates nothing.
    pub fn add_column(&mut self, board: Id) -> Option<Id> {
        let id = self.new_id();
        let board = self.boards.iter_mut().find(|b| b.id == board)?;
        board.columns.push(Column {
            id,
            name: "New column".into(),
            limit: None,
            cards: Vec::new(),
        });
        Some(id)
    }

    /// Sets the Column Limit from typed text: a positive integer, or blank to remove it.
    /// Anything else (or an unknown Column) changes nothing and returns `None`.
    pub fn set_limit(&mut self, column: Id, input: &str) -> Option<()> {
        let input = input.trim();
        let limit = if input.is_empty() {
            None
        } else {
            Some(input.parse::<u32>().ok().filter(|l| *l >= 1)?)
        };
        let column = self
            .boards
            .iter_mut()
            .flat_map(|b| &mut b.columns)
            .find(|c| c.id == column)?;
        column.limit = limit;
        Some(())
    }

    /// Appends a Card to the bottom of a Column. A blank title, or an unknown Column, creates nothing.
    pub fn add_card(&mut self, column: Id, title: &str) -> Option<Id> {
        let title = title
            .split(['\r', '\n'])
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let title = title.trim();
        if title.is_empty() {
            return None;
        }
        let id = self.new_id();
        let column = self
            .boards
            .iter_mut()
            .flat_map(|b| &mut b.columns)
            .find(|c| c.id == column)?;
        let now = now();
        column.cards.push(Card {
            id,
            title: title.into(),
            created: now.clone(),
            updated: now,
            checklist: Vec::new(),
        });
        Some(id)
    }

    /// Moves a Card to `index` in a Column (clamped to its end). Unknown Card or Column: nothing.
    pub fn move_card(&mut self, card: Id, column: Id, index: usize) {
        let (Some((fb, fc)), Some((tb, tc))) = (
            self.column_where(|c| c.cards.iter().any(|x| x.id == card)),
            self.column_where(|c| c.id == column),
        ) else {
            return;
        };
        let new_column = (fb, fc) != (tb, tc);
        let from = &mut self.boards[fb].columns[fc].cards;
        let at = from.iter().position(|x| x.id == card).expect("found above");
        let mut moving = from.remove(at);
        if new_column {
            moving.updated = now(); // its Column changed; reordering alone is not its own change
        }
        let to = &mut self.boards[tb].columns[tc].cards;
        to.insert(index.min(to.len()), moving);
    }

    /// (Board index, Column index) of the first Column matching `f`.
    fn column_where(&self, f: impl Fn(&Column) -> bool) -> Option<(usize, usize)> {
        self.boards
            .iter()
            .enumerate()
            .find_map(|(b, board)| Some((b, board.columns.iter().position(&f)?)))
    }

    /// The Card with this ID.
    pub fn card(&self, id: Id) -> Option<&Card> {
        self.boards
            .iter()
            .flat_map(|b| &b.columns)
            .flat_map(|c| &c.cards)
            .find(|c| c.id == id)
    }

    /// Marks the Card as changed just now.
    pub fn touch(&mut self, card: Id) {
        let columns = self.boards.iter_mut().flat_map(|b| &mut b.columns);
        if let Some(card) = columns.flat_map(|c| &mut c.cards).find(|c| c.id == card) {
            card.updated = now();
        }
    }

    /// The name of the Board or Column, or the title of the Card, with this ID.
    pub fn name(&self, id: Id) -> Option<&str> {
        let found = self.boards.iter().find_map(|b| {
            if b.id == id {
                return Some(b.name.as_str());
            }
            b.columns.iter().find_map(|c| {
                if c.id == id {
                    return Some(c.name.as_str());
                }
                c.cards
                    .iter()
                    .find(|x| x.id == id)
                    .map(|x| x.title.as_str())
            })
        });
        found.or_else(|| self.item(id).map(|i| i.text.as_str()))
    }

    /// Renames the Board or Column, or retitles the Card, with this ID to the trimmed name.
    /// A blank name changes nothing. Does not touch `updated`: the caller does on commit.
    pub fn rename(&mut self, id: Id, name: &str) -> Option<()> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let target = self.boards.iter_mut().find_map(|b| {
            if b.id == id {
                return Some(&mut b.name);
            }
            b.columns.iter_mut().find_map(|c| {
                if c.id == id {
                    return Some(&mut c.name);
                }
                c.cards
                    .iter_mut()
                    .find(|x| x.id == id)
                    .map(|x| &mut x.title)
            })
        });
        match target {
            Some(target) => *target = name.into(),
            None => self.item_mut(id)?.text = name.into(),
        }
        Some(())
    }

    fn items_mut(&mut self) -> impl Iterator<Item = &mut Vec<Item>> {
        let columns = self.boards.iter_mut().flat_map(|b| &mut b.columns);
        columns.flat_map(|c| &mut c.cards).map(|c| &mut c.checklist)
    }

    fn item_mut(&mut self, id: Id) -> Option<&mut Item> {
        self.items_mut().flatten().find(|i| i.id == id)
    }

    /// The Checklist Item with this ID.
    pub fn item(&self, id: Id) -> Option<&Item> {
        let columns = self.boards.iter().flat_map(|b| &b.columns);
        let cards = columns.flat_map(|c| &c.cards);
        cards.flat_map(|c| &c.checklist).find(|i| i.id == id)
    }

    /// The Card whose Checklist holds this Item.
    pub fn card_of_item(&self, item: Id) -> Option<Id> {
        let columns = self.boards.iter().flat_map(|b| &b.columns);
        let mut cards = columns.flat_map(|c| &c.cards);
        cards
            .find(|c| c.checklist.iter().any(|i| i.id == item))
            .map(|c| c.id)
    }

    /// Adds a blank Item after `after` (an Item of the Card), or at the end. The caller edits it
    /// and removes it again if it stays blank. Does not touch `updated`. Unknown Card: nothing.
    pub fn add_item(&mut self, card: Id, after: Option<Id>) -> Option<Id> {
        let id = self.new_id();
        let list = self.items_of(card)?;
        let at = after.and_then(|a| list.iter().position(|i| i.id == a));
        let at = at.map_or(list.len(), |p| p + 1);
        list.insert(
            at,
            Item {
                id,
                text: String::new(),
                done: false,
            },
        );
        Some(id)
    }

    /// Ticks or unticks an Item.
    pub fn toggle_item(&mut self, item: Id) -> Option<()> {
        let item = self.item_mut(item)?;
        item.done = !item.done;
        Some(())
    }

    /// Moves an Item one place up (`-1`) or down (`1`); at an end, nothing changes and `None` returns.
    pub fn move_item(&mut self, item: Id, by: isize) -> Option<()> {
        let list = self.items_mut().find(|l| l.iter().any(|i| i.id == item))?;
        let from = list.iter().position(|i| i.id == item)?;
        let to = from.checked_add_signed(by).filter(|to| *to < list.len())?;
        list.swap(from, to);
        Some(())
    }

    pub fn remove_item(&mut self, item: Id) -> Option<()> {
        let list = self.items_mut().find(|l| l.iter().any(|i| i.id == item))?;
        list.retain(|i| i.id != item);
        Some(())
    }

    fn items_of(&mut self, card: Id) -> Option<&mut Vec<Item>> {
        let columns = self.boards.iter_mut().flat_map(|b| &mut b.columns);
        let mut cards = columns.flat_map(|c| &mut c.cards);
        cards.find(|c| c.id == card).map(|c| &mut c.checklist)
    }
}
