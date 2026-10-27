//! Load repairs: a hand-edited file with bad references is repaired, never refused.
//! Used on load and by Board import.
use std::collections::HashSet;

use serde::{Deserialize, Deserializer};

use crate::domain::{Board, Card, Data, Id};
use crate::labels::PALETTE;

/// The Data with every repair applied, and whether anything changed.
pub fn repair(mut data: Data) -> (Data, bool) {
    let before = data.clone();
    let mut next = data.next_id;
    each_id(&mut data.boards, &mut |id| next = next.max(*id));
    // A repeated ID gets a fresh one; references to it (Card Labels, an archived Card's
    // Column) keep meaning the first holder.
    let mut seen = HashSet::new();
    each_id(&mut data.boards, &mut |id| {
        if !seen.insert(*id) {
            next += 1;
            *id = next;
        }
    });
    data.next_id = next;
    for board in &mut data.boards {
        name(&mut board.name, "Untitled");
        for label in &mut board.labels {
            name(&mut label.name, "Label");
            if !PALETTE.iter().any(|(key, _)| *key == label.color) {
                label.color = PALETTE[0].0.into();
            }
        }
        for column in &mut board.columns {
            name(&mut column.name, "New column");
            column.limit = column.limit.filter(|l| *l >= 1);
        }
        let labels: HashSet<Id> = board.labels.iter().map(|l| l.id).collect();
        for card in cards(board) {
            name(&mut card.title, "Untitled");
            card.labels.retain(|id| labels.contains(id));
            card.checklist.retain(|i| !i.text.trim().is_empty());
            for item in &mut card.checklist {
                item.text = item.text.trim().into();
            }
        }
    }
    let repaired = data != before;
    (data, repaired)
}

/// Trims a name; a blank one becomes `default`.
fn name(name: &mut String, default: &str) {
    let trimmed = name.trim();
    *name = if trimmed.is_empty() { default } else { trimmed }.into();
}

/// Reads a Column Limit leniently: any integer below 1 reads as 0, which `repair` clears.
pub(crate) fn limit<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let limit = Option::<i64>::deserialize(d)?;
    Ok(limit.map(|l| l.clamp(0, u32::MAX.into()) as u32))
}

/// Every entity ID in file order: each Board, its Labels, its Columns with their Cards and
/// Items, then its archived Cards.
fn each_id(boards: &mut [Board], f: &mut impl FnMut(&mut Id)) {
    for board in boards {
        f(&mut board.id);
        board.labels.iter_mut().for_each(|l| f(&mut l.id));
        for column in &mut board.columns {
            f(&mut column.id);
            column.cards.iter_mut().for_each(|c| card_ids(c, f));
        }
        let archived = board.archived_cards.iter_mut();
        archived.for_each(|a| card_ids(&mut a.card, f));
    }
}

fn card_ids(card: &mut Card, f: &mut impl FnMut(&mut Id)) {
    f(&mut card.id);
    card.checklist.iter_mut().for_each(|i| f(&mut i.id));
}

/// Every Card of the Board, active then archived.
fn cards(board: &mut Board) -> impl Iterator<Item = &mut Card> {
    let active = board.columns.iter_mut().flat_map(|c| &mut c.cards);
    active.chain(board.archived_cards.iter_mut().map(|a| &mut a.card))
}
