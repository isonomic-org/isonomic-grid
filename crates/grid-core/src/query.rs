use serde::{Deserialize, Serialize};

use crate::{ColumnId, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub column: ColumnId,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Text contains, ignoring case and accents.
    Contains,
    /// Text starts with, ignoring case and accents.
    StartsWith,
}

/// A filter expression. A comparison with a null cell is false;
/// `IsNull` is the only test a null passes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Filter {
    All(Vec<Filter>),
    Any(Vec<Filter>),
    Not(Box<Filter>),
    Is { column: ColumnId, compare: Compare, value: Value },
    OneOf { column: ColumnId, values: Vec<Value> },
    IsNull { column: ColumnId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AggregateFn {
    /// Rows, nulls included.
    Count,
    /// Non-null cells.
    CountValues,
    Sum,
    Average,
    Min,
    Max,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    pub column: ColumnId,
    pub function: AggregateFn,
}

/// Which groups are open. Paths are group keys from the top level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Expansion {
    #[default]
    All,
    None,
    Only(Vec<Vec<Value>>),
    AllExcept(Vec<Vec<Value>>),
}

impl Expansion {
    pub fn is_open(&self, path: &[Value]) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::Only(open) => open.iter().any(|p| p == path),
            Self::AllExcept(closed) => !closed.iter().any(|p| p == path),
        }
    }
}

/// Everything a view asks of its data, as plain serializable data: a
/// saved view, an agent's request and a remote fetch are all this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ViewQuery {
    /// The columns to return, in order. Empty means all.
    pub columns: Vec<ColumnId>,
    pub filter: Option<Filter>,
    /// Text every returned row must contain in some text column.
    pub search: String,
    pub sort: Vec<SortKey>,
    pub group_by: Vec<ColumnId>,
    pub aggregates: Vec<Aggregate>,
    pub expanded: Expansion,
}
