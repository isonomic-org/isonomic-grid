// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Which columns show, in what order, and what is pinned. Positions
//! here count the columns shown, left to right.

use grid_core::ColumnId;
use grid_layout::Pins;
use ui::Context;

use crate::state::{GridEvent, GridState};

impl GridState {
    fn columns_changed(&mut self, cx: &mut Context<Self>) {
        let count = self.columns.len();
        self.column_pins.leading = self.column_pins.leading.min(count);
        self.column_pins.trailing = self.column_pins.trailing.min(count - self.column_pins.leading);
        // The selection is held by column ID, so it goes with its columns.
        self.restore_selection();
        if let Some(editing) = &mut self.editing {
            editing.cell = self.selection.active;
        }
        cx.emit(GridEvent::ColumnsChanged);
        cx.notify();
    }

    /// The columns shown, in order.
    pub fn shown_columns(&self) -> Vec<ColumnId> {
        let specs = self.table().specs();
        self.columns.iter().filter_map(|index| Some(specs.get(*index)?.id.clone())).collect()
    }

    /// Moves a shown column to another place among them.
    pub fn move_column(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.columns.len();
        if from >= count || to >= count || from == to {
            return;
        }
        let column = self.columns.remove(from);
        self.columns.insert(to, column);
        let width = self.widths.remove(from);
        self.widths.insert(to, width);
        self.columns_changed(cx);
    }

    /// Hides a shown column. The last one stays.
    pub fn hide_column(&mut self, column: usize, cx: &mut Context<Self>) {
        if column >= self.columns.len() || self.columns.len() == 1 {
            return;
        }
        self.columns.remove(column);
        self.widths.remove(column);
        self.columns_changed(cx);
    }

    /// Shows a hidden column at a place among the shown ones, at its
    /// starting width. Nothing happens for a column already shown or
    /// one the table does not have.
    pub fn show_column(&mut self, id: &ColumnId, at: usize, cx: &mut Context<Self>) {
        let Some(index) = self.table().column_index(id) else { return };
        if self.columns.contains(&index) {
            return;
        }
        let at = at.min(self.columns.len());
        self.columns.insert(at, index);
        self.widths.insert(at, crate::state::COLUMN_REMS);
        self.columns_changed(cx);
    }

    /// How many columns and rows are pinned at each end.
    pub fn pins(&self) -> (Pins, Pins) {
        (self.column_pins, self.row_pins)
    }

    /// Pins columns at the left and the right. More than there are
    /// columns pins them all, left first.
    pub fn pin_columns(&mut self, leading: usize, trailing: usize, cx: &mut Context<Self>) {
        let count = self.columns.len();
        let leading = leading.min(count);
        self.column_pins = Pins { leading, trailing: trailing.min(count - leading) };
        cx.notify();
    }

    /// Pins rows at the top and the bottom of the rows now shown.
    pub fn pin_rows(&mut self, leading: usize, trailing: usize, cx: &mut Context<Self>) {
        let count = self.row_count();
        let leading = leading.min(count);
        self.row_pins = Pins { leading, trailing: trailing.min(count - leading) };
        cx.notify();
    }
}
