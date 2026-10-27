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
        column.cards.push(Card {
            id,
            title: title.into(),
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
        let from = &mut self.boards[fb].columns[fc].cards;
        let at = from.iter().position(|x| x.id == card).expect("found above");
        let moving = from.remove(at);
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

    /// The name of the Board or Column with this ID.
    pub fn name(&self, id: Id) -> Option<&str> {
        self.boards.iter().find_map(|b| {
            if b.id == id {
                return Some(b.name.as_str());
            }
            b.columns
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.as_str())
        })
    }

    /// Renames the Board or Column with this ID to the trimmed name. A blank name changes nothing.
    pub fn rename(&mut self, id: Id, name: &str) -> Option<()> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let target = self.boards.iter_mut().find_map(|b| {
            if b.id == id {
                return Some(&mut b.name);
            }
            b.columns
                .iter_mut()
                .find(|c| c.id == id)
                .map(|c| &mut c.name)
        })?;
        *target = name.into();
        Some(())
    }
}
