// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interaction state for a grid, with no drawing: the active cell and
//! selected ranges, keyboard navigation, row selection that scales to
//! windowed sources, an edit log for undo and redo, and the line of text
//! being edited in a cell.

mod line;
mod log;
mod rows;
mod selection;

pub use line::LineEdit;
pub use log::EditLog;
pub use rows::RowSelection;
pub use selection::{Cell, CellRange, Move, Selection};
