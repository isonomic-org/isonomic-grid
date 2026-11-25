// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use grid_core::{Column, Direction, Table, compare, fold};

/// A sort key: the class (value, not-a-number, null) and then the
/// value's rank. Comparing keys compares cells.
pub type Key = (u8, u64);

const NULL: Key = (2, 0);
const NAN: Key = (1, 0);

fn float_rank(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 == 0 { bits | 1 << 63 } else { !bits }
}

/// One key per row in `rows`, for one column. Nulls and not-a-number
/// keep their place after the values in both directions.
pub fn keys(table: &Table, index: usize, rows: &[u32], direction: Direction) -> Vec<Key> {
    let Some((spec, column)) = table.column(index) else { return vec![NULL; rows.len()] };
    let turn = |rank: u64| match direction {
        Direction::Ascending => (0, rank),
        Direction::Descending => (0, !rank),
    };
    let at = |row: &u32| *row as usize;
    match column {
        Column::Int(data, valid) => rows
            .iter()
            .map(|row| if valid[at(row)] { turn((data[at(row)] as u64) ^ (1 << 63)) } else { NULL })
            .collect(),
        Column::Bool(data, valid) => rows
            .iter()
            .map(|row| if valid[at(row)] { turn(u64::from(data[at(row)])) } else { NULL })
            .collect(),
        Column::Float(data, valid) => rows
            .iter()
            .map(|row| match (valid[at(row)], data[at(row)]) {
                (false, _) => NULL,
                (true, value) if value.is_nan() => NAN,
                (true, value) => turn(float_rank(value)),
            })
            .collect(),
        Column::Text(data, valid) => {
            // Every text column goes through a dictionary: each
            // distinct string is folded and ranked once, and rows are
            // then compared by rank alone.
            let mut distinct: HashMap<&str, u64> = HashMap::new();
            for row in rows.iter().filter(|row| valid[at(row)]) {
                distinct.entry(&data[at(row)]).or_insert(0);
            }
            let mut ordered: Vec<(String, &str)> =
                distinct.keys().map(|text| (fold(text), *text)).collect();
            ordered.sort_unstable();
            for (rank, (_, text)) in ordered.iter().enumerate() {
                distinct.insert(text, rank as u64);
            }
            rows.iter()
                .map(|row| if valid[at(row)] { turn(distinct[&*data[at(row)]]) } else { NULL })
                .collect()
        }
        Column::Other(_) => {
            let cells: Vec<_> = rows.iter().map(|row| column.get(spec.ty, at(row))).collect();
            let mut order: Vec<usize> = (0..cells.len()).collect();
            order.sort_by(|a, b| compare(&cells[*a], &cells[*b]));
            let mut out = vec![NULL; cells.len()];
            let mut rank = 0u64;
            for (position, cell) in order.iter().enumerate() {
                if position > 0 && compare(&cells[order[position - 1]], &cells[*cell]).is_ne() {
                    rank += 1;
                }
                out[*cell] = match cells[*cell].class() {
                    0 => turn(rank),
                    class => (class, 0),
                };
            }
            out
        }
    }
}
