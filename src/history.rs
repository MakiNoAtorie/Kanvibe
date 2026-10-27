//! One undo history across all Boards. Each step is a snapshot of the Boards that shares
//! unchanged Boards with its neighbour (copy-on-write), so a step costs only the Boards it touched.
//! Only Boards are snapshotted: the ID counter, settings and UI state are not.
use std::sync::Arc;

use crate::domain::{Board, Id};

type Snapshot = Vec<Arc<Board>>;

/// Most undo steps kept; recording one more drops the oldest.
const LIMIT: usize = 100;

#[derive(Debug, Default)]
pub struct History {
    /// The Boards as of the last recorded step; `None` until first synced.
    present: Option<Snapshot>,
    undo: Vec<Step>,
    redo: Vec<Step>,
}

#[derive(Debug)]
struct Step {
    /// The Boards on the other side of this step.
    boards: Snapshot,
    /// The Board to show after undo / after redo; `None` keeps the current Board.
    show_undo: Option<Id>,
    show_redo: Option<Id>,
}

impl History {
    /// Takes the given Boards as the starting point if nothing is recorded yet (e.g. after load).
    pub fn sync(&mut self, boards: &[Board]) {
        if self.present.is_none() {
            self.present = Some(share(boards, &[]));
        }
    }

    /// Records a step if `boards` differ from the last recorded state.
    pub fn record(&mut self, boards: &[Board], show_undo: Option<Id>, show_redo: Option<Id>) {
        let present = self.present.take().unwrap_or_default();
        let next = share(boards, &present);
        if next.len() == present.len() && next.iter().zip(&present).all(|(a, b)| Arc::ptr_eq(a, b))
        {
            self.present = Some(present);
            return;
        }
        self.undo.push(Step {
            boards: present,
            show_undo,
            show_redo,
        });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.present = Some(next);
    }

    /// Steps back: returns the Boards to restore and the Board to show, if any.
    pub fn undo(&mut self) -> Option<(Vec<Board>, Option<Id>)> {
        let step = self.undo.pop()?;
        let show = step.show_undo;
        let restored = self.swap(step, |s, step| s.redo.push(step));
        Some((restored, show))
    }

    /// Steps forward again: returns the Boards to restore and the Board to show, if any.
    pub fn redo(&mut self) -> Option<(Vec<Board>, Option<Id>)> {
        let step = self.redo.pop()?;
        let show = step.show_redo;
        let restored = self.swap(step, |s, step| s.undo.push(step));
        Some((restored, show))
    }

    /// Makes `step.boards` the present and pushes the old present as the reverse step.
    fn swap(&mut self, mut step: Step, push: impl FnOnce(&mut Self, Step)) -> Vec<Board> {
        let present = self.present.take().unwrap_or_default();
        let restored = std::mem::replace(&mut step.boards, present);
        let boards = restored.iter().map(|b| (**b).clone()).collect();
        self.present = Some(restored);
        push(self, step);
        boards
    }
}

/// Snapshots `boards`, reusing the `previous` Arc for every Board that did not change.
fn share(boards: &[Board], previous: &[Arc<Board>]) -> Snapshot {
    boards
        .iter()
        .map(|board| match previous.iter().find(|p| p.id == board.id) {
            Some(p) if **p == *board => Arc::clone(p),
            _ => Arc::new(board.clone()),
        })
        .collect()
}
