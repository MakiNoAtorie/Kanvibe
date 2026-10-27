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
    /// Archived Boards sit in the sidebar's "Archived (n)" section and cannot be opened.
    #[serde(default)]
    pub archived: bool,
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
    /// Markdown (ADR 0003); empty when the Card has none.
    #[serde(default)]
    pub description: String,
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
            archived: false,
            columns,
        });
        Some(id)
    }

    /// The active Board with this ID.
    pub fn active_board(&self, id: Id) -> Option<&Board> {
        self.boards.iter().find(|b| b.id == id && !b.archived)
    }

    /// Archives an active Board. Unknown or already archived: nothing.
    pub fn archive_board(&mut self, id: Id) -> Option<()> {
        let board = self.boards.iter_mut().find(|b| b.id == id && !b.archived)?;
        board.archived = true;
        Some(())
    }

    /// Permanently removes an archived Board with everything in it. Active Boards: nothing.
    pub fn delete_board(&mut self, id: Id) -> Option<()> {
        let at = self.boards.iter().position(|b| b.id == id && b.archived)?;
        self.boards.remove(at);
        Some(())
    }

    /// Makes an archived Board active again, at the bottom of the active list.
    pub fn restore_board(&mut self, id: Id) -> Option<()> {
        let at = self.boards.iter().position(|b| b.id == id && b.archived)?;
        let mut board = self.boards.remove(at);
        board.archived = false;
        self.boards.push(board);
        Some(())
    }

    /// The Board to show for `current`: itself while active; if archived or gone, the next
    /// active Board below it, else the one above, else the first active one; `None` stays `None`.
    /// `order` is the Board IDs from before the change, used when `current` is gone.
    pub fn fallback(&self, current: Option<Id>, order: &[Id]) -> Option<Id> {
        let current = current?;
        if self.active_board(current).is_some() {
            return Some(current);
        }
        let now: Vec<Id> = self.boards.iter().map(|b| b.id).collect();
        let order = if now.contains(&current) { &now } else { order };
        let at = order.iter().position(|id| *id == current).unwrap_or(0);
        let (above, below) = order.split_at(at);
        let active = |id: &&Id| self.active_board(**id).is_some();
        below
            .iter()
            .find(active)
            .or(above.iter().rev().find(active))
            .or(now.iter().find(active))
            .copied()
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
            description: String::new(),
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

    /// The Card's Markdown description.
    pub fn description(&self, card: Id) -> Option<&str> {
        self.card(card).map(|c| c.description.as_str())
    }

    /// Replaces the Card's description as typed. Does not touch `updated`: the caller does on commit.
    pub fn set_description(&mut self, card: Id, text: &str) -> Option<()> {
        let columns = self.boards.iter_mut().flat_map(|b| &mut b.columns);
        let card = columns.flat_map(|c| &mut c.cards).find(|c| c.id == card)?;
        card.description = text.into();
        Some(())
    }

    /// The name of the Board or Column, or the title of the Card, with this ID.
    pub fn name(&self, id: Id) -> Option<&str> {
        self.boards.iter().find_map(|b| {
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
        })
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
        })?;
        *target = name.into();
        Some(())
    }
}
