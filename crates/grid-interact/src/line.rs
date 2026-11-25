// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

/// One line of text being edited in a cell: the text, a caret, and the
/// part an input method is still composing. Offsets are UTF-8 bytes
/// and always fall between characters; anything else is moved back to
/// the nearest one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineEdit {
    text: String,
    caret: usize,
    marked: Option<Range<usize>>,
}

impl LineEdit {
    /// Starts with the caret after the text.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self { caret: text.len(), text, marked: None }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The text an input method is composing, if any.
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// The nearest character boundary at or before `at`.
    fn floor(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    fn range(&self, range: Range<usize>) -> Range<usize> {
        let start = self.floor(range.start);
        start..self.floor(range.end).max(start)
    }

    /// Replaces a range with text and puts the caret after it. Without
    /// a range, the text being composed is replaced, or else the text
    /// goes in at the caret. With `mark`, the new text becomes what is
    /// being composed; otherwise composition ends.
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str, mark: bool) {
        let range = match range {
            Some(range) => self.range(range),
            None => self.marked.clone().unwrap_or(self.caret..self.caret),
        };
        let range = self.range(range);
        self.text.replace_range(range.clone(), text);
        self.caret = range.start + text.len();
        self.marked = (mark && !text.is_empty()).then_some(range.start..self.caret);
    }

    /// Types text at the caret, over anything being composed.
    pub fn insert(&mut self, text: &str) {
        self.replace(None, text, false);
    }

    /// Ends composition and keeps what was composed.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    fn before(&self) -> usize {
        self.text[..self.caret].char_indices().next_back().map_or(0, |(at, _)| at)
    }

    fn after(&self) -> usize {
        self.text[self.caret..].chars().next().map_or(self.caret, |c| self.caret + c.len_utf8())
    }

    /// Removes the character before the caret. `false` at the start.
    pub fn backspace(&mut self) -> bool {
        let from = self.before();
        let removed = from < self.caret;
        self.replace(Some(from..self.caret), "", false);
        removed
    }

    /// Removes the character after the caret. `false` at the end.
    pub fn delete(&mut self) -> bool {
        let to = self.after();
        let removed = to > self.caret;
        self.replace(Some(self.caret..to), "", false);
        removed
    }

    pub fn move_left(&mut self) {
        self.caret = self.before();
        self.marked = None;
    }

    pub fn move_right(&mut self) {
        self.caret = self.after();
        self.marked = None;
    }

    pub fn move_to_start(&mut self) {
        self.caret = 0;
        self.marked = None;
    }

    pub fn move_to_end(&mut self) {
        self.caret = self.text.len();
        self.marked = None;
    }

    /// Puts the caret at a byte offset, moved back to a boundary.
    pub fn move_to(&mut self, at: usize) {
        self.caret = self.floor(at);
        self.marked = None;
    }
}
