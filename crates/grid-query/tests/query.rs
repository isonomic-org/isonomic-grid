// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_core::*;
use grid_query::*;

type Person = (&'static str, Option<&'static str>, Option<i64>, Option<f64>, bool);

fn people() -> Table {
    let specs = vec![
        ColumnSpec::new("name", "Name", ColumnType::Text),
        ColumnSpec::new("team", "Team", ColumnType::Text),
        ColumnSpec::new("age", "Age", ColumnType::Int),
        ColumnSpec::new("score", "Score", ColumnType::Float),
        ColumnSpec::new("active", "Active", ColumnType::Bool),
    ];
    let mut table = Table::new(specs);
    let rows: [Person; 8] = [
        ("Élodie", Some("core"), Some(31), Some(9.5), true),
        ("bob", Some("core"), Some(25), Some(f64::NAN), true),
        ("Alice", Some("apps"), Some(31), Some(7.0), false),
        ("dave", None, None, None, true),
        ("Carol", Some("apps"), Some(42), Some(-1.5), true),
        ("eve", Some("core"), Some(25), Some(9.5), false),
        ("Zoë", Some("apps"), Some(19), Some(3.25), true),
        ("alice", Some("infra"), Some(31), None, true),
    ];
    for (name, team, age, score, active) in rows {
        let opt = |v: Option<Value>| v.unwrap_or(Value::Null);
        table
            .push(&[
                Value::text(name),
                opt(team.map(Value::text)),
                opt(age.map(Value::Int)),
                opt(score.map(Value::Float)),
                Value::Bool(active),
            ])
            .unwrap();
    }
    table
}

fn col(name: &str) -> ColumnId {
    ColumnId::new(name)
}

fn sort(name: &str, direction: Direction) -> SortKey {
    SortKey { column: col(name), direction }
}

fn names(table: &Table, result: &ViewResult) -> Vec<String> {
    result
        .rows
        .iter()
        .map(|row| match row {
            ViewRow::Data { row, .. } => table.get(*row as usize, 0).as_str().unwrap().to_owned(),
            ViewRow::Group { path, count, .. } => {
                let key = path.last().unwrap();
                format!(
                    "[{} x{count}]",
                    key.as_str().map_or_else(|| format!("{key:?}"), str::to_owned)
                )
            }
        })
        .collect()
}

fn is(column: &str, compare: Compare, value: Value) -> Filter {
    Filter::Is { column: col(column), compare, value }
}

#[test]
fn nulls_and_not_a_number_sort_last_in_both_directions() {
    let table = people();
    let mut query =
        ViewQuery { sort: vec![sort("score", Direction::Ascending)], ..Default::default() };
    let up = names(&table, &execute(&table, &query).unwrap());
    assert_eq!(up, ["Carol", "Zoë", "Alice", "Élodie", "eve", "bob", "dave", "alice"]);
    query.sort = vec![sort("score", Direction::Descending)];
    let down = names(&table, &execute(&table, &query).unwrap());
    assert_eq!(down, ["Élodie", "eve", "Alice", "Zoë", "Carol", "bob", "dave", "alice"]);
}

#[test]
fn text_sorts_by_folded_form_and_ties_break_by_row() {
    let table = people();
    let query = ViewQuery { sort: vec![sort("name", Direction::Ascending)], ..Default::default() };
    let sorted = names(&table, &execute(&table, &query).unwrap());
    assert_eq!(sorted, ["Alice", "alice", "bob", "Carol", "dave", "Élodie", "eve", "Zoë"]);
    let query = ViewQuery {
        sort: vec![sort("age", Direction::Descending), sort("name", Direction::Descending)],
        ..Default::default()
    };
    let sorted = names(&table, &execute(&table, &query).unwrap());
    assert_eq!(sorted, ["Carol", "Élodie", "alice", "Alice", "eve", "bob", "Zoë", "dave"]);
    let unsorted = names(&table, &execute(&table, &ViewQuery::default()).unwrap());
    assert_eq!(unsorted[0], "Élodie", "no sort keeps source order");
}

#[test]
fn filters_combine_and_nulls_fail_comparisons() {
    let table = people();
    let run = |filter: Filter| {
        let query = ViewQuery { filter: Some(filter), ..Default::default() };
        names(&table, &execute(&table, &query).unwrap())
    };
    assert_eq!(run(is("age", Compare::Ge, Value::Int(31))), ["Élodie", "Alice", "Carol", "alice"]);
    assert_eq!(run(is("age", Compare::Lt, Value::Float(25.5))), ["bob", "eve", "Zoë"]);
    assert_eq!(run(Filter::Not(Box::new(is("age", Compare::Ge, Value::Int(0))))), ["dave"]);
    assert_eq!(run(Filter::IsNull { column: col("team") }), ["dave"]);
    assert_eq!(run(is("score", Compare::Eq, Value::Float(9.5))), ["Élodie", "eve"]);
    assert_eq!(
        run(is("score", Compare::Ne, Value::Float(9.5))).len(),
        4,
        "not-a-number is unequal"
    );
    assert_eq!(run(is("name", Compare::Contains, Value::text("ELO"))), ["Élodie"]);
    assert_eq!(run(is("name", Compare::StartsWith, Value::text("zoe"))), ["Zoë"]);
    assert_eq!(run(is("name", Compare::Eq, Value::text("alice"))), ["alice"]);
    assert_eq!(run(is("active", Compare::Eq, Value::Bool(false))), ["Alice", "eve"]);
    let both = Filter::All(vec![
        is("team", Compare::Eq, Value::text("core")),
        Filter::Any(vec![
            is("age", Compare::Gt, Value::Int(30)),
            is("active", Compare::Eq, Value::Bool(false)),
        ]),
    ]);
    assert_eq!(run(both), ["Élodie", "eve"]);
    let one_of = Filter::OneOf { column: col("age"), values: vec![Value::Int(19), Value::Int(42)] };
    assert_eq!(run(one_of), ["Carol", "Zoë"]);
    assert_eq!(
        run(is("name", Compare::Gt, Value::Int(3))),
        Vec::<String>::new(),
        "mismatched types match nothing"
    );
    let unknown =
        ViewQuery { filter: Some(is("nope", Compare::Eq, Value::Int(1))), ..Default::default() };
    assert_eq!(execute(&table, &unknown), Err(SourceError::UnknownColumn));
}

#[test]
fn search_ignores_case_and_accents() {
    let table = people();
    let query = ViewQuery { search: " elodie ".into(), ..Default::default() };
    assert_eq!(names(&table, &execute(&table, &query).unwrap()), ["Élodie"]);
    let query =
        ViewQuery { search: "APPS".into(), columns: vec![col("name")], ..Default::default() };
    assert!(
        execute(&table, &query).unwrap().rows.is_empty(),
        "search stays within the view's columns"
    );
}

#[test]
fn grouping_aggregates_and_expansion() {
    let table = people();
    let mut query = ViewQuery {
        group_by: vec![col("team")],
        sort: vec![sort("age", Direction::Ascending)],
        aggregates: vec![
            Aggregate { column: col("age"), function: AggregateFn::Sum },
            Aggregate { column: col("score"), function: AggregateFn::Average },
            Aggregate { column: col("score"), function: AggregateFn::Max },
            Aggregate { column: col("name"), function: AggregateFn::Count },
            Aggregate { column: col("score"), function: AggregateFn::CountValues },
        ],
        ..Default::default()
    };
    let result = execute(&table, &query).unwrap();
    let shown = names(&table, &result);
    assert_eq!(
        shown,
        [
            "[apps x3]",
            "Zoë",
            "Alice",
            "Carol",
            "[core x3]",
            "bob",
            "eve",
            "Élodie",
            "[infra x1]",
            "alice",
            "[Null x1]",
            "dave"
        ]
    );
    let ViewRow::Group { aggregates, .. } = &result.rows[4] else { panic!() };
    assert_eq!(
        aggregates,
        &[Value::Int(81), Value::Float(9.5), Value::Float(9.5), Value::Int(3), Value::Int(3)],
        "not-a-number counts as a value but stays out of sums"
    );
    assert_eq!(result.totals[0], Value::Int(204));
    assert_eq!(result.matched, 8);

    query.expanded = Expansion::Only(vec![vec![Value::text("core")]]);
    let shown = names(&table, &execute(&table, &query).unwrap());
    assert_eq!(
        shown,
        ["[apps x3]", "[core x3]", "bob", "eve", "Élodie", "[infra x1]", "[Null x1]"]
    );

    query.group_by = vec![col("team"), col("age")];
    query.expanded = Expansion::All;
    let result = execute(&table, &query).unwrap();
    assert_eq!(
        result.rows.iter().filter(|row| matches!(row, ViewRow::Group { depth: 1, .. })).count(),
        7
    );
    assert_eq!(
        result.rows.iter().filter(|row| matches!(row, ViewRow::Data { depth: 2, .. })).count(),
        8
    );
}

#[test]
fn the_memory_source_pages_groups_and_rows() {
    let mut source = MemorySource::new(people());
    let query = ViewQuery {
        group_by: vec![col("team")],
        sort: vec![sort("name", Direction::Ascending)],
        columns: vec![col("name"), col("age")],
        expanded: Expansion::None,
        ..Default::default()
    };
    let top =
        FetchRequest { query: query.clone(), group_path: vec![], page: PageRequest::Range(0..10) };
    let page = source.fetch(&top).unwrap();
    assert_eq!(page.count, Count::Exact(4));
    assert!(
        matches!(&page.rows[0], PageRow::Group { key, count: 3, .. } if *key == Value::text("apps"))
    );
    let apps = FetchRequest {
        query: query.clone(),
        group_path: vec![Value::text("apps")],
        page: PageRequest::Range(0..2),
    };
    let page = source.fetch(&apps).unwrap();
    assert_eq!(page.count, Count::Exact(3));
    let PageRow::Data { id, values } = &page.rows[1] else { panic!() };
    assert_eq!(values, &[Value::text("Carol"), Value::Int(42)]);
    let after = FetchRequest { page: PageRequest::After { row: *id, limit: 5 }, ..apps.clone() };
    let page = source.fetch(&after).unwrap();
    assert!(
        matches!(&page.rows[..], [PageRow::Data { values, .. }] if values[0] == Value::text("Zoë"))
    );
    let before = FetchRequest { page: PageRequest::Before { row: *id, limit: 5 }, ..apps };
    assert_eq!(source.fetch(&before).unwrap().rows.len(), 1);
}

#[test]
fn edits_are_all_or_nothing_and_reported() {
    let mut source = MemorySource::new(people());
    let first = source.table().row_id(0).unwrap();
    let good = Edit::SetCell { row: first, column: col("age"), value: Value::Int(32) };
    let bad = Edit::SetCell { row: first, column: col("age"), value: Value::text("old") };
    let insert = Edit::InsertRow { values: vec![Value::text("new")] };
    assert_eq!(
        source.apply(&[good.clone(), insert.clone(), bad]),
        Err(SourceError::WrongType(col("age")))
    );
    assert_eq!(source.table().get(0, 2), Value::Int(31));
    assert_eq!(source.table().len(), 8);
    assert!(source.changes().is_empty());

    let changes = source.apply(&[good, insert, Edit::DeleteRow { row: first }]).unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(source.changes(), changes);
    assert_eq!(source.table().len(), 8);
    assert!(source.table().row_of(first).is_none());
    let Change::Deleted { row, values } = &changes[2] else { panic!() };
    source.apply(&[Edit::RestoreRow { row: *row, values: values.clone() }]).unwrap();
    assert_eq!(source.table().get(source.table().row_of(first).unwrap(), 2), Value::Int(32));
    let limits = Limits { rows: 1, ..Limits::default() };
    let mut tiny = Table::with_limits(vec![ColumnSpec::new("a", "A", ColumnType::Int)], limits);
    tiny.push(&[Value::Int(1)]).unwrap();
    assert_eq!(tiny.push(&[Value::Int(2)]), Err(SourceError::Limit("rows")));
}

#[test]
fn a_query_is_plain_data() {
    let query = ViewQuery {
        filter: Some(is("age", Compare::Ge, Value::Int(21))),
        sort: vec![sort("name", Direction::Descending)],
        group_by: vec![col("team")],
        expanded: Expansion::AllExcept(vec![vec![Value::Null]]),
        ..Default::default()
    };
    let text = serde_json::to_string(&query).unwrap();
    assert_eq!(serde_json::from_str::<ViewQuery>(&text).unwrap(), query);
}
