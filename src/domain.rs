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
    #[serde(default)]
    pub cards: Vec<Card>,
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
