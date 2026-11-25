// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_core::*;
use grid_formats::*;
use grid_query::{MemorySource, ViewRow};

fn spec(ty: ColumnType) -> ColumnSpec {
    ColumnSpec::new("c", "C", ty).choices(&["Low", "Médium", "High"])
}

#[test]
fn delimited_text_round_trips_awkward_fields() {
    let rows = vec![
        vec!["plain".to_owned(), "has,comma".to_owned(), "has \"quotes\"".to_owned()],
        vec!["line\nbreak".to_owned(), String::new(), " padded ".to_owned()],
        vec![String::new(), String::new(), String::new()],
    ];
    for delimiter in [',', '\t'] {
        let text = write_delimited(rows.clone(), delimiter);
        assert_eq!(read_delimited(&text, delimiter), rows);
    }
    let windows = "\u{feff}a,b\r\n1,\"x\r\ny\"\r\n\r\n2,";
    assert_eq!(read_delimited(windows, ','), [vec!["a", "b"], vec!["1", "x\r\ny"], vec!["2", ""]]);
    assert_eq!(read_delimited("a,\"open", ','), [vec!["a", "open"]]);
    assert!(read_delimited("", ',').is_empty());
}

#[test]
fn every_type_parses_what_it_formats() {
    let cases = [
        (ColumnType::Bool, Value::Bool(true), "true"),
        (ColumnType::Int, Value::Int(-42), "-42"),
        (ColumnType::Float, Value::Float(1.5), "1.5"),
        (ColumnType::Decimal, Value::Decimal(Decimal { mantissa: -1205, scale: 2 }), "-12.05"),
        (ColumnType::Decimal, Value::Decimal(Decimal { mantissa: 5, scale: 3 }), "0.005"),
        (ColumnType::Text, Value::text(" keep spaces "), " keep spaces "),
        (ColumnType::Date, Value::Date(20_782), "2026-11-25"),
        (ColumnType::Date, Value::Date(-1), "1969-12-31"),
        (
            ColumnType::DateTime,
            Value::DateTime(1_795_617_000_500_000),
            "2026-11-25T14:30:00.500000Z",
        ),
        (ColumnType::Duration, Value::Duration(90_500_000), "90.5"),
        (ColumnType::Choice, Value::Choice(1), "Médium"),
        (ColumnType::Choices, Value::Choices(vec![0, 2]), "Low;High"),
        (ColumnType::Ref, Value::Ref(7), "7"),
        (ColumnType::Json, Value::Json("{\"a\":1}".into()), "{\"a\":1}"),
        (ColumnType::Bytes, Value::Bytes([0u8, 255, 16].as_slice().into()), "00ff10"),
    ];
    for (ty, value, text) in cases {
        let column = spec(ty);
        assert_eq!(format_value(&value, &column), text, "{ty:?}");
        assert_eq!(parse_value(text, &column), Ok(value), "{ty:?}");
    }
    assert_eq!(parse_value("  ", &spec(ColumnType::Int)), Ok(Value::Null));
}

#[test]
fn dates_and_times_are_strict_and_utc() {
    assert_eq!(days_from_date(1970, 1, 1), Some(0));
    assert_eq!(days_from_date(2000, 2, 29), Some(11_016));
    assert_eq!(days_from_date(2023, 2, 29), None);
    assert_eq!(days_from_date(1900, 2, 29), None);
    for days in [-800_000, -1, 0, 59, 11_016, 20_782, 2_900_000] {
        let (year, month, day) = date_from_days(days);
        assert_eq!(days_from_date(year, month, day), Some(days));
    }
    let moment = spec(ColumnType::DateTime);
    let utc = parse_value("2026-11-25T14:30", &moment).unwrap();
    assert_eq!(parse_value("2026-11-25 15:30:00+01:00", &moment), Ok(utc.clone()));
    assert_eq!(parse_value("2026-11-25T09:30:00-05:00", &moment), Ok(utc.clone()));
    assert_eq!(parse_value("2026-11-25T14:30:00Z", &moment), Ok(utc));
    for bad in
        ["2026-13-01T00:00", "2026-11-25T24:00", "2026-11-25", "yesterday", "2026-11-25T10:00+1"]
    {
        assert!(parse_value(bad, &moment).is_err(), "{bad}");
    }
    assert_eq!(parse_value("medium", &spec(ColumnType::Choice)), Ok(Value::Choice(1)));
    for (ty, bad) in [
        (ColumnType::Bool, "maybe"),
        (ColumnType::Int, "1.5"),
        (ColumnType::Bytes, "abc"),
        (ColumnType::Choice, "Huge"),
    ] {
        let error = parse_value(bad, &spec(ty)).unwrap_err();
        assert_eq!((error.expected, error.text.as_str()), (ty, bad));
    }
}

const CSV: &str = "Name,Joined,Age,Score,Active,Notes\nAda,2024-01-15,36,9.5,yes,\"likes, commas\"\nBo,2025-06-01,,7,no,\nCy,2023-11-30,29,,yes,\"two\nlines\"\n";

#[test]
fn import_infers_types_and_export_round_trips() {
    let table = import_delimited(CSV, ',', Limits::default()).unwrap();
    let types: Vec<ColumnType> = table.specs().iter().map(|spec| spec.ty).collect();
    use ColumnType::*;
    assert_eq!(types, [Text, Date, Int, Float, Bool, Text]);
    assert_eq!(table.len(), 3);
    assert_eq!(table.get(1, 2), Value::Null);
    assert_eq!(table.get(0, 3), Value::Float(9.5));
    assert_eq!(table.get(1, 3), Value::Float(7.0));

    // Export what a sorted, filtered view shows.
    let mut source = MemorySource::new(table);
    let query = ViewQuery {
        sort: vec![SortKey { column: ColumnId::new("c2"), direction: Direction::Descending }],
        filter: Some(Filter::Is {
            column: ColumnId::new("c4"),
            compare: Compare::Eq,
            value: Value::Bool(true),
        }),
        ..Default::default()
    };
    let view = source.view(&query).unwrap();
    let table = source.table();
    let header = table.specs().iter().map(|spec| spec.title.clone()).collect::<Vec<_>>();
    let lines = view.rows.iter().filter_map(|row| match row {
        ViewRow::Data { row, .. } => Some(
            table
                .specs()
                .iter()
                .enumerate()
                .map(|(c, spec)| format_value(&table.get(*row as usize, c), spec))
                .collect(),
        ),
        ViewRow::Group { .. } => None,
    });
    let text = write_delimited(std::iter::once(header).chain(lines), '\t');
    assert_eq!(
        text,
        "Name\tJoined\tAge\tScore\tActive\tNotes\nAda\t2024-01-15\t36\t9.5\ttrue\tlikes, commas\nCy\t2023-11-30\t29\t\ttrue\t\"two\nlines\"\n"
    );
    let limits = Limits { rows: 2, ..Limits::default() };
    assert_eq!(import_delimited(CSV, ',', limits).unwrap_err(), SourceError::Limit("rows"));
    assert_eq!(import_delimited("", ',', Limits::default()).unwrap().specs().len(), 0);
}

#[test]
fn pasting_sets_cells_and_adds_rows() {
    let table = import_delimited(CSV, ',', Limits::default()).unwrap();
    let all = table.specs().to_vec();
    let mut source = MemorySource::new(table);
    let rows = [source.table().row_id(1).unwrap(), source.table().row_id(2).unwrap()];
    let fields = read_delimited("41\t8.25\n\t\n50\t1\n", '\t');
    let edits = paste_edits(&fields, &rows, &all[2..4], &all).unwrap();
    assert_eq!(edits.len(), 5);
    source.apply(&edits).unwrap();
    let table = source.table();
    assert_eq!((table.get(1, 2), table.get(1, 3)), (Value::Int(41), Value::Float(8.25)));
    assert_eq!((table.get(2, 2), table.get(2, 3)), (Value::Null, Value::Null));
    assert_eq!(
        table.row(3),
        [Value::Null, Value::Null, Value::Int(50), Value::Float(1.0), Value::Null, Value::Null]
    );
    let bad = read_delimited("forty\n", '\t');
    assert!(paste_edits(&bad, &rows, &all[2..4], &all).is_err());
}
