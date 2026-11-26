// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shaped cell text, kept between frames.
//!
//! Shaping a line is the expensive part of drawing a cell. The toolkit
//! keeps the lines of the previous frame; this keeps a few thousand
//! more, so scrolling back over rows seen a moment ago shapes nothing.
//!
//! A frame shapes only so many new lines. A jump to rows never seen
//! draws what it can, leaves the rest of the cells empty, and fills
//! them in over the next few frames, so no single frame stalls.

use std::collections::HashMap;
use std::rc::Rc;

use grid_core::RowId;
use ui::{Font, Hsla, Pixels, ShapedLine, SharedString};

/// The fewest lines kept in each of the two generations.
const GENERATION: usize = 4096;
/// A generation holds at least this many screens of cells, so paging
/// back and forth between two screens never drops either.
const SCREENS: usize = 3;

/// A shaped line is a few kilobytes wide, most of it room for
/// decorations a cell never has, so the caches share each one and hand
/// out the pointer.
pub type Line = Rc<ShapedLine>;
type Lines = HashMap<SharedString, Line>;

/// A data cell: its row, and its column's place in the table.
pub type CellKey = (RowId, usize);
type Cells = HashMap<CellKey, Line>;

/// What every line in the cache was shaped with.
#[derive(Clone, PartialEq)]
pub struct Face {
    pub font: Font,
    pub size: Pixels,
    pub text: Hsla,
    pub header: Hsla,
}

/// A bounded cache in two generations: a line used again moves to the
/// newer one, and when that fills the older one is dropped. Column
/// titles are shaped in another color and kept apart.
#[derive(Default)]
pub struct LineCache {
    face: Option<Face>,
    newer: Lines,
    older: Lines,
    titles: Lines,
    /// The lines of data cells by row and column, in two generations
    /// like the text, so a cell drawn again is found without its
    /// text being read or formatted. They hold for one version of the
    /// table's data.
    newer_cells: Cells,
    older_cells: Cells,
    data: Option<u64>,
    /// Lines served from the cache, and lines shaped, since the last
    /// call to [`LineCache::take_counts`].
    hits: usize,
    misses: usize,
    /// New lines this frame may still shape, and how many it had to
    /// leave for the next frame.
    budget: usize,
    allowed: usize,
    deferred: usize,
    /// The most lines one frame has shaped.
    peak: usize,
    /// Lines a generation holds before the older one is dropped.
    room: usize,
}

impl LineCache {
    /// Forgets every line if the font, size or colors changed.
    pub fn set_face(&mut self, face: Face) {
        if self.face.as_ref() != Some(&face) {
            self.newer.clear();
            self.older.clear();
            self.titles.clear();
            self.newer_cells.clear();
            self.older_cells.clear();
            self.face = Some(face);
        }
    }

    /// The font, size and colors the last frame drew with.
    pub fn face(&self) -> Option<&Face> {
        self.face.as_ref()
    }

    /// Says which version of the table's data is about to be drawn.
    /// The lines kept by cell are dropped when it is a new one; the
    /// lines kept by text stay, so little is shaped again.
    pub fn use_data(&mut self, version: u64) {
        if self.data != Some(version) {
            self.newer_cells.clear();
            self.older_cells.clear();
            self.data = Some(version);
        }
    }

    /// The line a data cell was last drawn with, if it is still kept.
    pub fn cell(&mut self, key: CellKey) -> Option<Line> {
        if let Some(line) = self.newer_cells.get(&key) {
            self.hits += 1;
            return Some(line.clone());
        }
        let line = self.older_cells.remove(&key)?;
        self.hits += 1;
        self.keep(key, line.clone());
        Some(line)
    }

    /// The text of the line a data cell was last drawn with.
    pub fn drawn(&self, key: CellKey) -> Option<&str> {
        let line = self.newer_cells.get(&key).or_else(|| self.older_cells.get(&key))?;
        Some(line.text.as_ref())
    }

    /// Keeps the line a data cell was drawn with.
    pub fn keep(&mut self, key: CellKey, line: Line) {
        if self.newer_cells.len() >= self.room.max(GENERATION) {
            self.older_cells = std::mem::take(&mut self.newer_cells);
        }
        self.newer_cells.insert(key, line);
    }

    /// Starts a frame of `cells` cells, which may shape at most
    /// `budget` new lines.
    pub fn begin_frame(&mut self, budget: usize, cells: usize) {
        self.room = GENERATION.max(cells.saturating_mul(SCREENS));
        self.budget = budget;
        self.allowed = budget;
        self.deferred = 0;
    }

    /// Ends a frame, noting how many lines it shaped.
    pub fn end_frame(&mut self) {
        self.peak = self.peak.max(self.allowed - self.budget);
    }

    /// The most lines any one frame shaped since the last call.
    pub fn take_peak(&mut self) -> usize {
        std::mem::take(&mut self.peak)
    }

    /// How many lines this frame left for the next one to shape.
    pub fn deferred(&self) -> usize {
        self.deferred
    }

    /// The shaped line for a cell's text, shaping it only if the cache
    /// does not hold it. `None` when it would have to be shaped and
    /// this frame has already shaped all it may.
    pub fn line(
        &mut self,
        text: &str,
        header: bool,
        shape: impl FnOnce(SharedString) -> ShapedLine,
    ) -> Option<Line> {
        let held = if header { &self.titles } else { &self.newer };
        if let Some(line) = held.get(text) {
            self.hits += 1;
            return Some(line.clone());
        }
        let older = if header { None } else { self.older.remove_entry(text) };
        let (key, line) = match older {
            Some(entry) => {
                self.hits += 1;
                entry
            }
            None if self.budget == 0 => {
                self.deferred += 1;
                return None;
            }
            None => {
                self.budget -= 1;
                self.misses += 1;
                let key = SharedString::from(text.to_owned());
                (key.clone(), Rc::new(shape(key)))
            }
        };
        let held = if header { &mut self.titles } else { &mut self.newer };
        if held.len() >= self.room {
            let full = std::mem::take(held);
            if !header {
                self.older = full;
            }
        }
        let held = if header { &mut self.titles } else { &mut self.newer };
        held.insert(key, line.clone());
        Some(line)
    }

    /// Hits and misses since the last call.
    pub fn take_counts(&mut self) -> (usize, usize) {
        (std::mem::take(&mut self.hits), std::mem::take(&mut self.misses))
    }
}
