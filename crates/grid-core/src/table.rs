use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Column, ColumnId, ColumnSpec, SourceError, Value};

/// A row's identity. Stable for the life of the table and never
/// reused, so selection, edits and expansion survive sorting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RowId(pub u64);

/// Bounds on an in-memory table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub rows: usize,
    pub columns: usize,
    pub cell_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { rows: 10_000_000, columns: 2_000, cell_bytes: 1 << 20 }
    }
}

/// A columnar table in memory: one typed vector per column, with a
/// row-ID vector beside them.
#[derive(Clone, Debug)]
pub struct Table {
    specs: Vec<ColumnSpec>,
    columns: Vec<Column>,
    ids: Vec<RowId>,
    index: HashMap<RowId, usize>,
    next_id: u64,
    limits: Limits,
    version: u64,
}

impl Table {
    pub fn new(specs: Vec<ColumnSpec>) -> Self {
        Self::with_limits(specs, Limits::default())
    }

    pub fn with_limits(mut specs: Vec<ColumnSpec>, limits: Limits) -> Self {
        specs.truncate(limits.columns);
        let columns = specs.iter().map(|spec| Column::new(spec.ty)).collect();
        Self {
            specs,
            columns,
            ids: Vec::new(),
            index: HashMap::new(),
            next_id: 1,
            limits,
            version: 0,
        }
    }

    pub fn specs(&self) -> &[ColumnSpec] {
        &self.specs
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Grows with every change; a cached result is valid while this
    /// stays the same.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn column_index(&self, id: &ColumnId) -> Option<usize> {
        self.specs.iter().position(|spec| spec.id == *id)
    }

    pub fn column(&self, index: usize) -> Option<(&ColumnSpec, &Column)> {
        Some((self.specs.get(index)?, self.columns.get(index)?))
    }

    pub fn row_id(&self, row: usize) -> Option<RowId> {
        self.ids.get(row).copied()
    }

    pub fn row_ids(&self) -> &[RowId] {
        &self.ids
    }

    pub fn row_of(&self, id: RowId) -> Option<usize> {
        self.index.get(&id).copied()
    }

    pub fn get(&self, row: usize, column: usize) -> Value {
        match self.column(column) {
            Some((spec, data)) if row < self.len() => data.get(spec.ty, row),
            _ => Value::Null,
        }
    }

    pub fn row(&self, row: usize) -> Vec<Value> {
        (0..self.specs.len()).map(|column| self.get(row, column)).collect()
    }

    fn check(&self, column: usize, value: &Value) -> Result<(), SourceError> {
        let spec = self.specs.get(column).ok_or(SourceError::UnknownColumn)?;
        if !spec.ty.accepts(value) {
            return Err(SourceError::WrongType(spec.id.clone()));
        }
        let bytes = match value {
            Value::Bytes(bytes) => bytes.len(),
            other => other.as_str().map_or(0, str::len),
        };
        if bytes > self.limits.cell_bytes { Err(SourceError::Limit("cell size")) } else { Ok(()) }
    }

    /// Appends a row. Missing trailing values are null.
    pub fn push(&mut self, values: &[Value]) -> Result<RowId, SourceError> {
        if self.len() >= self.limits.rows {
            return Err(SourceError::Limit("rows"));
        }
        if values.len() > self.specs.len() {
            return Err(SourceError::UnknownColumn);
        }
        for (column, value) in values.iter().enumerate() {
            self.check(column, value)?;
        }
        let row = self.len();
        for (column, data) in self.columns.iter_mut().enumerate() {
            data.put(row, values.get(column).unwrap_or(&Value::Null));
        }
        let id = RowId(self.next_id);
        self.next_id += 1;
        self.ids.push(id);
        self.index.insert(id, row);
        self.version += 1;
        Ok(id)
    }

    /// Sets one cell and returns the value it replaced.
    pub fn set(&mut self, id: RowId, column: usize, value: &Value) -> Result<Value, SourceError> {
        let row = self.row_of(id).ok_or(SourceError::UnknownRow(id))?;
        self.check(column, value)?;
        let old = self.get(row, column);
        self.columns[column].put(row, value);
        self.version += 1;
        Ok(old)
    }

    /// Removes a row and returns its values.
    pub fn remove(&mut self, id: RowId) -> Result<Vec<Value>, SourceError> {
        let row = self.row_of(id).ok_or(SourceError::UnknownRow(id))?;
        let values = self.row(row);
        for data in &mut self.columns {
            data.remove(row);
        }
        self.ids.remove(row);
        self.index.remove(&id);
        for (at, later) in self.ids.iter().enumerate().skip(row) {
            self.index.insert(*later, at);
        }
        self.version += 1;
        Ok(values)
    }

    /// Puts back a removed row under its old ID, at the end.
    pub fn restore(&mut self, id: RowId, values: &[Value]) -> Result<(), SourceError> {
        if self.index.contains_key(&id) {
            return Err(SourceError::Conflict);
        }
        let fresh = self.push(values)?;
        let row = self.len() - 1;
        self.ids[row] = id;
        self.index.remove(&fresh);
        self.index.insert(id, row);
        self.next_id = self.next_id.max(id.0 + 1);
        Ok(())
    }
}
