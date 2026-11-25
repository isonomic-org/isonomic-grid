// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_core::{ColumnSpec, ColumnType, Edit, Limits, RowId, SourceError, Table, Value};

use crate::text::parse_value;
use crate::{ParseError, read_delimited};

/// The narrowest type every non-empty field of a column fits.
pub fn infer_type<'a>(fields: impl Iterator<Item = &'a str> + Clone) -> ColumnType {
    let candidates = [
        ColumnType::Int,
        ColumnType::Float,
        ColumnType::Bool,
        ColumnType::Date,
        ColumnType::DateTime,
    ];
    let filled = || fields.clone().filter(|field| !field.trim().is_empty());
    if filled().next().is_none() {
        return ColumnType::Text;
    }
    for ty in candidates {
        let spec = ColumnSpec::new("", "", ty);
        if filled().all(|field| parse_value(field, &spec).is_ok()) {
            return ty;
        }
    }
    ColumnType::Text
}

/// Reads delimited text with a header row into a table, inferring
/// each column's type. Rows past the row limit are an error; short
/// rows are padded with nulls and long rows are cut.
pub fn import_delimited(text: &str, delimiter: char, limits: Limits) -> Result<Table, SourceError> {
    let mut rows = read_delimited(text, delimiter).into_iter();
    let header = rows.next().unwrap_or_default();
    let rows: Vec<Vec<String>> = rows.collect();
    let mut specs = Vec::new();
    for (index, title) in header.iter().enumerate().take(limits.columns) {
        let fields = rows.iter().map(move |row| row.get(index).map_or("", String::as_str));
        let title = if title.trim().is_empty() {
            format!("Column {}", index + 1)
        } else {
            title.trim().to_owned()
        };
        specs.push(ColumnSpec::new(&format!("c{index}"), &title, infer_type(fields)));
    }
    let mut table = Table::with_limits(specs, limits);
    for row in rows {
        let values: Vec<Value> = table
            .specs()
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                parse_value(row.get(index).map_or("", String::as_str), spec).unwrap_or(Value::Null)
            })
            .collect();
        table.push(&values)?;
    }
    Ok(table)
}

/// Turns pasted fields into edits. Fields land on the target rows and
/// columns starting at the top-left cell; rows past the last target
/// row become new rows. A field that does not parse stops the paste.
pub fn paste_edits(
    fields: &[Vec<String>],
    rows: &[RowId],
    columns: &[ColumnSpec],
    all_columns: &[ColumnSpec],
) -> Result<Vec<Edit>, ParseError> {
    let mut edits = Vec::new();
    for (offset, line) in fields.iter().enumerate() {
        let cells = line.iter().zip(columns).filter(|(_, spec)| spec.editable);
        match rows.get(offset) {
            Some(row) => {
                for (field, spec) in cells {
                    let value = parse_value(field, spec)?;
                    edits.push(Edit::SetCell { row: *row, column: spec.id.clone(), value });
                }
            }
            None => {
                let mut values = vec![Value::Null; all_columns.len()];
                for (field, spec) in cells {
                    if let Some(at) = all_columns.iter().position(|known| known.id == spec.id) {
                        values[at] = parse_value(field, spec)?;
                    }
                }
                edits.push(Edit::InsertRow { values });
            }
        }
    }
    Ok(edits)
}
