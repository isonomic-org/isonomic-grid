use grid_core::*;
use grid_interact::*;
use grid_query::MemorySource;

fn cell(row: usize, column: usize) -> Cell {
    Cell { row, column }
}

#[test]
fn the_keyboard_moves_and_extends() {
    let mut selection = Selection::at(cell(2, 1));
    let go = |s: &mut Selection, step, extend| s.navigate(step, 10, 4, 5, extend);
    assert_eq!(go(&mut selection, Move::Down, false), cell(3, 1));
    assert_eq!(go(&mut selection, Move::PageDown, false), cell(8, 1));
    assert_eq!(go(&mut selection, Move::PageDown, false), cell(9, 1));
    assert_eq!(go(&mut selection, Move::End, false), cell(9, 3));
    assert_eq!(go(&mut selection, Move::Next, false), cell(9, 3), "the last cell does not wrap");
    assert_eq!(go(&mut selection, Move::FirstCell, false), cell(0, 0));
    assert_eq!(go(&mut selection, Move::Previous, false), cell(0, 0));
    assert_eq!(go(&mut selection, Move::Left, false), cell(0, 0));
    go(&mut selection, Move::End, false);
    assert_eq!(go(&mut selection, Move::Next, false), cell(1, 0), "Tab wraps to the next row");
    assert_eq!(go(&mut selection, Move::Previous, false), cell(0, 3));

    let mut selection = Selection::at(cell(2, 1));
    go(&mut selection, Move::Down, true);
    go(&mut selection, Move::Right, true);
    assert_eq!(selection.active, cell(2, 1), "extending keeps the active cell");
    assert_eq!(selection.ranges, [CellRange { anchor: cell(2, 1), head: cell(3, 2) }]);
    assert!(selection.contains(cell(3, 1)) && !selection.contains(cell(4, 1)));
    assert_eq!(selection.ranges[0].cell_count(), 4);
    go(&mut selection, Move::Up, false);
    assert_eq!(selection, Selection::at(cell(1, 1)), "moving collapses the selection");
    assert_eq!(Selection::default().navigate(Move::Down, 0, 0, 5, false), cell(0, 0));
}

#[test]
fn the_pointer_selects_extends_and_adds() {
    let mut selection = Selection::at(cell(0, 0));
    selection.click(cell(2, 2), true, false);
    assert_eq!(selection.ranges[0].rows(), (0, 2));
    selection.click(cell(5, 1), false, true);
    selection.drag_to(cell(6, 3));
    assert_eq!(selection.ranges.len(), 2);
    assert_eq!(selection.active, cell(5, 1));
    assert!(
        selection.contains(cell(1, 1))
            && selection.contains(cell(6, 2))
            && !selection.contains(cell(4, 0))
    );
    selection.click(cell(9, 9), false, false);
    assert_eq!(selection, Selection::at(cell(9, 9)));
    selection.select_all(100, 8);
    assert_eq!(selection.ranges[0].cell_count(), 800);
}

#[test]
fn selecting_all_rows_does_not_list_them() {
    let mut rows = RowSelection::default();
    rows.toggle(RowId(4));
    rows.set(RowId(9), true);
    assert_eq!(rows.count(1_000_000), 2);
    rows.select_all();
    rows.toggle(RowId(4));
    rows.set(RowId(77), false);
    assert_eq!(rows.count(1_000_000), 999_998);
    assert!(rows.contains(RowId(123_456)) && !rows.contains(RowId(77)));
    assert_eq!(rows, RowSelection::AllExcept([RowId(4), RowId(77)].into()));
    rows.clear();
    assert!(rows.is_empty(1_000_000));
}

#[test]
fn the_edit_log_undoes_and_redoes_against_a_source() {
    let specs = vec![
        ColumnSpec::new("name", "Name", ColumnType::Text),
        ColumnSpec::new("n", "N", ColumnType::Int),
    ];
    let mut table = Table::new(specs);
    let a = table.push(&[Value::text("a"), Value::Int(1)]).unwrap();
    let b = table.push(&[Value::text("b"), Value::Int(2)]).unwrap();
    let mut source = MemorySource::new(table);
    let mut log = EditLog::new(100);
    let n = ColumnId::new("n");
    let snapshot = |source: &MemorySource| -> Vec<Vec<Value>> {
        let table = source.table();
        let mut rows: Vec<(RowId, Vec<Value>)> =
            (0..table.len()).map(|row| (table.row_id(row).unwrap(), table.row(row))).collect();
        rows.sort_by_key(|(id, _)| *id);
        rows.into_iter().map(|(_, values)| values).collect()
    };
    let start = snapshot(&source);
    log.apply(&mut source, &[Edit::SetCell { row: a, column: n.clone(), value: Value::Int(10) }])
        .unwrap();
    let edited = snapshot(&source);
    let batch = [
        Edit::DeleteRow { row: b },
        Edit::InsertRow { values: vec![Value::text("c"), Value::Int(3)] },
    ];
    log.apply(&mut source, &batch).unwrap();
    let end = snapshot(&source);
    assert_eq!(end.len(), 2);

    assert!(log.undo(&mut source).unwrap().is_some());
    assert_eq!(snapshot(&source), edited);
    assert!(source.table().row_of(b).is_some(), "a restored row keeps its ID");
    assert!(log.undo(&mut source).unwrap().is_some());
    assert_eq!(snapshot(&source), start);
    assert_eq!(log.undo(&mut source).unwrap(), None);
    assert!(log.redo(&mut source).unwrap().is_some() && log.redo(&mut source).unwrap().is_some());
    assert_eq!(snapshot(&source), end);
    assert!(!log.can_redo());

    let bad = Edit::SetCell { row: a, column: n, value: Value::text("x") };
    assert!(log.apply(&mut source, &[bad]).is_err());
    assert!(log.undo(&mut source).unwrap().is_some(), "a refused edit leaves the log intact");
    assert_eq!(snapshot(&source), edited);
}
