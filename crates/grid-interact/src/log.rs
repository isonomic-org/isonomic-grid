// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_core::{Change, DataSource, Edit, SourceError};

/// The edits that take a batch of changes back, newest first.
fn invert(changes: &[Change]) -> Vec<Edit> {
    changes
        .iter()
        .rev()
        .filter_map(|change| match change {
            Change::Cell { row, column, old, .. } => {
                Some(Edit::SetCell { row: *row, column: column.clone(), value: old.clone() })
            }
            Change::Inserted { row } => Some(Edit::DeleteRow { row: *row }),
            Change::Deleted { row, values } => {
                Some(Edit::RestoreRow { row: *row, values: values.clone() })
            }
            Change::Reset => None,
        })
        .collect()
}

/// Undo and redo for edits made through one view. Each entry is what
/// one user action changed; undoing applies the inverse edits to the
/// source, so it works against any source that accepts edits.
#[derive(Clone, Debug, Default)]
pub struct EditLog {
    done: Vec<Vec<Change>>,
    undone: Vec<Vec<Change>>,
    limit: usize,
}

impl EditLog {
    pub fn new(limit: usize) -> Self {
        Self { done: Vec::new(), undone: Vec::new(), limit: limit.max(1) }
    }

    /// Applies one user action's edits and remembers how to undo it.
    pub fn apply(
        &mut self,
        source: &mut dyn DataSource,
        edits: &[Edit],
    ) -> Result<Vec<Change>, SourceError> {
        let changes = source.apply(edits)?;
        if !changes.is_empty() {
            self.done.push(changes.clone());
            self.undone.clear();
            if self.done.len() > self.limit {
                self.done.remove(0);
            }
        }
        Ok(changes)
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Undoes the last action. `Ok(None)` when there is none. On an
    /// error the action stays undoable.
    pub fn undo(
        &mut self,
        source: &mut dyn DataSource,
    ) -> Result<Option<Vec<Change>>, SourceError> {
        let Some(batch) = self.done.pop() else { return Ok(None) };
        match source.apply(&invert(&batch)) {
            Ok(changes) => {
                self.undone.push(changes.clone());
                Ok(Some(changes))
            }
            Err(error) => {
                self.done.push(batch);
                Err(error)
            }
        }
    }

    pub fn redo(
        &mut self,
        source: &mut dyn DataSource,
    ) -> Result<Option<Vec<Change>>, SourceError> {
        let Some(batch) = self.undone.pop() else { return Ok(None) };
        match source.apply(&invert(&batch)) {
            Ok(changes) => {
                self.done.push(changes.clone());
                Ok(Some(changes))
            }
            Err(error) => {
                self.undone.push(batch);
                Err(error)
            }
        }
    }
}
