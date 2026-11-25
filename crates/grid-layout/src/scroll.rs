use crate::Axis;

/// A scroll position held as an item and a distance into it, so the
/// view stays put when items above it change size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub index: usize,
    pub within: f32,
}

impl ScrollAnchor {
    pub fn at(axis: &Axis, offset: f64) -> Self {
        match axis.index_at(offset) {
            Some(index) => Self { index, within: (offset.max(0.0) - axis.offset(index)) as f32 },
            None => Self { index: 0, within: 0.0 },
        }
    }

    pub fn offset(&self, axis: &Axis) -> f64 {
        let index = self.index.min(axis.count());
        axis.offset(index) + f64::from(self.within.min(axis.size(index)))
    }
}

/// Maps a very long axis onto a scrollbar range a toolkit can hold.
/// Content offsets stay exact in 64 bits; only the thumb is scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollScale {
    content: f64,
    viewport: f64,
    limit: f64,
}

impl ScrollScale {
    /// `limit` is the longest content the toolkit's scrollbar takes.
    pub fn new(content: f64, viewport: f64, limit: f64) -> Self {
        Self { content: content.max(0.0), viewport: viewport.max(0.0), limit: limit.max(1.0) }
    }

    /// The content size to report to the scrollbar.
    pub fn scaled_content(&self) -> f64 {
        self.content.min(self.limit.max(self.viewport))
    }

    fn spans(&self) -> (f64, f64) {
        ((self.content - self.viewport).max(0.0), (self.scaled_content() - self.viewport).max(0.0))
    }

    pub fn to_scaled(&self, offset: f64) -> f64 {
        let (real, scaled) = self.spans();
        if real <= 0.0 { 0.0 } else { offset.clamp(0.0, real) / real * scaled }
    }

    pub fn from_scaled(&self, offset: f64) -> f64 {
        let (real, scaled) = self.spans();
        if scaled <= 0.0 { 0.0 } else { offset.clamp(0.0, scaled) / scaled * real }
    }
}
