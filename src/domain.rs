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
    /// Archived Cards, oldest archived first.
    #[serde(default)]
    pub archived_cards: Vec<Archived>,
}

/// A Card in its Board's Archive, remembering the Column it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Archived {
    pub card: Card,
    /// The original Column; may point at a Column that no longer exists.
    pub column_id: Id,
    pub archived_at: String,
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
}

fn archived(mut card: Card, column_id: Id) -> Archived {
    let archived_at = now();
    card.updated = archived_at.clone();
    Archived {
        card,
        column_id,
        archived_at,
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
            archived_cards: Vec::new(),
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

    /// Sends an active Card to its Board's Archive, remembering its Column.
    pub fn archive_card(&mut self, card: Id) -> Option<()> {
        let (b, c) = self.column_where(|c| c.cards.iter().any(|x| x.id == card))?;
        let board = &mut self.boards[b];
        let column = &mut board.columns[c];
        let at = column.cards.iter().position(|x| x.id == card)?;
        let column_id = column.id;
        let card = column.cards.remove(at);
        board.archived_cards.push(archived(card, column_id));
        Some(())
    }

    /// Removes a Column, archiving its Cards (in order) with it as their original Column.
    /// Returns how many Cards were archived.
    pub fn remove_column(&mut self, column: Id) -> Option<usize> {
        let (b, c) = self.column_where(|x| x.id == column)?;
        let board = &mut self.boards[b];
        let removed = board.columns.remove(c);
        let n = removed.cards.len();
        let cards = removed.cards.into_iter().map(|card| archived(card, column));
        board.archived_cards.extend(cards);
        Some(n)
    }

    /// Puts an archived Card at the bottom of its original Column, else of its Board's first
    /// Column; a Board without Columns gets a "To do" Column first. Returns the Column.
    pub fn restore_card(&mut self, card: Id) -> Option<Id> {
        let b = self
            .boards
            .iter()
            .position(|b| b.archived_cards.iter().any(|a| a.card.id == card))?;
        let spare = self.boards[b].columns.is_empty().then(|| self.new_id());
        let board = &mut self.boards[b];
        let at = board
            .archived_cards
            .iter()
            .position(|a| a.card.id == card)?;
        let Archived {
            mut card,
            column_id,
            ..
        } = board.archived_cards.remove(at);
        card.updated = now();
        if let Some(id) = spare {
            board.columns.push(Column {
                id,
                name: "To do".into(),
                limit: None,
                cards: Vec::new(),
            });
        }
        let target = board.columns.iter_mut().find(|c| c.id == column_id);
        let target = match target {
            Some(column) => column,
            None => &mut board.columns[0],
        };
        target.cards.push(card);
        Some(target.id)
    }

    /// Removes an archived Card for good.
    pub fn delete_archived(&mut self, card: Id) -> Option<()> {
        let board = self
            .boards
            .iter_mut()
            .find(|b| b.archived_cards.iter().any(|a| a.card.id == card))?;
        board.archived_cards.retain(|a| a.card.id != card);
        Some(())
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
