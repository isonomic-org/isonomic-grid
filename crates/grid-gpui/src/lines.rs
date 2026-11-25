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

use ui::{Font, Hsla, Pixels, ShapedLine, SharedString};

/// Lines kept in each of the two generations.
const GENERATION: usize = 4096;

/// What every line in the cache was shaped with.
#[derive(Clone, PartialEq)]
pub struct Face {
    pub font: Font,
    pub size: Pixels,
    pub text: Hsla,
    pub header: Hsla,
}

/// A bounded cache in two generations: a line used again moves to the
/// newer one, and when that fills the older one is dropped.
#[derive(Default)]
pub struct LineCache {
    face: Option<Face>,
    newer: HashMap<(SharedString, bool), ShapedLine>,
    older: HashMap<(SharedString, bool), ShapedLine>,
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
}

impl LineCache {
    /// Forgets every line if the font, size or colors changed.
    pub fn set_face(&mut self, face: Face) {
        if self.face.as_ref() != Some(&face) {
            self.newer.clear();
            self.older.clear();
            self.face = Some(face);
        }
    }

    /// The font, size and colors the last frame drew with.
    pub fn face(&self) -> Option<&Face> {
        self.face.as_ref()
    }

    /// Starts a frame, which may shape at most `budget` new lines.
    pub fn begin_frame(&mut self, budget: usize) {
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

    /// The shaped line for a cell's text, shaping it only if neither
    /// generation holds it. `None` when it would have to be shaped and
    /// this frame has already shaped all it may.
    pub fn line(
        &mut self,
        text: &SharedString,
        header: bool,
        shape: impl FnOnce() -> ShapedLine,
    ) -> Option<&ShapedLine> {
        let key = (text.clone(), header);
        if !self.newer.contains_key(&key) {
            let line = match self.older.remove(&key) {
                Some(line) => {
                    self.hits += 1;
                    line
                }
                None if self.budget == 0 => {
                    self.deferred += 1;
                    return None;
                }
                None => {
                    self.budget -= 1;
                    self.misses += 1;
                    shape()
                }
            };
            if self.newer.len() >= GENERATION {
                self.older = std::mem::take(&mut self.newer);
            }
            self.newer.insert(key.clone(), line);
        } else {
            self.hits += 1;
        }
        self.newer.get(&key)
    }

    /// Hits and misses since the last call.
    pub fn take_counts(&mut self) -> (usize, usize) {
        (std::mem::take(&mut self.hits), std::mem::take(&mut self.misses))
    }
}
