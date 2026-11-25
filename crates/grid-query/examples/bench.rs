//! Rough timings for the query pipeline. Run with
//! `cargo run --release -p grid-query --example bench -- 1000000`.

use std::time::Instant;

use grid_core::*;
use grid_query::execute;

fn main() {
    let rows: usize = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1_000_000);
    let specs = vec![
        ColumnSpec::new("id", "ID", ColumnType::Int),
        ColumnSpec::new("amount", "Amount", ColumnType::Float),
        ColumnSpec::new("name", "Name", ColumnType::Text),
        ColumnSpec::new("city", "City", ColumnType::Text),
    ];
    let cities = ["Rabat", "Tanger", "Toronto", "Miami", "Buffalo", "Lisbon", "Zürich", "Århus"];
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let started = Instant::now();
    let mut table = Table::new(specs);
    for _ in 0..rows {
        let n = next();
        let row = [
            Value::Int((n % 1_000_000) as i64),
            Value::Float((n >> 20) as f64 / 1e6),
            Value::text(&format!("Customer {:07}", n % 5_000_000)),
            Value::text(cities[(n >> 40) as usize % cities.len()]),
        ];
        table.push(&row).expect("push");
    }
    println!("build {rows} rows: {:?}", started.elapsed());

    let col = ColumnId::new;
    let sort = |name: &str| SortKey { column: col(name), direction: Direction::Ascending };
    let is = |name: &str, compare, value| Filter::Is { column: col(name), compare, value };
    let cases: Vec<(&str, ViewQuery)> = vec![
        ("no query", ViewQuery::default()),
        ("sort by integer", ViewQuery { sort: vec![sort("id")], ..Default::default() }),
        (
            "sort by two keys",
            ViewQuery { sort: vec![sort("city"), sort("amount")], ..Default::default() },
        ),
        (
            "sort by text, mostly distinct",
            ViewQuery { sort: vec![sort("name")], ..Default::default() },
        ),
        (
            "numeric filter",
            ViewQuery {
                filter: Some(is("id", Compare::Lt, Value::Int(1_000))),
                ..Default::default()
            },
        ),
        (
            "text contains",
            ViewQuery {
                filter: Some(is("name", Compare::Contains, Value::text("0042"))),
                ..Default::default()
            },
        ),
        ("search all columns", ViewQuery { search: "zurich".into(), ..Default::default() }),
        (
            "group by city, sum and average, collapsed",
            ViewQuery {
                group_by: vec![col("city")],
                aggregates: vec![
                    Aggregate { column: col("amount"), function: AggregateFn::Sum },
                    Aggregate { column: col("amount"), function: AggregateFn::Average },
                ],
                expanded: Expansion::None,
                ..Default::default()
            },
        ),
    ];
    for (label, query) in cases {
        let started = Instant::now();
        let result = execute(&table, &query).expect("query");
        println!(
            "{label}: {:?} ({} rows shown, {} matched)",
            started.elapsed(),
            result.rows.len(),
            result.matched
        );
    }
}
