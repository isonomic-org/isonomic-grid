// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;

use grid_core::{
    Column, ColumnId, ColumnType, Compare, Filter, SourceError, Table, Value, compare, fold,
};

use crate::Bitmap;

fn holds(ordering: Ordering, test: Compare) -> bool {
    match test {
        Compare::Eq => ordering == Ordering::Equal,
        Compare::Ne => ordering != Ordering::Equal,
        Compare::Lt => ordering == Ordering::Less,
        Compare::Le => ordering != Ordering::Greater,
        Compare::Gt => ordering == Ordering::Greater,
        Compare::Ge => ordering != Ordering::Less,
        Compare::Contains | Compare::StartsWith => false,
    }
}

/// Whether `hay` contains (or starts with) an already folded needle.
/// Plain ASCII text is matched in place, with no allocation.
fn text_matches(hay: &str, needle: &str, prefix: bool) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !hay.is_ascii() || !needle.is_ascii() {
        let folded = fold(hay);
        return if prefix { folded.starts_with(needle) } else { folded.contains(needle) };
    }
    let (hay, needle) = (hay.as_bytes(), needle.as_bytes());
    if prefix {
        return hay.len() >= needle.len() && hay[..needle.len()].eq_ignore_ascii_case(needle);
    }
    hay.windows(needle.len()).any(|window| window.eq_ignore_ascii_case(needle))
}

fn column_of(table: &Table, id: &ColumnId) -> Result<usize, SourceError> {
    table.column_index(id).ok_or(SourceError::UnknownColumn)
}

fn leaf(table: &Table, index: usize, test: Compare, value: &Value) -> Bitmap {
    let len = table.len();
    let Some((spec, column)) = table.column(index) else { return Bitmap::new(len, false) };
    if value.is_null() {
        return Bitmap::new(len, false);
    }
    let textual = matches!(test, Compare::Contains | Compare::StartsWith);
    match (column, value) {
        (Column::Int(data, valid), Value::Int(_) | Value::Date(_) | Value::DateTime(_))
        | (Column::Int(data, valid), Value::Duration(_) | Value::Choice(_) | Value::Ref(_))
            if !textual =>
        {
            let wanted = match value {
                Value::Int(v) | Value::DateTime(v) | Value::Duration(v) => *v,
                Value::Date(v) => i64::from(*v),
                Value::Choice(v) => i64::from(*v),
                Value::Ref(v) => *v as i64,
                _ => 0,
            };
            Bitmap::from_fn(len, |row| valid[row] && holds(data[row].cmp(&wanted), test))
        }
        (Column::Int(data, valid), Value::Float(wanted)) if !textual => {
            Bitmap::from_fn(len, |row| {
                valid[row] && (data[row] as f64).partial_cmp(wanted).is_some_and(|o| holds(o, test))
            })
        }
        (Column::Float(data, valid), _) if !textual && value.as_f64().is_some() => {
            let wanted = value.as_f64().unwrap_or(f64::NAN);
            Bitmap::from_fn(len, |row| {
                valid[row]
                    && match data[row].partial_cmp(&wanted) {
                        Some(ordering) => holds(ordering, test),
                        None => test == Compare::Ne,
                    }
            })
        }
        (Column::Text(text), Value::Text(wanted)) if textual => {
            // Match each distinct string once, then look rows up.
            let needle = fold(wanted);
            let prefix = test == Compare::StartsWith;
            let hits: Vec<bool> = text
                .dictionary()
                .iter()
                .map(|entry| text_matches(entry, &needle, prefix))
                .collect();
            let (codes, valid) = (text.codes(), text.validity());
            Bitmap::from_fn(len, |row| valid[row] && hits[codes[row] as usize])
        }
        (Column::Text(text), Value::Text(wanted)) if matches!(test, Compare::Eq | Compare::Ne) => {
            let equal = test == Compare::Eq;
            let (codes, valid) = (text.codes(), text.validity());
            match text.code_of(wanted) {
                Some(code) => {
                    Bitmap::from_fn(len, |row| valid[row] && (codes[row] == code) == equal)
                }
                None => Bitmap::from_fn(len, |row| valid[row] && !equal),
            }
        }
        _ => {
            let numeric = value.as_f64().is_some();
            Bitmap::from_fn(len, |row| {
                let cell = column.get(spec.ty, row);
                if cell.is_null() || cell.as_f64().is_some() != numeric || textual {
                    return false;
                }
                if cell.class() == 1 || value.class() == 1 {
                    return test == Compare::Ne;
                }
                holds(compare(&cell, value), test)
            })
        }
    }
}

/// The rows a filter keeps.
pub fn matching(table: &Table, filter: &Filter) -> Result<Bitmap, SourceError> {
    let len = table.len();
    Ok(match filter {
        Filter::All(parts) => {
            let mut out = Bitmap::new(len, true);
            for part in parts {
                out.and(&matching(table, part)?);
            }
            out
        }
        Filter::Any(parts) => {
            let mut out = Bitmap::new(len, false);
            for part in parts {
                out.or(&matching(table, part)?);
            }
            out
        }
        Filter::Not(inner) => {
            let mut out = matching(table, inner)?;
            out.not();
            out
        }
        Filter::Is { column, compare, value } => {
            leaf(table, column_of(table, column)?, *compare, value)
        }
        Filter::OneOf { column, values } => {
            let index = column_of(table, column)?;
            let mut out = Bitmap::new(len, false);
            for value in values {
                out.or(&leaf(table, index, Compare::Eq, value));
            }
            out
        }
        Filter::IsNull { column } => {
            let index = column_of(table, column)?;
            let (_, data) = table.column(index).ok_or(SourceError::UnknownColumn)?;
            Bitmap::from_fn(len, |row| data.is_null(row))
        }
    })
}

/// The rows whose text, in any of the given columns, contains the
/// search text. Case and accents are ignored.
pub fn search(table: &Table, text: &str, columns: &[usize]) -> Bitmap {
    let needle = fold(text.trim());
    let mut out = Bitmap::new(table.len(), needle.is_empty());
    if needle.is_empty() {
        return out;
    }
    for index in columns {
        let Some((spec, column)) = table.column(*index) else { continue };
        let hit = match column {
            Column::Text(text) => {
                let hits: Vec<bool> = text
                    .dictionary()
                    .iter()
                    .map(|entry| text_matches(entry, &needle, false))
                    .collect();
                let (codes, valid) = (text.codes(), text.validity());
                Bitmap::from_fn(table.len(), |row| valid[row] && hits[codes[row] as usize])
            }
            _ if spec.ty == ColumnType::Choice => {
                Bitmap::from_fn(table.len(), |row| match column.get(spec.ty, row) {
                    Value::Choice(choice) => spec
                        .choices
                        .get(choice as usize)
                        .is_some_and(|label| text_matches(label, &needle, false)),
                    _ => false,
                })
            }
            _ => continue,
        };
        out.or(&hit);
    }
    out
}
