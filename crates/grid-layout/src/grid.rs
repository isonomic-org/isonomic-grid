use std::ops::Range;

use crate::Axis;

/// How many items are pinned at each end of an axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pins {
    pub leading: usize,
    pub trailing: usize,
}

/// The visible area and how far it is scrolled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A run of visible items along one axis: pinned at the start,
/// scrolling, or pinned at the end.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub indices: Range<usize>,
    /// Where the first item sits, in view coordinates.
    pub start: f32,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Header { column: usize },
    Cell { row: usize, column: usize },
    Outside,
}

/// One axis with its pins, measured against a viewport extent.
struct Track<'a> {
    axis: &'a Axis,
    pins: Pins,
    extent: f32,
    scroll: f64,
}

impl Track<'_> {
    fn count(&self) -> usize {
        self.axis.count()
    }

    fn leading(&self) -> usize {
        self.pins.leading.min(self.count())
    }

    /// The first trailing-pinned item.
    fn tail(&self) -> usize {
        self.count() - self.pins.trailing.min(self.count() - self.leading())
    }

    fn lead_size(&self) -> f64 {
        self.axis.offset(self.leading())
    }

    fn tail_size(&self) -> f64 {
        self.axis.total() - self.axis.offset(self.tail())
    }

    /// Where the trailing pinned band starts in the view.
    fn tail_start(&self) -> f64 {
        (f64::from(self.extent) - self.tail_size())
            .max(self.lead_size())
            .min(self.axis.offset(self.tail()))
    }

    fn max_scroll(&self) -> f64 {
        let room = f64::from(self.extent) - self.lead_size() - self.tail_size();
        (self.axis.offset(self.tail()) - self.lead_size() - room.max(0.0)).max(0.0)
    }

    fn scroll(&self) -> f64 {
        self.scroll.clamp(0.0, self.max_scroll())
    }

    fn bands(&self) -> [Band; 3] {
        let (lead, tail) = (self.leading(), self.tail());
        let from = self.lead_size() + self.scroll();
        let to = from + (self.tail_start() - self.lead_size());
        let mut middle = self.axis.range(from, to);
        middle.start = middle.start.clamp(lead, tail);
        middle.end = middle.end.clamp(middle.start, tail);
        let middle_start = (self.axis.offset(middle.start) - self.scroll()) as f32;
        [
            Band { indices: 0..lead, start: 0.0, pinned: true },
            Band { indices: middle, start: middle_start, pinned: false },
            Band { indices: tail..self.count(), start: self.tail_start() as f32, pinned: true },
        ]
    }

    /// The item under a view coordinate.
    fn locate(&self, at: f32) -> Option<usize> {
        let at = f64::from(at);
        if at < 0.0 || at >= f64::from(self.extent) || self.count() == 0 {
            return None;
        }
        let (lead, tail) = (self.leading(), self.tail());
        if at < self.lead_size() {
            return self.axis.index_at(at);
        }
        if at >= self.tail_start() && tail < self.count() {
            let offset = self.axis.offset(tail) + at - self.tail_start();
            return (offset < self.axis.total()).then(|| self.axis.index_at(offset)).flatten();
        }
        let offset = at + self.scroll();
        let index = self.axis.index_at(offset)?;
        (index >= lead && index < tail && offset < self.axis.offset(tail)).then_some(index)
    }

    /// Where an item sits in the view, and its size.
    fn place(&self, index: usize) -> Option<(f32, f32)> {
        if index >= self.count() {
            return None;
        }
        let offset = self.axis.offset(index);
        let start = if index < self.leading() {
            offset
        } else if index >= self.tail() {
            self.tail_start() + offset - self.axis.offset(self.tail())
        } else {
            offset - self.scroll()
        };
        Some((start as f32, self.axis.size(index)))
    }

    /// The scroll offset that brings a scrolling item fully into view
    /// with the least movement.
    fn reveal(&self, index: usize) -> f64 {
        if index < self.leading() || index >= self.tail() {
            return self.scroll();
        }
        let start = self.axis.offset(index) - self.lead_size();
        let end = start + f64::from(self.axis.size(index));
        let room = (self.tail_start() - self.lead_size()).max(0.0);
        let scroll = self.scroll();
        let next = if start < scroll {
            start
        } else if end > scroll + room {
            end - room
        } else {
            scroll
        };
        next.clamp(0.0, self.max_scroll())
    }
}

/// A grid's geometry: two axes, their pins and a header row.
#[derive(Clone, Debug)]
pub struct GridLayout {
    pub rows: Axis,
    pub columns: Axis,
    pub row_pins: Pins,
    pub column_pins: Pins,
    pub header: f32,
}

impl GridLayout {
    fn row_track(&self, view: &Viewport) -> Track<'_> {
        let extent = (view.height - self.header).max(0.0);
        Track { axis: &self.rows, pins: self.row_pins, extent, scroll: view.scroll_y }
    }

    fn column_track(&self, view: &Viewport) -> Track<'_> {
        Track {
            axis: &self.columns,
            pins: self.column_pins,
            extent: view.width,
            scroll: view.scroll_x,
        }
    }

    /// Visible rows as three bands. Positions are below the header.
    pub fn visible_rows(&self, view: &Viewport) -> [Band; 3] {
        let mut bands = self.row_track(view).bands();
        for band in &mut bands {
            band.start += self.header;
        }
        bands
    }

    pub fn visible_columns(&self, view: &Viewport) -> [Band; 3] {
        self.column_track(view).bands()
    }

    /// The largest scroll offsets, horizontal then vertical.
    pub fn max_scroll(&self, view: &Viewport) -> (f64, f64) {
        (self.column_track(view).max_scroll(), self.row_track(view).max_scroll())
    }

    pub fn hit(&self, view: &Viewport, x: f32, y: f32) -> Hit {
        let Some(column) = self.column_track(view).locate(x) else { return Hit::Outside };
        if y >= 0.0 && y < self.header {
            return Hit::Header { column };
        }
        match self.row_track(view).locate(y - self.header) {
            Some(row) => Hit::Cell { row, column },
            None => Hit::Outside,
        }
    }

    /// Where a cell sits in the view. It may lie outside the viewport.
    pub fn cell_rect(&self, view: &Viewport, row: usize, column: usize) -> Option<Rect> {
        let (y, height) = self.row_track(view).place(row)?;
        let (x, width) = self.column_track(view).place(column)?;
        Some(Rect { x, y: y + self.header, width, height })
    }

    /// The scroll offsets that bring a cell into view, horizontal
    /// then vertical.
    pub fn reveal(&self, view: &Viewport, row: usize, column: usize) -> (f64, f64) {
        (self.column_track(view).reveal(column), self.row_track(view).reveal(row))
    }
}
