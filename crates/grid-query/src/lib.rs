// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The query pipeline over an in-memory table: filter, search, sort,
//! group, aggregate, flatten.
//!
//! Semantics every executor shares: nulls sort last in both
//! directions; not-a-number sorts after numbers and before nulls;
//! ties break by row ID; text search ignores case and accents;
//! aggregates skip nulls.

mod aggregate;
mod bitmap;
mod exec;
mod filter;
mod keys;
mod memory;
mod radix;

pub use aggregate::aggregate;
pub use bitmap::Bitmap;
pub use exec::{ViewResult, ViewRow, execute};
pub use filter::{matching, search};
pub use memory::MemorySource;
