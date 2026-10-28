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
    /// When the Board was archived (RFC 3339 UTC); `None` while active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub columns: Vec<Column>,
    /// Archived Cards, oldest archived first.
    #[serde(default)]
    pub archived_cards: Vec<Archived>,
    /// This Board's Labels, in display order.
    #[serde(default)]
    pub labels: Vec<Label>,
}

/// A Board's Label. `color` is a palette key (see `labels::PALETTE`), never an index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub id: Id,
    pub name: String,
    pub color: String,
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
    #[serde(deserialize_with = "crate::repair::limit")]
    pub limit: Option<u32>,
    /// Folded to a narrow strip; its Cards are hidden but still counted and still take drops.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl Column {
    /// More Cards than the Column Limit.
    pub fn over_limit(&self) -> bool {
        self.limit.is_some_and(|l| self.cards.len() > l as usize)
    }

    /// The header count's tooltip: what the Column Limit is, and whether it is exceeded.
    pub fn limit_tip(&self) -> Option<String> {
        let limit = self.limit?;
        Some(match self.over_limit() {
            true => format!("Over the Column Limit ({} of {limit})", self.cards.len()),
            false => format!("Column Limit {limit}"),
        })
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
    /// IDs of Labels of the Card's own Board. Read them in Board order via `Data::card_labels`.
    #[serde(default)]
    pub labels: Vec<Id>,
    /// Markdown (ADR 0003); empty when the Card has none.
    #[serde(default)]
    pub description: String,
    /// Absolute paths of files on this machine, as the Open dialog gave them; never copied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
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

    /// Untouched for more than [`STALE_DAYS`] before `now`. An unreadable `updated` is fresh.
    pub fn is_stale(&self, now: jiff::Timestamp) -> bool {
        let limit = jiff::SignedDuration::from_hours(24 * STALE_DAYS);
        self.updated
            .parse::<jiff::Timestamp>()
            .is_ok_and(|updated| now.duration_since(updated) > limit)
    }
}

/// Days without a change after which a Card fades (ageing).
pub const STALE_DAYS: i64 = 14;

fn archived(mut card: Card, column_id: Id) -> Archived {
    let archived_at = now();
    card.updated = archived_at.clone();
    Archived {
        card,
        column_id,
        archived_at,
    }
}

/// "1 Card", "n Cards".
pub fn cards(n: usize) -> String {
    format!("{n} Card{}", if n == 1 { "" } else { "s" })
}

/// The current time as an RFC 3339 UTC string with second precision.
pub fn now() -> String {
    jiff::Timestamp::from_second(jiff::Timestamp::now().as_second())
        .expect("now is in range")
        .to_string()
}

impl Data {
    pub(crate) fn new_id(&mut self) -> Id {
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
                collapsed: false,
                cards: Vec::new(),
            })
            .into();
        self.boards.push(Board {
            id,
            name: name.into(),
            archived: false,
            archived_at: None,
            columns,
            archived_cards: Vec::new(),
            labels: Vec::new(),
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
        board.archived_at = Some(now());
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
        board.archived_at = None;
        self.boards.push(board);
        Some(())
    }

    /// Moves a Board into the gap before the active Board at `gap` (`gap` counts active
    /// Boards only; past the end = last). Archived Boards keep their order.
    pub fn move_board(&mut self, id: Id, gap: usize) -> Option<()> {
        let at = self.boards.iter().position(|b| b.id == id && !b.archived)?;
        let from = self.boards[..at].iter().filter(|b| !b.archived).count();
        let board = self.boards.remove(at);
        let slot = gap.saturating_sub((from < gap) as usize);
        let mut active = self.boards.iter().enumerate().filter(|(_, b)| !b.archived);
        let at = match active.nth(slot) {
            Some((i, _)) => i,
            // After the last active Board (or no other active Board: stay put).
            None => match self.boards.iter().rposition(|b| !b.archived) {
                Some(last) => last + 1,
                None => at,
            },
        };
        self.boards.insert(at, board);
        Some(())
    }

    /// Moves a Column into the gap before its Board's Column at `gap` (past the end = last).
    pub fn move_column(&mut self, id: Id, gap: usize) -> Option<()> {
        let columns = &mut self
            .boards
            .iter_mut()
            .find(|b| b.columns.iter().any(|c| c.id == id))?
            .columns;
        let at = columns.iter().position(|c| c.id == id)?;
        let column = columns.remove(at);
        columns.insert(
            gap.saturating_sub((at < gap) as usize).min(columns.len()),
            column,
        );
        Some(())
    }

    /// The Board to show for `current`: itself while active; if archived or gone, the next
    /// active Board below it, else the one above, else the first active one. `None` (nothing
    /// open) opens the first active Board once there is one (undo, restore).
    /// `order` is the Board IDs from before the change, used when `current` is gone.
    pub fn fallback(&self, current: Option<Id>, order: &[Id]) -> Option<Id> {
        let Some(current) = current else {
            return self.boards.iter().find(|b| !b.archived).map(|b| b.id);
        };
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
            collapsed: false,
            cards: Vec::new(),
        });
        Some(id)
    }

    /// Collapses an expanded Column, or expands a collapsed one. Unknown Column: nothing.
    pub fn toggle_collapse(&mut self, column: Id) -> Option<()> {
        let column = self
            .boards
            .iter_mut()
            .flat_map(|b| &mut b.columns)
            .find(|c| c.id == column)?;
        column.collapsed = !column.collapsed;
        Some(())
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
            labels: Vec::new(),
            description: String::new(),
            files: Vec::new(),
        });
        Some(id)
    }

    /// Copies a Card right after itself: new IDs for it and its Items, Items unticked, created
    /// and updated now; every other field is kept. Returns the copy's ID; unknown Card: nothing.
    pub fn duplicate_card(&mut self, card: Id) -> Option<Id> {
        let (b, c) = self.holding(card)?;
        let at = self.boards[b].columns[c]
            .cards
            .iter()
            .position(|x| x.id == card)?;
        let mut copy = self.boards[b].columns[c].cards[at].clone();
        copy.id = self.new_id();
        for item in &mut copy.checklist {
            item.id = self.new_id();
            item.done = false;
        }
        copy.created = now();
        copy.updated = copy.created.clone();
        let id = copy.id;
        self.boards[b].columns[c].cards.insert(at + 1, copy);
        Some(id)
    }

    /// Moves a Card to `index` in a Column (clamped to its end), touching it unless it stays
    /// where it was. Unknown Card or Column: nothing.
    pub fn move_card(&mut self, card: Id, column: Id, index: usize) {
        let (Some((fb, fc)), Some((tb, tc))) =
            (self.holding(card), self.column_where(|c| c.id == column))
        else {
            return;
        };
        let same_column = (fb, fc) == (tb, tc);
        let from = &mut self.boards[fb].columns[fc].cards;
        let at = from.iter().position(|x| x.id == card).expect("found above");
        let mut moving = from.remove(at);
        let to = &mut self.boards[tb].columns[tc].cards;
        let index = index.min(to.len());
        // Moving is a change to the Card (ageing); staying put is not. Neighbours shifting isn't.
        if !(same_column && index == at) {
            moving.updated = now();
        }
        to.insert(index, moving);
    }

    /// Moves an active Card to the bottom of another active Board's first Column (a "To do"
    /// Column is created if it has none). Labels are matched by name (trimmed, case-insensitive):
    /// the target's Label is used (its colour wins), else the source's is appended to the target.
    /// Own or archived Board, unknown Card or Board: nothing.
    pub fn move_to_board(&mut self, card: Id, board: Id) -> Option<()> {
        let (b, c) = self.holding(card)?;
        let t = self
            .boards
            .iter()
            .position(|x| x.id == board && !x.archived)?;
        if t == b {
            return None;
        }
        let from = &mut self.boards[b].columns[c].cards;
        let at = from.iter().position(|x| x.id == card)?;
        let mut card = from.remove(at);
        let source = card
            .labels
            .iter()
            .filter_map(|id| self.boards[b].labels.iter().find(|l| l.id == *id))
            .cloned()
            .collect::<Vec<_>>();
        let key = |name: &str| name.trim().to_lowercase();
        card.labels.clear();
        for label in source {
            let found = self.boards[t]
                .labels
                .iter()
                .find(|l| key(&l.name) == key(&label.name))
                .map(|l| l.id);
            let id = found.unwrap_or_else(|| {
                let id = self.new_id();
                self.boards[t].labels.push(Label { id, ..label });
                id
            });
            card.labels.push(id);
        }
        if self.boards[t].columns.is_empty() {
            let id = self.new_id();
            self.boards[t].columns.push(Column {
                id,
                name: "To do".into(),
                limit: None,
                collapsed: false,
                cards: Vec::new(),
            });
        }
        card.updated = now(); // its Column changed
        self.boards[t].columns[0].cards.push(card);
        Some(())
    }

    /// Sends an active Card to its Board's Archive, remembering its Column.
    pub fn archive_card(&mut self, card: Id) -> Option<()> {
        let (b, c) = self.holding(card)?;
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
                collapsed: false,
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
    pub(crate) fn column_where(&self, f: impl Fn(&Column) -> bool) -> Option<(usize, usize)> {
        self.boards
            .iter()
            .enumerate()
            .find_map(|(b, board)| Some((b, board.columns.iter().position(&f)?)))
    }

    /// (Board index, Column index) of the Column holding this active Card.
    pub(crate) fn holding(&self, card: Id) -> Option<(usize, usize)> {
        self.column_where(|c| c.cards.iter().any(|x| x.id == card))
    }

    /// The Board holding this active Card.
    pub fn board_of_card(&self, card: Id) -> Option<Id> {
        self.holding(card).map(|(b, _)| self.boards[b].id)
    }

    /// The Column holding this active Card.
    pub fn column_of_card(&self, card: Id) -> Option<Id> {
        self.holding(card)
            .map(|(b, c)| self.boards[b].columns[c].id)
    }

    /// The Board whose Archive holds this Card.
    pub fn board_of_archived(&self, card: Id) -> Option<Id> {
        let has = |b: &&Board| b.archived_cards.iter().any(|a| a.card.id == card);
        Some(self.boards.iter().find(has)?.id)
    }

    /// The Column with this ID.
    pub fn column(&self, id: Id) -> Option<&Column> {
        let (b, c) = self.column_where(|c| c.id == id)?;
        Some(&self.boards[b].columns[c])
    }

    /// The Board holding this Column.
    pub fn board_of_column(&self, column: Id) -> Option<Id> {
        let (b, _) = self.column_where(|c| c.id == column)?;
        Some(self.boards[b].id)
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

    /// The name of the Board, Column or Label, or the title of the Card, with this ID.
    pub fn name(&self, id: Id) -> Option<&str> {
        let found = self.boards.iter().find_map(|b| {
            if b.id == id {
                return Some(b.name.as_str());
            }
            b.columns
                .iter()
                .find_map(|c| {
                    if c.id == id {
                        return Some(c.name.as_str());
                    }
                    c.cards
                        .iter()
                        .find(|x| x.id == id)
                        .map(|x| x.title.as_str())
                })
                .or_else(|| {
                    b.labels
                        .iter()
                        .find(|l| l.id == id)
                        .map(|l| l.name.as_str())
                })
        });
        found.or_else(|| self.item(id).map(|i| i.text.as_str()))
    }

    /// Renames the Board or Column, or retitles the Card, with this ID to the trimmed name.
    /// A blank name, or a Label name that clashes on its Board, changes nothing. Does not touch `updated`: the caller does on commit.
    pub fn rename(&mut self, id: Id, name: &str) -> Option<()> {
        let name = name.trim();
        if name.is_empty() || self.is_label_clash(id, name) {
            return None;
        }
        let target = self.boards.iter_mut().find_map(|b| {
            if b.id == id {
                return Some(&mut b.name);
            }
            b.columns
                .iter_mut()
                .find_map(|c| {
                    if c.id == id {
                        return Some(&mut c.name);
                    }
                    c.cards
                        .iter_mut()
                        .find(|x| x.id == id)
                        .map(|x| &mut x.title)
                })
                .or_else(|| {
                    b.labels
                        .iter_mut()
                        .find(|l| l.id == id)
                        .map(|l| &mut l.name)
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
