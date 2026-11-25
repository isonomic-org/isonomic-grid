use grid_core::{AggregateFn, Column, Table, Value, compare};

/// One aggregate over the given rows of one column. Nulls are
/// skipped; so is not-a-number. An aggregate over no values is null,
/// except the counts.
pub fn aggregate(table: &Table, index: usize, function: AggregateFn, rows: &[u32]) -> Value {
    if function == AggregateFn::Count {
        return Value::Int(rows.len() as i64);
    }
    let Some((spec, column)) = table.column(index) else { return Value::Null };
    let present = |row: &&u32| !column.is_null(**row as usize);
    if function == AggregateFn::CountValues {
        return Value::Int(rows.iter().filter(present).count() as i64);
    }
    if matches!(function, AggregateFn::Min | AggregateFn::Max) {
        let cells = rows.iter().filter(present).map(|row| column.get(spec.ty, *row as usize));
        let cells = cells.filter(|cell| cell.class() == 0);
        let best = if function == AggregateFn::Min {
            cells.min_by(compare)
        } else {
            cells.max_by(compare)
        };
        return best.unwrap_or(Value::Null);
    }
    // Sum and average. Integers stay integers until they overflow.
    let mut count = 0usize;
    let mut float = 0.0f64;
    let mut int: Option<i64> = Some(0);
    for row in rows.iter().filter(present) {
        let row = *row as usize;
        let value = match column {
            Column::Int(data, _) => {
                int = int.and_then(|sum| sum.checked_add(data[row]));
                data[row] as f64
            }
            Column::Float(data, _) if data[row].is_nan() => continue,
            Column::Float(data, _) => {
                int = None;
                data[row]
            }
            _ => {
                int = None;
                match column.get(spec.ty, row).as_f64() {
                    Some(value) if !value.is_nan() => value,
                    _ => continue,
                }
            }
        };
        float += value;
        count += 1;
    }
    match (function, count, int) {
        (_, 0, _) => Value::Null,
        (AggregateFn::Sum, _, Some(sum)) if spec.ty == grid_core::ColumnType::Int => {
            Value::Int(sum)
        }
        (AggregateFn::Sum, ..) => Value::Float(float),
        _ => Value::Float(float / count as f64),
    }
}
