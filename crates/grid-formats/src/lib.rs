// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delimited text in and out, and typed parsing of cell text.
//!
//! The clipboard carries tab-separated text so any spreadsheet can
//! read it; a file export is comma-separated. Both use the same
//! quoting rules.

mod date;
mod delimited;
mod import;
mod text;

pub use date::{date_from_days, datetime_from_micros, days_from_date, micros_from_datetime};
pub use delimited::{read_delimited, write_delimited};
pub use import::{import_delimited, infer_type, paste_edits};
pub use text::{ParseError, format_value, parse_value};
