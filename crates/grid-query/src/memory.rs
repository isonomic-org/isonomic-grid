use std::rc::Rc;

use grid_core::{
    Capabilities, Change, ColumnSpec, Count, DataSource, Edit, Expansion, FetchRequest, Page,
    PageRequest, PageRow, SourceError, Table, ViewQuery,
};

use crate::{ViewResult, ViewRow, execute};

/// A data source over an in-memory table. It does everything itself.
pub struct MemorySource {
    table: Table,
    cache: Option<(u64, ViewQuery, Rc<ViewResult>)>,
    pending: Vec<Change>,
}

impl MemorySource {
    pub fn new(table: Table) -> Self {
        Self { table, cache: None, pending: Vec::new() }
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    /// The whole result for a query, as shown: closed groups hide
    /// their rows. Cached until the table or the query changes.
    pub fn view(&mut self, query: &ViewQuery) -> Result<Rc<ViewResult>, SourceError> {
        if let Some((version, cached, result)) = &self.cache
            && *version == self.table.version()
            && cached == query
        {
            return Ok(result.clone());
        }
        let result = Rc::new(execute(&self.table, query)?);
        self.cache = Some((self.table.version(), query.clone(), result.clone()));
        Ok(result)
    }

    fn apply_one(&mut self, edit: &Edit) -> Result<Change, SourceError> {
        match edit {
            Edit::SetCell { row, column, value } => {
                let index = self.table.column_index(column).ok_or(SourceError::UnknownColumn)?;
                let old = self.table.set(*row, index, value)?;
                Ok(Change::Cell { row: *row, column: column.clone(), old, new: value.clone() })
            }
            Edit::InsertRow { values } => Ok(Change::Inserted { row: self.table.push(values)? }),
            Edit::DeleteRow { row } => {
                Ok(Change::Deleted { row: *row, values: self.table.remove(*row)? })
            }
            Edit::RestoreRow { row, values } => {
                self.table.restore(*row, values)?;
                Ok(Change::Inserted { row: *row })
            }
        }
    }

    fn revert(&mut self, change: &Change) {
        let undo = match change {
            Change::Cell { row, column, old, .. } => {
                Edit::SetCell { row: *row, column: column.clone(), value: old.clone() }
            }
            Change::Inserted { row } => Edit::DeleteRow { row: *row },
            Change::Deleted { row, values } => {
                Edit::RestoreRow { row: *row, values: values.clone() }
            }
            Change::Reset => return,
        };
        let _ = self.apply_one(&undo);
    }
}

impl DataSource for MemorySource {
    fn schema(&self) -> &[ColumnSpec] {
        self.table.specs()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            filter: true,
            search: true,
            sort: true,
            group: true,
            aggregate: true,
            edit: true,
            exact_count: true,
        }
    }

    fn fetch(&mut self, request: &FetchRequest) -> Result<Page, SourceError> {
        // A fetch names its group, so it reads the full tree.
        let mut query = request.query.clone();
        query.expanded = Expansion::All;
        let result = self.view(&query)?;
        let depth = request.group_path.len();
        if depth > query.group_by.len() {
            return Err(SourceError::Unsupported("a group path deeper than the grouping"));
        }
        let groups = query.group_by.len();
        let mut current: &[grid_core::Value] = &[];
        let mut level: Vec<&ViewRow> = Vec::new();
        for row in &result.rows {
            match row {
                ViewRow::Group { depth: at, path, .. } => {
                    current = path;
                    if *at == depth && path[..depth] == request.group_path[..] {
                        level.push(row);
                    }
                }
                ViewRow::Data { .. } => {
                    if depth == groups && current == request.group_path.as_slice() {
                        level.push(row);
                    }
                }
            }
        }
        let at = |id| {
            level
                .iter()
                .position(|row| matches!(row, ViewRow::Data { id: found, .. } if *found == id))
        };
        let range = match &request.page {
            PageRequest::Range(range) => range.start.min(level.len())..range.end.min(level.len()),
            PageRequest::After { row, limit } => {
                let start = at(*row).ok_or(SourceError::UnknownRow(*row))? + 1;
                start..(start + limit).min(level.len())
            }
            PageRequest::Before { row, limit } => {
                let end = at(*row).ok_or(SourceError::UnknownRow(*row))?;
                end.saturating_sub(*limit)..end
            }
        };
        let mut wanted = Vec::new();
        for id in &query.columns {
            wanted.push(self.table.column_index(id).ok_or(SourceError::UnknownColumn)?);
        }
        let rows = level[range]
            .iter()
            .map(|row| match row {
                ViewRow::Group { path, count, aggregates, .. } => PageRow::Group {
                    key: path.last().cloned().unwrap_or(grid_core::Value::Null),
                    count: *count,
                    aggregates: aggregates.clone(),
                },
                ViewRow::Data { row, id, .. } => {
                    let all = self.table.row(*row as usize);
                    let values = if wanted.is_empty() {
                        all
                    } else {
                        wanted.iter().map(|column| all[*column].clone()).collect()
                    };
                    PageRow::Data { id: *id, values }
                }
            })
            .collect();
        Ok(Page { rows, count: Count::Exact(level.len()) })
    }

    fn apply(&mut self, edits: &[Edit]) -> Result<Vec<Change>, SourceError> {
        let mut changes = Vec::with_capacity(edits.len());
        for edit in edits {
            match self.apply_one(edit) {
                Ok(change) => changes.push(change),
                Err(error) => {
                    for change in changes.iter().rev() {
                        self.revert(change);
                    }
                    return Err(error);
                }
            }
        }
        self.pending.extend(changes.iter().cloned());
        Ok(changes)
    }

    fn changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.pending)
    }
}
