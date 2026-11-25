/// A cell by its position in the view: row in the flattened result,
/// column among the visible columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cell {
    pub row: usize,
    pub column: usize,
}

/// A rectangle of cells between an anchor and a moving end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRange {
    pub anchor: Cell,
    pub head: Cell,
}

impl CellRange {
    pub fn single(cell: Cell) -> Self {
        Self { anchor: cell, head: cell }
    }

    /// First and last row, inclusive.
    pub fn rows(&self) -> (usize, usize) {
        (self.anchor.row.min(self.head.row), self.anchor.row.max(self.head.row))
    }

    /// First and last column, inclusive.
    pub fn columns(&self) -> (usize, usize) {
        (self.anchor.column.min(self.head.column), self.anchor.column.max(self.head.column))
    }

    pub fn contains(&self, cell: Cell) -> bool {
        let ((top, bottom), (left, right)) = (self.rows(), self.columns());
        (top..=bottom).contains(&cell.row) && (left..=right).contains(&cell.column)
    }

    pub fn cell_count(&self) -> usize {
        let ((top, bottom), (left, right)) = (self.rows(), self.columns());
        (bottom - top + 1).saturating_mul(right - left + 1)
    }
}

/// A keyboard movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    /// The first column of the row.
    Home,
    /// The last column of the row.
    End,
    FirstCell,
    LastCell,
    /// Tab: the next cell, wrapping to the next row.
    Next,
    /// Shift-Tab: the previous cell, wrapping to the previous row.
    Previous,
}

/// The active cell and the selected ranges. The last range is the
/// one the keyboard extends.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub active: Cell,
    pub ranges: Vec<CellRange>,
}

impl Selection {
    pub fn at(cell: Cell) -> Self {
        Self { active: cell, ranges: vec![CellRange::single(cell)] }
    }

    pub fn contains(&self, cell: Cell) -> bool {
        self.ranges.iter().any(|range| range.contains(cell))
    }

    /// A click: selects one cell, extends the last range (Shift), or
    /// starts another range (the platform's add-to-selection key).
    pub fn click(&mut self, cell: Cell, extend: bool, add: bool) {
        match (extend, self.ranges.last_mut()) {
            (true, Some(last)) => last.head = cell,
            _ => {
                if !add {
                    self.ranges.clear();
                }
                self.ranges.push(CellRange::single(cell));
                self.active = cell;
            }
        }
    }

    /// Extends the last range to a cell, as a drag does.
    pub fn drag_to(&mut self, cell: Cell) {
        if let Some(last) = self.ranges.last_mut() {
            last.head = cell;
        }
    }

    /// Selects every cell of a view with `rows` rows and `columns`
    /// columns.
    pub fn select_all(&mut self, rows: usize, columns: usize) {
        if rows == 0 || columns == 0 {
            return;
        }
        let head = Cell { row: rows - 1, column: columns - 1 };
        self.ranges = vec![CellRange { anchor: Cell::default(), head }];
    }

    /// Moves the active cell, or with `extend` the end of the last
    /// range. Returns the cell to scroll into view.
    pub fn navigate(
        &mut self,
        step: Move,
        rows: usize,
        columns: usize,
        page: usize,
        extend: bool,
    ) -> Cell {
        if rows == 0 || columns == 0 {
            return self.active;
        }
        let (last_row, last_column) = (rows - 1, columns - 1);
        let from = match (extend, self.ranges.last()) {
            (true, Some(last)) => last.head,
            _ => self.active,
        };
        let from = Cell { row: from.row.min(last_row), column: from.column.min(last_column) };
        let page = page.max(1);
        let to = match step {
            Move::Up => Cell { row: from.row.saturating_sub(1), ..from },
            Move::Down => Cell { row: (from.row + 1).min(last_row), ..from },
            Move::Left => Cell { column: from.column.saturating_sub(1), ..from },
            Move::Right => Cell { column: (from.column + 1).min(last_column), ..from },
            Move::PageUp => Cell { row: from.row.saturating_sub(page), ..from },
            Move::PageDown => Cell { row: from.row.saturating_add(page).min(last_row), ..from },
            Move::Home => Cell { column: 0, ..from },
            Move::End => Cell { column: last_column, ..from },
            Move::FirstCell => Cell::default(),
            Move::LastCell => Cell { row: last_row, column: last_column },
            Move::Next if from.column < last_column => Cell { column: from.column + 1, ..from },
            Move::Next if from.row < last_row => Cell { row: from.row + 1, column: 0 },
            Move::Previous if from.column > 0 => Cell { column: from.column - 1, ..from },
            Move::Previous if from.row > 0 => Cell { row: from.row - 1, column: last_column },
            Move::Next | Move::Previous => from,
        };
        if extend {
            match self.ranges.last_mut() {
                Some(last) => last.head = to,
                None => self.ranges.push(CellRange { anchor: self.active, head: to }),
            }
        } else {
            *self = Self::at(to);
        }
        to
    }
}
