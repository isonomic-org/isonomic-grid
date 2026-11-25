// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::rc::Rc;

use grid_core::{
    ColumnId, ColumnSpec, ColumnType, Direction, Edit, Expansion, RowId, SortKey, Table, Value,
    ViewQuery,
};
use grid_formats::{format_value, parse_value, write_delimited};
use grid_interact::{Cell, EditLog, Move, Selection};
use grid_layout::Pins;
use grid_query::{MemorySource, ViewResult, ViewRow};
use ui::{Context, EventEmitter};

/// A column's starting width, in rems.
const COLUMN_REMS: f32 = 10.0;
/// The narrowest a column can be dragged, in rems.
const MIN_COLUMN_REMS: f32 = 2.0;
const UNDO_STEPS: usize = 200;

/// What a [`GridState`] tells its observers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridEvent {
    /// The rows shown changed: an edit, or a new query.
    Changed,
    SelectionChanged,
}

/// A cell being edited: the text typed so far, and why the last
/// attempt to commit it was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
    pub cell: Cell,
    pub text: String,
    pub error: Option<String>,
}

/// One grid: its data, what it shows of it, and where the user is.
pub struct GridState {
    source: MemorySource,
    query: ViewQuery,
    /// Visible columns, as indices into the source's schema.
    columns: Vec<usize>,
    /// Width of each visible column, in rems.
    widths: Vec<f32>,
    selection: Selection,
    editing: Option<Editing>,
    log: EditLog,
    pub(crate) scroll: (f64, f64),
    pub(crate) row_pins: Pins,
    pub(crate) column_pins: Pins,
}

impl EventEmitter<GridEvent> for GridState {}

impl GridState {
    pub fn new(table: Table, _cx: &mut Context<Self>) -> Self {
        let columns: Vec<usize> = (0..table.specs().len()).collect();
        Self {
            widths: vec![COLUMN_REMS; columns.len()],
            columns,
            source: MemorySource::new(table),
            query: ViewQuery::default(),
            selection: Selection::at(Cell::default()),
            editing: None,
            log: EditLog::new(UNDO_STEPS),
            scroll: (0.0, 0.0),
            row_pins: Pins::default(),
            column_pins: Pins::default(),
        }
    }

    pub fn table(&self) -> &Table {
        self.source.table()
    }

    pub fn query(&self) -> &ViewQuery {
        &self.query
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn widths(&self) -> &[f32] {
        &self.widths
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// The rows to show, in order. Cached until data or query change.
    pub fn result(&mut self) -> Rc<ViewResult> {
        self.source.view(&self.query).unwrap_or_else(|_| {
            Rc::new(ViewResult { rows: Vec::new(), matched: 0, totals: Vec::new() })
        })
    }

    pub fn row_count(&mut self) -> usize {
        self.result().rows.len()
    }

    fn spec(&self, column: usize) -> Option<&ColumnSpec> {
        self.table().specs().get(*self.columns.get(column)?)
    }

    /// A column's title, with an arrow when the view sorts by it.
    pub fn header_text(&self, column: usize) -> String {
        let Some(spec) = self.spec(column) else { return String::new() };
        match self.query.sort.iter().find(|key| key.column == spec.id).map(|key| key.direction) {
            Some(Direction::Ascending) => format!("{} ↑", spec.title),
            Some(Direction::Descending) => format!("{} ↓", spec.title),
            None => spec.title.clone(),
        }
    }

    /// Whether a column's cells read better right-aligned.
    pub fn is_numeric(&self, column: usize) -> bool {
        self.spec(column).is_some_and(|spec| {
            spec.ty.is_numeric() || matches!(spec.ty, ColumnType::Date | ColumnType::DateTime)
        })
    }

    /// The text a cell shows. A group row shows its key and its row
    /// count in the first column.
    pub fn cell_text(&mut self, row: usize, column: usize) -> String {
        let result = self.result();
        match result.rows.get(row) {
            Some(ViewRow::Data { row, .. }) => {
                let Some(schema) = self.columns.get(column).copied() else { return String::new() };
                let value = self.table().get(*row as usize, schema);
                self.spec(column).map(|spec| format_value(&value, spec)).unwrap_or_default()
            }
            Some(ViewRow::Group { path, count, open, depth, .. }) if column == 0 => {
                let column =
                    self.query.group_by.get(*depth).and_then(|id| self.table().column_index(id));
                let spec = column.and_then(|index| self.table().specs().get(index));
                let key = match (path.last(), spec) {
                    (Some(Value::Null) | None, _) => "(empty)".to_owned(),
                    (Some(value), Some(spec)) => format_value(value, spec),
                    (Some(value), None) => format!("{value:?}"),
                };
                let arrow = if *open { "▾" } else { "▸" };
                format!("{}{arrow} {key} ({count})", "  ".repeat(*depth))
            }
            _ => String::new(),
        }
    }

    pub fn is_group(&mut self, row: usize) -> bool {
        matches!(self.result().rows.get(row), Some(ViewRow::Group { .. }))
    }

    pub fn row_id(&mut self, row: usize) -> Option<RowId> {
        match self.result().rows.get(row) {
            Some(ViewRow::Data { id, .. }) => Some(*id),
            _ => None,
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let (rows, columns) = (self.row_count(), self.column_count());
        let clamp = |cell: Cell| Cell {
            row: cell.row.min(rows.saturating_sub(1)),
            column: cell.column.min(columns.saturating_sub(1)),
        };
        if clamp(self.selection.active) != self.selection.active {
            self.selection = Selection::at(clamp(self.selection.active));
        }
        cx.emit(GridEvent::Changed);
        cx.notify();
    }

    /// Replaces the view query.
    pub fn set_query(&mut self, query: ViewQuery, cx: &mut Context<Self>) {
        self.query = query;
        self.changed(cx);
    }

    /// A header click: sort ascending, then descending, then not.
    pub fn sort_by(&mut self, column: usize, cx: &mut Context<Self>) {
        let Some(id) = self.spec(column).map(|spec| spec.id.clone()) else { return };
        let current = self.query.sort.iter().find(|key| key.column == id).map(|key| key.direction);
        self.query.sort = match current {
            None => vec![SortKey { column: id, direction: Direction::Ascending }],
            Some(Direction::Ascending) => {
                vec![SortKey { column: id, direction: Direction::Descending }]
            }
            Some(Direction::Descending) => Vec::new(),
        };
        self.changed(cx);
    }

    pub fn group_by(&mut self, columns: Vec<ColumnId>, cx: &mut Context<Self>) {
        self.query.group_by = columns;
        self.query.expanded = Expansion::All;
        self.changed(cx);
    }

    /// Opens or closes the group at a view row.
    pub fn toggle_group(&mut self, row: usize, cx: &mut Context<Self>) {
        let result = self.result();
        let Some(ViewRow::Group { path, open, .. }) = result.rows.get(row) else { return };
        let (path, open) = (path.clone(), *open);
        let without = |list: &[Vec<Value>]| list.iter().filter(|p| **p != path).cloned().collect();
        let with = |list: &[Vec<Value>]| list.iter().cloned().chain([path.clone()]).collect();
        self.query.expanded = match (&self.query.expanded, open) {
            (Expansion::All, true) => Expansion::AllExcept(vec![path.clone()]),
            (Expansion::AllExcept(closed), true) => Expansion::AllExcept(with(closed)),
            (Expansion::AllExcept(closed), false) => Expansion::AllExcept(without(closed)),
            (Expansion::None, false) => Expansion::Only(vec![path.clone()]),
            (Expansion::Only(opened), false) => Expansion::Only(with(opened)),
            (Expansion::Only(opened), true) => Expansion::Only(without(opened)),
            (other, _) => other.clone(),
        };
        self.changed(cx);
    }

    /// Moves or extends the selection with the keyboard and returns
    /// the cell to bring into view.
    pub fn navigate(
        &mut self,
        step: Move,
        extend: bool,
        page: usize,
        cx: &mut Context<Self>,
    ) -> Cell {
        let (rows, columns) = (self.row_count(), self.column_count());
        let to = self.selection.navigate(step, rows, columns, page, extend);
        cx.emit(GridEvent::SelectionChanged);
        cx.notify();
        to
    }

    pub fn click(&mut self, cell: Cell, extend: bool, add: bool, cx: &mut Context<Self>) {
        self.selection.click(cell, extend, add);
        cx.emit(GridEvent::SelectionChanged);
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let (rows, columns) = (self.row_count(), self.column_count());
        self.selection.select_all(rows, columns);
        cx.emit(GridEvent::SelectionChanged);
        cx.notify();
    }

    /// Sets a visible column's width in rems, no narrower than the
    /// minimum. Unknown columns and widths that are not numbers are
    /// ignored.
    pub fn set_width(&mut self, column: usize, rems: f32, cx: &mut Context<Self>) {
        let Some(width) = self.widths.get_mut(column) else { return };
        if !rems.is_finite() {
            return;
        }
        let rems = rems.max(MIN_COLUMN_REMS);
        if *width != rems {
            *width = rems;
            cx.notify();
        }
    }

    pub fn scroll_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) {
        let to = (x.max(0.0), y.max(0.0));
        if self.scroll != to {
            self.scroll = to;
            cx.notify();
        }
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
        let edit = Edit::SetCell { row: id, column: spec.id, value };
        self.log.apply(&mut self.source, &[edit]).map_err(|error| error.to_string())?;
        self.changed(cx);
        Ok(())
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
        self.editing = Some(Editing { cell, text, error: None });
        cx.notify();
    }

    /// Types into the cell being edited.
    pub fn edit_insert(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(editing) = &mut self.editing {
            editing.text.push_str(text);
            editing.error = None;
            cx.notify();
        }
    }

    /// Removes the last character typed.
    pub fn edit_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(editing) = &mut self.editing {
            editing.text.pop();
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
        match self.set_cell_text(row, column, &editing.text, cx) {
            Ok(()) => true,
            Err(error) => {
                self.editing = Some(Editing { error: Some(error), ..editing });
                cx.notify();
                false
            }
        }
    }

    /// Empties the active cell.
    pub fn clear_cell(&mut self, cx: &mut Context<Self>) {
        let cell = self.selection.active;
        let _ = self.set_cell_text(cell.row, cell.column, "", cx);
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if matches!(self.log.undo(&mut self.source), Ok(Some(_))) {
            self.changed(cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if matches!(self.log.redo(&mut self.source), Ok(Some(_))) {
            self.changed(cx);
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
}
