use std::fmt;
use std::ops::Range;

use crate::{ColumnId, ColumnSpec, RowId, Value, ViewQuery};

/// What a source can do itself. A view asks the source for what it
/// supports and does the rest locally, or turns the feature off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub filter: bool,
    pub search: bool,
    pub sort: bool,
    pub group: bool,
    pub aggregate: bool,
    pub edit: bool,
    /// Whether row counts are exact.
    pub exact_count: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageRequest {
    /// Rows by position in the result.
    Range(Range<usize>),
    /// Up to `limit` rows after a known row, for sources that page
    /// by key.
    After {
        row: RowId,
        limit: usize,
    },
    Before {
        row: RowId,
        limit: usize,
    },
}

/// A request for one window of rows.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchRequest {
    pub query: ViewQuery,
    /// The group whose children are wanted; empty for the top level.
    pub group_path: Vec<Value>,
    pub page: PageRequest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageRow {
    Group { key: Value, count: usize, aggregates: Vec<Value> },
    Data { id: RowId, values: Vec<Value> },
}

/// How many rows a level holds, as far as the source knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    Exact(usize),
    AtLeast(usize),
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub rows: Vec<PageRow>,
    pub count: Count,
}

/// One edit against a source.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    SetCell {
        row: RowId,
        column: ColumnId,
        value: Value,
    },
    InsertRow {
        values: Vec<Value>,
    },
    DeleteRow {
        row: RowId,
    },
    /// Puts a deleted row back under its ID. Undo uses this.
    RestoreRow {
        row: RowId,
        values: Vec<Value>,
    },
}

/// What changed in a source, by its own edits or by someone else's.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Cell {
        row: RowId,
        column: ColumnId,
        old: Value,
        new: Value,
    },
    Inserted {
        row: RowId,
    },
    Deleted {
        row: RowId,
        values: Vec<Value>,
    },
    /// Too much changed to list; fetch again.
    Reset,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceError {
    UnknownRow(RowId),
    UnknownColumn,
    WrongType(ColumnId),
    Limit(&'static str),
    /// The edit lost to another change.
    Conflict,
    Unsupported(&'static str),
    Failed(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRow(row) => write!(f, "unknown row {}", row.0),
            Self::UnknownColumn => f.write_str("unknown column"),
            Self::WrongType(column) => write!(f, "wrong type for column {}", column.as_str()),
            Self::Limit(which) => write!(f, "limit exceeded: {which}"),
            Self::Conflict => f.write_str("the edit conflicts with another change"),
            Self::Unsupported(what) => write!(f, "the source does not support {what}"),
            Self::Failed(why) => write!(f, "the source failed: {why}"),
        }
    }
}

impl std::error::Error for SourceError {}

/// Where a grid's rows come from. An in-memory table, a query against
/// the engine and a remote service all look like this.
pub trait DataSource {
    fn schema(&self) -> &[ColumnSpec];
    fn capabilities(&self) -> Capabilities;
    fn fetch(&mut self, request: &FetchRequest) -> Result<Page, SourceError>;
    /// Applies edits in order, all or nothing, and reports what
    /// changed.
    fn apply(&mut self, edits: &[Edit]) -> Result<Vec<Change>, SourceError>;
    /// Changes made since the last call, including other parties'.
    fn changes(&mut self) -> Vec<Change>;
}
