// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Values, typed columns, the columnar table, the view query and the
//! data source contract.
//!
//! This crate is headless and never panics on input.

mod column;
mod query;
mod source;
mod table;
mod text;
mod value;

pub use column::{Column, ColumnId, ColumnSpec, ColumnType};
pub use query::{
    Aggregate, AggregateFn, Compare, Direction, Expansion, Filter, SortKey, ViewQuery,
};
pub use source::{
    Capabilities, Change, Count, DataSource, Edit, FetchRequest, Page, PageRequest, PageRow,
    SourceError,
};
pub use table::{Limits, RowId, Table};
pub use text::TextColumn;
pub use value::{Decimal, Value, compare, fold};
