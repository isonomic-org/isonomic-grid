// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Changing cells: the editor in the active cell, clearing, and the
//! clipboard. Every change goes through the edit log as one step, so
//! one undo takes back one paste or one cleared range.

use grid_core::{ColumnSpec, Edit, RowId, Value};
use grid_formats::{parse_value, paste_edits, read_delimited, write_delimited};
use grid_interact::{Cell, CellRange, LineEdit};
use ui::Context;

use crate::state::{GridEvent, GridState};

/// The most cells one paste or one clear may change.
const MOST_CELLS: usize = 200_000;

/// A cell being edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
    pub cell: Cell,
    pub line: LineEdit,
    /// Why the last attempt to commit was refused.
    pub error: Option<String>,
    /// Whether the edit began from the cell's own text. Then the arrow
    /// keys move the caret; in an edit begun by typing they confirm
    /// and move to the next cell.
    pub amend: bool,
}

impl Editing {
    pub fn text(&self) -> &str {
        self.line.text()
    }
}

impl GridState {
    fn refuse(&mut self, reason: String, cx: &mut Context<Self>) {
        cx.emit(GridEvent::Refused(reason));
    }

    /// Applies edits as one undo step.
    fn apply(&mut self, edits: &[Edit], cx: &mut Context<Self>) -> Result<(), String> {
        if edits.is_empty() {
            return Ok(());
        }
        self.log.apply(&mut self.source, edits).map_err(|error| error.to_string())?;
        self.changed(cx);
        Ok(())
    }

    /// Sets a cell from typed text. An error says why the text is not
    /// a value of the column's type, or why the source refused it.
    pub fn set_cell_text(
        &mut self,
        row: usize,
        column: usize,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let id = self.row_id(row).ok_or("not a data row")?;
        let spec = self.spec(column).ok_or("no such column")?.clone();
        let value = parse_value(text, &spec).map_err(|error| error.to_string())?;
        self.apply(&[Edit::SetCell { row: id, column: spec.id, value }], cx)
    }

    pub fn editing(&self) -> Option<&Editing> {
        self.editing.as_ref()
    }

    /// Starts editing the active cell. `replace` is what the user
    /// typed to start; without it the cell's text is kept to amend.
    pub fn begin_edit(&mut self, replace: Option<&str>, cx: &mut Context<Self>) {
        let cell = self.selection.active;
        let editable = self.spec(cell.column).is_some_and(|spec| spec.editable);
        if self.editing.is_some() || !editable || self.row_id(cell.row).is_none() {
            return;
        }
        let text = replace.map_or_else(|| self.cell_text(cell.row, cell.column), str::to_owned);
        let line = LineEdit::new(text);
        self.editing = Some(Editing { cell, line, error: None, amend: replace.is_none() });
        cx.notify();
    }

    /// Changes the line being edited. Any change clears the last
    /// refusal.
    pub fn edit_line(&mut self, change: impl FnOnce(&mut LineEdit), cx: &mut Context<Self>) {
        if let Some(editing) = &mut self.editing {
            change(&mut editing.line);
            editing.error = None;
            cx.notify();
        }
    }

    pub fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            cx.notify();
        }
    }

    /// Writes the edited text to its cell. On a refusal the editor
    /// stays open and carries the reason. `true` when nothing is
    /// left being edited.
    pub fn commit_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(editing) = self.editing.take() else { return true };
        let Cell { row, column } = editing.cell;
        match self.set_cell_text(row, column, editing.text(), cx) {
            Ok(()) => true,
            Err(error) => {
                self.editing = Some(Editing { error: Some(error), ..editing });
                cx.notify();
                false
            }
        }
    }

    /// The data rows of a range, and its columns.
    fn targets(&mut self, range: CellRange) -> (Vec<RowId>, Vec<ColumnSpec>) {
        let ((top, bottom), (left, right)) = (range.rows(), range.columns());
        let bottom = bottom.min(self.row_count().saturating_sub(1));
        let rows = (top..=bottom).filter_map(|row| self.row_id(row)).collect();
        let columns = (left..=right).filter_map(|column| self.spec(column).cloned()).collect();
        (rows, columns)
    }

    /// Empties every editable cell in the selection, as one step.
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        let ranges = self.selection.ranges.clone();
        let cells: usize = ranges.iter().map(CellRange::cell_count).sum();
        if cells > MOST_CELLS {
            return self.refuse(format!("{cells} cells is too many to clear at once"), cx);
        }
        let mut edits = Vec::new();
        for range in ranges {
            let (rows, columns) = self.targets(range);
            for row in &rows {
                for spec in columns.iter().filter(|spec| spec.editable) {
                    let (row, column) = (*row, spec.id.clone());
                    edits.push(Edit::SetCell { row, column, value: Value::Null });
                }
            }
        }
        if let Err(reason) = self.apply(&edits, cx) {
            self.refuse(reason, cx);
        }
    }

    /// The last selected range as tab-separated text.
    pub fn copy_text(&mut self) -> String {
        let Some(range) = self.selection.ranges.last().copied() else { return String::new() };
        let ((top, bottom), (left, right)) = (range.rows(), range.columns());
        let bottom = bottom.min(self.row_count().saturating_sub(1));
        let mut lines = Vec::new();
        for row in top..=bottom {
            lines
                .push((left..=right).map(|column| self.cell_text(row, column)).collect::<Vec<_>>());
        }
        write_delimited(lines, '\t')
    }

    /// Pastes tab-separated text with its top-left field on the active
    /// cell. One field pasted over a larger selection fills it. Rows
    /// past the last one shown become new rows. Text a column cannot
    /// hold refuses the whole paste.
    pub fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let mut fields = read_delimited(text, '\t');
        if fields.last().is_some_and(|line| line.iter().all(String::is_empty)) {
            fields.pop();
        }
        let Some(range) = self.selection.ranges.last().copied() else { return };
        let active = self.selection.active;
        let single = fields.len() == 1 && fields[0].len() == 1;
        let (range, fill) = if single && range.cell_count() > 1 {
            (range, true)
        } else {
            let wide = fields.iter().map(Vec::len).max().unwrap_or(0);
            let last = Cell {
                row: active.row + fields.len().saturating_sub(1),
                column: active.column + wide.saturating_sub(1),
            };
            (CellRange { anchor: active, head: last }, false)
        };
        if fields.is_empty() {
            return;
        }
        if range.cell_count() > MOST_CELLS {
            return self.refuse(format!("{} cells is too many to paste", range.cell_count()), cx);
        }
        let (mut rows, columns) = self.targets(range);
        if fill {
            fields = vec![vec![fields[0][0].clone(); columns.len()]; rows.len()];
        } else {
            // Group rows in the way are stepped over: each pasted line
            // lands on the next data row for as long as there is one.
            let mut next = range.rows().1 + 1;
            let count = self.row_count();
            while rows.len() < fields.len() && next < count {
                rows.extend(self.row_id(next));
                next += 1;
            }
        }
        let all = self.table().specs().to_vec();
        let pasted = paste_edits(&fields, &rows, &columns, &all)
            .map_err(|error| error.to_string())
            .and_then(|edits| self.apply(&edits, cx));
        match pasted {
            Ok(()) => {
                let most = self.row_count().saturating_sub(1);
                let head = Cell {
                    row: range.rows().1.min(most),
                    column: range.columns().1.min(self.column_count().saturating_sub(1)),
                };
                let anchor = Cell { row: range.rows().0, column: range.columns().0 };
                self.selection.click(anchor, false, false);
                self.selection.click(head, true, false);
                cx.emit(GridEvent::SelectionChanged);
                cx.notify();
            }
            Err(reason) => self.refuse(reason, cx),
        }
    }
}
