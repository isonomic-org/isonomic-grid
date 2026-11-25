//! Interaction state for a grid, with no drawing: the active cell and
//! selected ranges, keyboard navigation, row selection that scales to
//! windowed sources, and an edit log for undo and redo.

mod log;
mod rows;
mod selection;

pub use log::EditLog;
pub use rows::RowSelection;
pub use selection::{Cell, CellRange, Move, Selection};
