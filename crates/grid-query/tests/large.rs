// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Large inputs take the radix path. Its order must equal a plain
//! comparison sort under the shared semantics.

use std::cmp::Ordering;

use grid_core::*;
use grid_query::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn table(rows: usize) -> Table {
    let specs = vec![
        ColumnSpec::new("n", "N", ColumnType::Int),
        ColumnSpec::new("x", "X", ColumnType::Float),
        ColumnSpec::new("word", "Word", ColumnType::Text),
        ColumnSpec::new("flag", "Flag", ColumnType::Bool),
    ];
    let words = ["delta", "Alpha", "élan", "echo", "alpha", "Bravo", "charlie", "Zulu", "éclair"];
    let mut rng = Rng(7);
    let mut table = Table::new(specs);
    for _ in 0..rows {
        let r = rng.next();
        let maybe = |keep: bool, value: Value| if keep { value } else { Value::Null };
        let x = match r % 23 {
            0 => f64::NAN,
            1 => -0.0,
            _ => ((r >> 8) % 2_000) as f64 / 7.0 - 100.0,
        };
        table
            .push(&[
                maybe(!r.is_multiple_of(11), Value::Int((r >> 16) as i64 % 500 - 250)),
                maybe(!r.is_multiple_of(13), Value::Float(x)),
                maybe(!r.is_multiple_of(17), Value::text(words[(r >> 24) as usize % words.len()])),
                Value::Bool(r.is_multiple_of(2)),
            ])
            .unwrap();
    }
    // Put a deleted row back, so table order no longer follows IDs.
    let moved = table.row_id(3).unwrap();
    let values = table.remove(moved).unwrap();
    table.restore(moved, &values).unwrap();
    table
}

fn expected(table: &Table, sort: &[SortKey]) -> Vec<RowId> {
    let mut rows: Vec<usize> = (0..table.len()).collect();
    rows.sort_by(|a, b| {
        for key in sort {
            let column = table.column_index(&key.column).unwrap();
            let (x, y) = (table.get(*a, column), table.get(*b, column));
            let mut ordering = compare(&x, &y);
            if key.direction == Direction::Descending && x.class() == 0 && y.class() == 0 {
                ordering = ordering.reverse();
            }
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        table.row_id(*a).cmp(&table.row_id(*b))
    });
    rows.into_iter().map(|row| table.row_id(row).unwrap()).collect()
}

#[test]
fn the_radix_path_matches_a_comparison_sort() {
    let table = table(6_000);
    let key = |name: &str, direction| SortKey { column: ColumnId::new(name), direction };
    use Direction::{Ascending as Up, Descending as Down};
    let cases = [
        vec![key("n", Up)],
        vec![key("n", Down)],
        vec![key("x", Up)],
        vec![key("x", Down)],
        vec![key("word", Up)],
        vec![key("word", Down), key("x", Up)],
        vec![key("flag", Down), key("word", Up), key("n", Down)],
    ];
    for sort in cases {
        let query = ViewQuery { sort: sort.clone(), ..Default::default() };
        let got: Vec<RowId> = execute(&table, &query)
            .unwrap()
            .rows
            .iter()
            .map(|row| match row {
                ViewRow::Data { id, .. } => *id,
                ViewRow::Group { .. } => unreachable!(),
            })
            .collect();
        assert!(got == expected(&table, &sort), "sort {sort:?}");
    }
}

#[test]
fn text_stays_correct_as_the_dictionary_changes() {
    let mut source = MemorySource::new(table(3_000));
    let word = ColumnId::new("word");
    let query = ViewQuery {
        group_by: vec![word.clone()],
        expanded: Expansion::None,
        filter: Some(Filter::Is {
            column: word.clone(),
            compare: Compare::Contains,
            value: Value::text("E"),
        }),
        ..Default::default()
    };
    let groups = |source: &mut MemorySource| -> Vec<String> {
        let view = source.view(&query).unwrap();
        view.rows
            .iter()
            .map(|row| match row {
                ViewRow::Group { path, .. } => path[0].as_str().unwrap().to_owned(),
                ViewRow::Data { .. } => unreachable!(),
            })
            .collect()
    };
    assert_eq!(groups(&mut source), ["charlie", "delta", "echo", "éclair", "élan"]);
    // A string the dictionary has never seen arrives, and one leaves.
    let first = source.table().row_id(0).unwrap();
    source
        .apply(&[Edit::SetCell { row: first, column: word.clone(), value: Value::text("Dune") }])
        .unwrap();
    assert_eq!(groups(&mut source), ["charlie", "delta", "Dune", "echo", "éclair", "élan"]);
    let exact = ViewQuery {
        filter: Some(Filter::Is {
            column: word,
            compare: Compare::Ne,
            value: Value::text("never seen"),
        }),
        ..Default::default()
    };
    let view = source.view(&exact).unwrap();
    assert!(
        view.matched > 2_700 && view.matched < 3_000,
        "nulls fail a comparison; every text passes"
    );
}
