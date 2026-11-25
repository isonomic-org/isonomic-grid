use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Value;

/// A column's stable name within its table.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ColumnId(pub Arc<str>);

impl ColumnId {
    pub fn new(name: &str) -> Self {
        Self(Arc::from(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnType {
    Bool,
    Int,
    Float,
    Decimal,
    Text,
    Date,
    DateTime,
    Duration,
    Choice,
    Choices,
    Ref,
    Json,
    Bytes,
    Custom,
}

impl ColumnType {
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Decimal | Self::Duration)
    }

    /// Whether a non-null value belongs in a column of this type.
    pub fn accepts(self, value: &Value) -> bool {
        matches!(
            (self, value),
            (_, Value::Null)
                | (Self::Bool, Value::Bool(_))
                | (Self::Int, Value::Int(_))
                | (Self::Float, Value::Float(_) | Value::Int(_))
                | (Self::Decimal, Value::Decimal(_))
                | (Self::Text, Value::Text(_))
                | (Self::Date, Value::Date(_))
                | (Self::DateTime, Value::DateTime(_))
                | (Self::Duration, Value::Duration(_))
                | (Self::Choice, Value::Choice(_))
                | (Self::Choices, Value::Choices(_))
                | (Self::Ref, Value::Ref(_))
                | (Self::Json, Value::Json(_))
                | (Self::Bytes, Value::Bytes(_))
                | (Self::Custom, Value::Custom(_))
        )
    }
}

/// A column's declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub id: ColumnId,
    pub title: String,
    pub ty: ColumnType,
    /// The labels of a choice column, by index.
    pub choices: Vec<String>,
    pub editable: bool,
}

impl ColumnSpec {
    pub fn new(id: &str, title: &str, ty: ColumnType) -> Self {
        Self {
            id: ColumnId::new(id),
            title: title.to_owned(),
            ty,
            choices: Vec::new(),
            editable: true,
        }
    }

    pub fn choices(mut self, labels: &[&str]) -> Self {
        self.choices = labels.iter().map(|label| (*label).to_owned()).collect();
        self
    }
}

/// One column's cells. The common types are stored as typed vectors;
/// a parallel validity vector marks nulls.
#[derive(Clone, Debug)]
pub enum Column {
    Bool(Vec<bool>, Vec<bool>),
    /// Integers, and every type kept as one: dates, date-times,
    /// durations, choices and references.
    Int(Vec<i64>, Vec<bool>),
    Float(Vec<f64>, Vec<bool>),
    Text(Vec<Arc<str>>, Vec<bool>),
    Other(Vec<Value>),
}

impl Column {
    pub fn new(ty: ColumnType) -> Self {
        match ty {
            ColumnType::Bool => Self::Bool(Vec::new(), Vec::new()),
            ColumnType::Float => Self::Float(Vec::new(), Vec::new()),
            ColumnType::Text => Self::Text(Vec::new(), Vec::new()),
            ColumnType::Int
            | ColumnType::Date
            | ColumnType::DateTime
            | ColumnType::Duration
            | ColumnType::Choice
            | ColumnType::Ref => Self::Int(Vec::new(), Vec::new()),
            _ => Self::Other(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Bool(_, valid)
            | Self::Int(_, valid)
            | Self::Float(_, valid)
            | Self::Text(_, valid) => valid.len(),
            Self::Other(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_null(&self, row: usize) -> bool {
        match self {
            Self::Bool(_, valid)
            | Self::Int(_, valid)
            | Self::Float(_, valid)
            | Self::Text(_, valid) => !valid.get(row).copied().unwrap_or(false),
            Self::Other(values) => values.get(row).is_none_or(Value::is_null),
        }
    }

    /// The cell as a value of the column's declared type.
    pub fn get(&self, ty: ColumnType, row: usize) -> Value {
        if self.is_null(row) {
            return Value::Null;
        }
        match self {
            Self::Bool(data, _) => Value::Bool(data[row]),
            Self::Float(data, _) => Value::Float(data[row]),
            Self::Text(data, _) => Value::Text(data[row].clone()),
            Self::Other(values) => values[row].clone(),
            Self::Int(data, _) => {
                let v = data[row];
                match ty {
                    ColumnType::Date => Value::Date(v as i32),
                    ColumnType::DateTime => Value::DateTime(v),
                    ColumnType::Duration => Value::Duration(v),
                    ColumnType::Choice => Value::Choice(v as u32),
                    ColumnType::Ref => Value::Ref(v as u64),
                    _ => Value::Int(v),
                }
            }
        }
    }

    fn slot(value: &Value) -> i64 {
        match value {
            Value::Int(v) | Value::DateTime(v) | Value::Duration(v) => *v,
            Value::Date(v) => i64::from(*v),
            Value::Choice(v) => i64::from(*v),
            Value::Ref(v) => *v as i64,
            _ => 0,
        }
    }

    /// Writes a cell; `row == len` appends. The caller has checked
    /// the type and the index.
    pub(crate) fn put(&mut self, row: usize, value: &Value) {
        let present = !value.is_null();
        fn store<T: Clone>(
            data: &mut Vec<T>,
            valid: &mut Vec<bool>,
            row: usize,
            v: T,
            present: bool,
        ) {
            if row >= data.len() {
                data.push(v);
                valid.push(present);
            } else {
                data[row] = v;
                valid[row] = present;
            }
        }
        match self {
            Self::Bool(data, valid) => {
                store(data, valid, row, matches!(value, Value::Bool(true)), present);
            }
            Self::Int(data, valid) => store(data, valid, row, Self::slot(value), present),
            Self::Float(data, valid) => {
                store(data, valid, row, value.as_f64().unwrap_or(0.0), present);
            }
            Self::Text(data, valid) => {
                let text = match value {
                    Value::Text(text) => text.clone(),
                    _ => Arc::from(""),
                };
                store(data, valid, row, text, present);
            }
            Self::Other(values) => {
                if row >= values.len() {
                    values.push(value.clone());
                } else {
                    values[row] = value.clone();
                }
            }
        }
    }

    pub(crate) fn remove(&mut self, row: usize) {
        match self {
            Self::Bool(data, valid) => {
                data.remove(row);
                valid.remove(row);
            }
            Self::Int(data, valid) => {
                data.remove(row);
                valid.remove(row);
            }
            Self::Float(data, valid) => {
                data.remove(row);
                valid.remove(row);
            }
            Self::Text(data, valid) => {
                data.remove(row);
                valid.remove(row);
            }
            Self::Other(values) => {
                values.remove(row);
            }
        }
    }
}
