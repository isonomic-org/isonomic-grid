// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use grid_core::RowId;

/// Selected rows, by ID.
///
/// Selecting everything in a windowed source cannot list every row,
/// so it is stored the other way round: everything that matches the
/// view, except the rows unticked since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowSelection {
    Only(BTreeSet<RowId>),
    AllExcept(BTreeSet<RowId>),
}

impl Default for RowSelection {
    fn default() -> Self {
        Self::Only(BTreeSet::new())
    }
}

impl RowSelection {
    pub fn contains(&self, row: RowId) -> bool {
        match self {
            Self::Only(rows) => rows.contains(&row),
            Self::AllExcept(rows) => !rows.contains(&row),
        }
    }

    pub fn set(&mut self, row: RowId, selected: bool) {
        match self {
            Self::Only(rows) if selected => drop(rows.insert(row)),
            Self::Only(rows) => drop(rows.remove(&row)),
            Self::AllExcept(rows) if selected => drop(rows.remove(&row)),
            Self::AllExcept(rows) => drop(rows.insert(row)),
        }
    }

    pub fn toggle(&mut self, row: RowId) {
        self.set(row, !self.contains(row));
    }

    pub fn select_all(&mut self) {
        *self = Self::AllExcept(BTreeSet::new());
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// How many rows are selected, given how many the view matches.
    pub fn count(&self, matched: usize) -> usize {
        match self {
            Self::Only(rows) => rows.len(),
            Self::AllExcept(rows) => matched.saturating_sub(rows.len()),
        }
    }

    pub fn is_empty(&self, matched: usize) -> bool {
        self.count(matched) == 0
    }
}
