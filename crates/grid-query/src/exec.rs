// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_core::{Direction, RowId, SourceError, Table, Value, ViewQuery};

use crate::keys::{Key, keys};
use crate::{Bitmap, aggregate, matching, search};

/// One row of a flattened result.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewRow {
    Group { depth: usize, path: Vec<Value>, count: usize, aggregates: Vec<Value>, open: bool },
    Data { depth: usize, row: u32, id: RowId },
}

impl ViewRow {
    pub fn depth(&self) -> usize {
        match self {
            Self::Group { depth, .. } | Self::Data { depth, .. } => *depth,
        }
    }
}

/// A query's result: the rows to show, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewResult {
    pub rows: Vec<ViewRow>,
    /// How many data rows passed the filter and the search.
    pub matched: usize,
    /// The aggregates over every matched row.
    pub totals: Vec<Value>,
}

struct Plan<'a> {
    table: &'a Table,
    query: &'a ViewQuery,
    /// Matched rows in final order.
    rows: Vec<u32>,
    /// One key vector per grouping level, in the same order.
    group_keys: Vec<Vec<Key>>,
    group_columns: Vec<usize>,
    aggregates: Vec<(usize, grid_core::AggregateFn)>,
}

impl Plan<'_> {
    fn totals(&self, rows: &[u32]) -> Vec<Value> {
        self.aggregates
            .iter()
            .map(|(column, function)| aggregate(self.table, *column, *function, rows))
            .collect()
    }

    fn emit(
        &self,
        level: usize,
        from: usize,
        to: usize,
        path: &mut Vec<Value>,
        out: &mut Vec<ViewRow>,
    ) {
        if level == self.group_columns.len() {
            for row in &self.rows[from..to] {
                let id = self.table.row_id(*row as usize).unwrap_or(RowId(0));
                out.push(ViewRow::Data { depth: level, row: *row, id });
            }
            return;
        }
        let level_keys = &self.group_keys[level];
        let mut start = from;
        while start < to {
            let mut end = start + 1;
            while end < to && level_keys[end] == level_keys[start] {
                end += 1;
            }
            path.push(self.table.get(self.rows[start] as usize, self.group_columns[level]));
            let open = self.query.expanded.is_open(path);
            out.push(ViewRow::Group {
                depth: level,
                path: path.clone(),
                count: end - start,
                aggregates: self.totals(&self.rows[start..end]),
                open,
            });
            if open {
                self.emit(level + 1, start, end, path, out);
            }
            path.pop();
            start = end;
        }
    }
}

/// Runs a query against a table.
pub fn execute(table: &Table, query: &ViewQuery) -> Result<ViewResult, SourceError> {
    let column = |id| table.column_index(id).ok_or(SourceError::UnknownColumn);
    let mut keep = match &query.filter {
        Some(filter) => matching(table, filter)?,
        None => Bitmap::new(table.len(), true),
    };
    if !query.search.trim().is_empty() {
        let mut searched = Vec::new();
        for id in &query.columns {
            searched.push(column(id)?);
        }
        if searched.is_empty() {
            searched = (0..table.specs().len()).collect();
        }
        keep.and(&search(table, &query.search, &searched));
    }
    let rows = keep.ones();
    let mut group_columns = Vec::new();
    for id in &query.group_by {
        group_columns.push(column(id)?);
    }
    let mut all_keys: Vec<Vec<Key>> = group_columns
        .iter()
        .map(|index| keys(table, *index, &rows, Direction::Ascending))
        .collect();
    for sort in &query.sort {
        all_keys.push(keys(table, column(&sort.column)?, &rows, sort.direction));
    }
    let ids = table.row_ids();
    // Ties break by row ID. Sorting is stable and runs from the
    // least significant key to the most, so the starting order is the
    // final tie-break: row IDs, which table order already follows
    // unless a deleted row was put back.
    let mut order: Vec<u32> = (0..rows.len() as u32).collect();
    if !all_keys.is_empty() {
        let id_of = |at: u32| ids[rows[at as usize] as usize];
        if !rows.windows(2).all(|pair| ids[pair[0] as usize] < ids[pair[1] as usize]) {
            order.sort_by_key(|at| id_of(*at));
        }
        for keys in all_keys.iter().rev() {
            crate::radix::sort_by(&mut order, keys);
        }
    }
    let sorted: Vec<u32> = order.iter().map(|at| rows[*at as usize]).collect();
    let group_keys: Vec<Vec<Key>> = all_keys
        .iter()
        .take(group_columns.len())
        .map(|keys| order.iter().map(|at| keys[*at as usize]).collect())
        .collect();
    let mut aggregates = Vec::new();
    for spec in &query.aggregates {
        aggregates.push((column(&spec.column)?, spec.function));
    }
    let plan = Plan { table, query, rows: sorted, group_keys, group_columns, aggregates };
    let mut out =
        Vec::with_capacity(if plan.group_columns.is_empty() { plan.rows.len() } else { 64 });
    plan.emit(0, 0, plan.rows.len(), &mut Vec::new(), &mut out);
    Ok(ViewResult { matched: plan.rows.len(), totals: plan.totals(&plan.rows), rows: out })
}
