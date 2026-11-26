// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Painting the cells of one band: their backgrounds, then all their
//! text, then the caret of a cell being edited.
//!
//! The library paints a line of text inside a layer of its own, and
//! placing each layer among what is already painted is most of what a
//! line costs when there are thousands. The cells of a band do not
//! overlap, so their text goes into one layer, glyph by glyph.

use ui::{
    Bounds, ContentMask, Font, Hsla, Pixels, Point, ShapedLine, TextRun, Window, fill, point, px,
    size,
};

use crate::element::{CARET, Colors, HAIRLINE, Painted};
use crate::lines::{Line, LineCache};

/// What every cell of a frame is painted with.
pub(crate) struct Brush {
    pub colors: Colors,
    pub font: Font,
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub padding: Pixels,
}

/// Paints cells that share a band, so none overlaps another, within
/// `bounds`.
pub(crate) fn paint_band(
    cells: &[Painted],
    lines: &mut LineCache,
    brush: &Brush,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    for cell in cells {
        if cell.opaque {
            window.paint_quad(fill(cell.bounds, brush.colors.background));
        }
        if cell.selected {
            window.paint_quad(fill(cell.bounds, brush.colors.selection));
        }
    }
    // Where the text of a cell being edited was put, and its line.
    let mut edited = None;
    window.paint_layer(bounds, |window| {
        for cell in cells {
            let Some(line) = line_of(cell, lines, brush, window) else { continue };
            let mut origin = cell.bounds.origin + point(brush.padding, brush.padding);
            let inner = cell.bounds.size.width - brush.padding * 2.0;
            if let Some(edit) = &cell.editing {
                // Text longer than the cell slides left so the caret
                // stays in it.
                let caret = line.x_for_index(edit.caret.min(cell.text.len()));
                origin.x -= (caret - inner + CARET).max(px(0.));
                edited = Some((origin, line.clone()));
            } else if cell.right {
                origin.x += inner - line.width;
            }
            let color = if cell.header { brush.colors.muted } else { brush.colors.foreground };
            if line.width > inner || cell.editing.is_some() {
                // Text wider than its cell is cut at the cell's edge.
                let mask = Some(ContentMask { bounds: cell.bounds });
                window.with_content_mask(mask, |window| {
                    paint_glyphs(&line, origin, brush.line_height, color, window);
                });
            } else {
                paint_glyphs(&line, origin, brush.line_height, color, window);
            }
        }
    });
    for cell in cells {
        let Some(edit) = &cell.editing else { continue };
        let origin = cell.bounds.origin + point(brush.padding, brush.padding);
        let (origin, line) = edited.take().map_or((origin, None), |(at, line)| (at, Some(line)));
        let x = |index: usize| line.as_ref().map_or(px(0.), |line| line.x_for_index(index));
        let caret = Bounds::new(
            origin + point(x(edit.caret.min(cell.text.len())), px(0.)),
            size(CARET, brush.line_height),
        );
        window.paint_quad(fill(caret, brush.colors.caret));
        if let Some(marked) = &edit.marked {
            // What an input method is still composing is underlined.
            let (from, to) = (x(marked.start), x(marked.end));
            let under = Bounds::new(
                origin + point(from, brush.line_height - HAIRLINE),
                size(to - from, HAIRLINE),
            );
            window.paint_quad(fill(under, brush.colors.foreground));
        }
        let mark = Bounds::new(
            cell.bounds.origin + point(px(0.), cell.bounds.size.height - CARET),
            size(cell.bounds.size.width, CARET),
        );
        let color = if edit.refused { brush.colors.error } else { brush.colors.caret };
        window.paint_quad(fill(mark, color));
    }
}

/// A cell's shaped text: the line it was last drawn with, or one found
/// by its text, or a new one if this frame may still shape it.
fn line_of(
    cell: &Painted,
    lines: &mut LineCache,
    brush: &Brush,
    window: &mut Window,
) -> Option<Line> {
    if let Some(line) = &cell.line {
        return Some(line.clone());
    }
    if cell.text.is_empty() {
        return None;
    }
    let line = lines.line(&cell.text, cell.header, |text| {
        let run = TextRun {
            len: text.len(),
            font: brush.font.clone(),
            color: if cell.header { brush.colors.muted } else { brush.colors.foreground },
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window.text_system().shape_line(text, brush.font_size, &[run], None)
    })?;
    if let Some(key) = cell.key {
        lines.keep(key, line.clone());
    }
    Some(line)
}

/// Paints a line's glyphs from its top left corner, centered in the
/// line height as the library centers them.
fn paint_glyphs(
    line: &ShapedLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let top = (line_height - line.ascent - line.descent) / 2.;
    let baseline = origin + point(px(0.), top + line.ascent);
    for run in &line.runs {
        for glyph in &run.glyphs {
            let at = baseline + glyph.position;
            // A glyph that cannot be drawn is left out; the rest are.
            let _ = if glyph.is_emoji {
                window.paint_emoji(at, run.font_id, glyph.id, line.font_size)
            } else {
                window.paint_glyph(at, run.font_id, glyph.id, line.font_size, color)
            };
        }
    }
}
