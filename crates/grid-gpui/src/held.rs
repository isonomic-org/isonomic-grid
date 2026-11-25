// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The selection, held by what it is on. The keyboard and the pointer
//! work in positions; a sort, a filter or an edit moves rows to other
//! positions. So after every change to the selection the state notes
//! the row and column IDs under it, and after every change to the rows
//! or columns it puts the selection back on them.

use std::collections::HashMap;

use grid_core::{ColumnId, RowId};
use grid_interact::{Cell, CellRange, Selection};
use grid_query::ViewRow;

use crate::state::GridState;

/// A cell by its row's ID and its column's ID. Where it was is kept
/// for rows that have no ID, such as a group's heading.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    row: Option<RowId>,
    column: Option<ColumnId>,
    at: Cell,
}

/// What the selection was last on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Held {
    active: Option<Key>,
    ranges: Vec<(Key, Key)>,
    /// Everything is selected, whatever the rows turn out to be.
    pub(crate) all: bool,
}

impl GridState {
    fn key(&mut self, at: Cell) -> Key {
        let column = self.spec(at.column).map(|spec| spec.id.clone());
        Key { row: self.row_id(at.row), column, at }
    }

    /// Notes what the selection is on. Call after every change to it.
    pub(crate) fn hold_selection(&mut self) {
        let selection = self.selection.clone();
        let mut ranges = Vec::with_capacity(selection.ranges.len());
        for range in &selection.ranges {
            ranges.push((self.key(range.anchor), self.key(range.head)));
        }
        self.held = Held { active: Some(self.key(selection.active)), ranges, all: false };
    }

    /// Puts the selection back on the rows and columns it was on. A
    /// range keeps the corners it has left; a selection whose rows are
    /// all gone falls back to the cell now at the old position. Call
    /// after the rows or the columns change.
    pub(crate) fn restore_selection(&mut self) {
        let held = std::mem::take(&mut self.held);
        let (rows, columns) = (self.row_count(), self.column_count());
        let last = Cell { row: rows.saturating_sub(1), column: columns.saturating_sub(1) };
        let keys = || held.active.iter().chain(held.ranges.iter().flat_map(|(a, b)| [a, b]));

        // One pass over the rows finds every row the selection names.
        let mut wanted: HashMap<RowId, Option<usize>> =
            keys().filter_map(|key| Some((key.row?, None))).collect();
        let mut missing = wanted.len();
        let result = self.result();
        for (position, row) in result.rows.iter().enumerate() {
            if missing == 0 {
                break;
            }
            if let ViewRow::Data { id, .. } = row
                && let Some(found @ None) = wanted.get_mut(id)
            {
                *found = Some(position);
                missing -= 1;
            }
        }
        let specs = self.table().specs();
        let shown: Vec<&ColumnId> =
            self.columns.iter().filter_map(|index| Some(&specs.get(*index)?.id)).collect();
        let place = |key: &Key| -> Option<Cell> {
            let row = match key.row {
                Some(id) => wanted.get(&id).copied().flatten()?,
                None => key.at.row.min(last.row),
            };
            let column = match &key.column {
                Some(id) => shown.iter().position(|shown| *shown == id)?,
                None => key.at.column.min(last.column),
            };
            Some(Cell { row, column })
        };
        let clamped = |key: &Key| Cell {
            row: key.at.row.min(last.row),
            column: key.at.column.min(last.column),
        };

        let active = held
            .active
            .as_ref()
            .map_or(Cell::default(), |key| place(key).unwrap_or_else(|| clamped(key)));
        let mut ranges: Vec<CellRange> = held
            .ranges
            .iter()
            .filter_map(|(anchor, head)| match (place(anchor), place(head)) {
                (Some(anchor), Some(head)) => Some(CellRange { anchor, head }),
                (Some(cell), None) | (None, Some(cell)) => Some(CellRange::single(cell)),
                (None, None) => None,
            })
            .collect();
        if ranges.is_empty() {
            ranges.push(CellRange::single(active));
        }
        self.selection = Selection { active, ranges };
        if held.all {
            self.selection.select_all(rows, columns);
        }
        self.hold_selection();
        self.held.all = held.all;
    }
}
