// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform text input for the cell being edited. Typing with no edit
//! open starts one that replaces the cell. The platform counts in
//! UTF-16 units; the line counts in bytes, and the two meet only here.

use std::ops::Range;

use ui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use crate::view::GridView;

/// The UTF-16 offset of a byte offset, rounded down to a character.
fn to_utf16(text: &str, byte: usize) -> usize {
    let mut at = byte.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    text[..at].encode_utf16().count()
}

/// The byte offset of a UTF-16 offset, cut to the text.
fn from_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (at, ch) in text.char_indices() {
        if seen >= units {
            return at;
        }
        seen += ch.len_utf16();
    }
    text.len()
}

fn bytes(text: &str, range: Range<usize>) -> Range<usize> {
    from_utf16(text, range.start)..from_utf16(text, range.end)
}

fn units(text: &str, range: Range<usize>) -> Range<usize> {
    to_utf16(text, range.start)..to_utf16(text, range.end)
}

impl GridView {
    /// Replaces part of the line being edited, starting an edit of the
    /// active cell if there is none.
    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        mark: bool,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            // A space on a yes-or-no cell flips it instead of typing.
            if text == " " && !mark && state.editing().is_none() && state.toggle_cell(cx) {
                return;
            }
            if state.editing().is_none() {
                state.begin_edit(Some(""), cx);
            }
            state.edit_line(
                |line| {
                    let range = range.map(|range| bytes(line.text(), range));
                    line.replace(range, text, mark);
                },
                cx,
            );
        });
    }
}

impl EntityInputHandler for GridView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let state = self.state.read(cx);
        let text = state.editing()?.text();
        let range = bytes(text, range);
        *adjusted = Some(units(text, range.clone()));
        Some(text.get(range)?.to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let state = self.state.read(cx);
        let at = state.editing().map_or(0, |edit| to_utf16(edit.text(), edit.line.caret()));
        Some(UTF16Selection { range: at..at, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let editing = self.state.read(cx).editing()?;
        Some(units(editing.text(), editing.line.marked()?))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.edit_line(|line| line.unmark(), cx));
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, false, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, true, cx);
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let active = self.state.read(cx).selection().active;
        self.cell_bounds(active.row, active.column)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{from_utf16, to_utf16};

    #[test]
    fn offsets_convert_and_never_split_a_character() {
        let text = "a😀é";
        for byte in [0, 1, 5, 7] {
            assert_eq!(from_utf16(text, to_utf16(text, byte)), byte);
        }
        assert_eq!(to_utf16(text, 3), 1, "inside a character rounds down");
        assert_eq!(to_utf16(text, 99), 4);
        assert_eq!(from_utf16(text, 99), text.len());
    }
}
