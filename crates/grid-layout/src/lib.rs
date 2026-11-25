// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Geometry for a grid: axes of row heights and column widths, pinned
//! regions, scrolling in 64-bit coordinates and hit testing.
//!
//! Nothing here draws. A view asks which rows and columns are visible
//! and where they sit, and paints only those.

mod axis;
mod grid;
mod scroll;

pub use axis::Axis;
pub use grid::{Band, GridLayout, Hit, Pins, Rect, Viewport};
pub use scroll::{ScrollAnchor, ScrollScale};
