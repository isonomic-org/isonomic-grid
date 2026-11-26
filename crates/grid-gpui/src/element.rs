// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one element that paints a grid. It asks the layout which rows
//! and columns are visible, reads just those cells, and paints them.

use std::cell::RefCell;
use std::rc::Rc;

use grid_interact::Cell;
use grid_layout::{Axis, Band, GridLayout, Viewport};
use ui::{
    App, Bounds, ContentMask, CursorStyle, DispatchPhase, Element, ElementId, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Style, Window, fill, point, px, quad, relative,
    size,
};

use crate::lines::{CellKey, Face, Line, LineCache};
use crate::paint::{Brush, paint_band};
use crate::state::GridState;

/// The line between rows.
pub(crate) const HAIRLINE: Pixels = px(1.);
/// The caret, and the rule under a cell being edited.
pub(crate) const CARET: Pixels = px(2.);
/// The most lines of text never seen before that one frame shapes.
/// The rest are left for the frames that follow.
const NEW_LINES_PER_FRAME: usize = 256;
/// Cell padding, as a share of the root font size.
pub(crate) const PADDING_REMS: f32 = 0.375;

/// What the last frame laid out, kept for hit testing and paging.
#[derive(Default)]
pub struct Frame {
    pub layout: Option<GridLayout>,
    pub viewport: Option<Viewport>,
    pub origin: Point<Pixels>,
    /// Cells painted, header included.
    pub painted: usize,
    /// Rows in the scrolling band.
    pub page: usize,
    /// Shaped cell text, kept between frames.
    pub lines: LineCache,
    /// The lines under rows, one per row.
    rules: Vec<Bounds<Pixels>>,
}

pub type SharedFrame = Rc<RefCell<Frame>>;

#[derive(Clone, Copy)]
pub struct Colors {
    pub background: Hsla,
    pub foreground: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
    pub error: Hsla,
}

pub struct Painted {
    pub(crate) bounds: Bounds<Pixels>,
    /// Empty when `line` already holds the cell's shaped text.
    pub(crate) text: String,
    pub(crate) line: Option<Line>,
    /// The row and column a data cell shows, to keep its line by.
    pub(crate) key: Option<CellKey>,
    pub(crate) right: bool,
    /// Cells that share a number do not overlap and are painted
    /// together; a higher number is painted over a lower one.
    pub(crate) band: u8,
    /// A column title: drawn in the muted color.
    pub(crate) header: bool,
    /// In a pinned row or column, or the header: drawn over whatever
    /// scrolls beneath it, on its own background.
    pub(crate) opaque: bool,
    /// What the cell's outline may be drawn within, so that an active
    /// cell scrolled partly under a pinned band is cut off by it.
    pub(crate) within: Bounds<Pixels>,
    pub(crate) selected: bool,
    /// The cell the keyboard acts on.
    pub(crate) active: bool,
    /// Being edited: where the caret is, what is being composed, and
    /// whether the last commit was refused.
    pub(crate) editing: Option<EditPaint>,
}

pub(crate) struct EditPaint {
    pub(crate) caret: usize,
    pub(crate) marked: Option<std::ops::Range<usize>>,
    pub(crate) refused: bool,
}

pub struct GridElement {
    pub view: Entity<crate::view::GridView>,
    pub focus: ui::FocusHandle,
    pub state: Entity<GridState>,
    pub frame: SharedFrame,
    pub scroll: crate::scroll::GridScrollHandle,
    /// Whether a column edge is under the pointer or being dragged.
    pub resizing: bool,
    pub colors: Colors,
}

impl IntoElement for GridElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// The cells of each band along one axis: index, start and size.
fn slots(band: &Band, axis: &Axis) -> Vec<(usize, f32, f32, bool)> {
    let mut at = band.start;
    band.indices
        .clone()
        .map(|index| {
            let size = axis.size(index);
            let slot = (index, at, size, band.pinned);
            at += size;
            slot
        })
        .collect()
}

impl Element for GridElement {
    type RequestLayoutState = ();
    type PrepaintState = (Vec<Painted>, Hitbox);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<Painted>, Hitbox) {
        let rem = f32::from(window.rem_size());
        let row_height = f32::from(window.line_height()) + 2.0 * PADDING_REMS * rem;
        let at = |x: f32, y: f32, w: f32, h: f32| {
            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h)))
        };
        let moved = self.scroll.take_moved();
        let mut rules = Vec::new();
        let mut frame = self.frame.borrow_mut();
        let lines = &mut frame.lines;
        let (cells, layout, viewport, page) = self.state.update(cx, |state, _| {
            // A scrollbar that was dragged since the last frame says
            // where to be; the layout below clamps what it asked for.
            if let Some(x) = moved.0 {
                state.scroll.0 = x.max(0.0);
            }
            if let Some(y) = moved.1 {
                state.scroll.1 = y.max(0.0);
            }
            let layout = GridLayout {
                rows: Axis::uniform(state.row_count(), row_height),
                columns: Axis::variable(state.widths().iter().map(|rems| rems * rem).collect()),
                row_pins: state.row_pins,
                column_pins: state.column_pins,
                header: row_height,
            };
            let viewport = Viewport {
                width: f32::from(bounds.size.width),
                height: f32::from(bounds.size.height),
                scroll_x: state.scroll.0,
                scroll_y: state.scroll.1,
            };
            let row_bands = layout.visible_rows(&viewport);
            let column_bands = layout.visible_columns(&viewport);
            let page = row_bands[1].indices.len();
            let columns: Vec<_> =
                column_bands.iter().flat_map(|band| slots(band, &layout.columns)).collect();
            let rows: Vec<_> =
                row_bands.iter().flat_map(|band| slots(band, &layout.rows)).collect();
            let mut cells = Vec::with_capacity((rows.len() + 1) * columns.len());
            // Scrolling cells first, pinned ones after, so pinned
            // cells paint over what scrolls beneath them.
            let right_edge = columns.iter().map(|(_, x, width, _)| x + width).fold(0.0, f32::max);
            // The part of the view that scrolls, between the pinned bands.
            let ends = |bands: &[Band; 3], axis: &Axis, start: f32, extent: f32| {
                let lead =
                    slots(&bands[0], axis).last().map_or(start, |(_, at, size, _)| at + size);
                let tail = if bands[2].indices.is_empty() { extent } else { bands[2].start };
                (lead, tail.max(lead))
            };
            let (left, right) = ends(&column_bands, &layout.columns, 0.0, viewport.width);
            let (top, bottom) = ends(&row_bands, &layout.rows, row_height, viewport.height);
            let clip = |column_pinned: bool, row_pinned: bool| {
                let (x, width) =
                    if column_pinned { (0.0, viewport.width) } else { (left, right - left) };
                let (y, height) =
                    if row_pinned { (0.0, viewport.height) } else { (top, bottom - top) };
                at(x, y, width, height)
            };
            // Read once what every cell would otherwise ask for again.
            let editing = state.editing().cloned();
            let selection = state.selection().clone();
            let numeric: Vec<bool> =
                columns.iter().map(|(column, ..)| state.is_numeric(*column)).collect();
            let groups: Vec<bool> = rows.iter().map(|(row, ..)| state.is_group(*row)).collect();
            let ids: Vec<_> = rows.iter().map(|(row, ..)| state.row_id(*row)).collect();
            lines.use_data(state.table().version());
            for layer in 0..=2 {
                for (((row, y, height, row_pinned), group), id) in
                    rows.iter().zip(&groups).zip(&ids)
                {
                    for (position, (column, x, width, column_pinned)) in columns.iter().enumerate()
                    {
                        if u8::from(*row_pinned) + u8::from(*column_pinned) != layer {
                            continue;
                        }
                        // A group row is one cell as wide as the grid.
                        if *group && position > 0 {
                            continue;
                        }
                        let width = if *group { right_edge - x } else { *width };
                        let cell = Cell { row: *row, column: *column };
                        let editing = editing.as_ref().filter(|editing| editing.cell == cell);
                        // A cell drawn a moment ago still has its line;
                        // its text is not even read again.
                        let key = match (editing, id, state.columns.get(*column)) {
                            (None, Some(id), Some(schema)) => Some((*id, *schema)),
                            _ => None,
                        };
                        let line = key.and_then(|key| lines.cell(key));
                        let mut text = match (editing, &line) {
                            (Some(editing), _) => editing.text().to_owned(),
                            (None, Some(_)) => String::new(),
                            (None, None) => state.cell_text(*row, *column),
                        };
                        if text.contains(['\n', '\r']) {
                            text = text.replace(['\n', '\r'], " ");
                        }
                        cells.push(Painted {
                            editing: editing.map(|editing| EditPaint {
                                caret: editing.line.caret(),
                                marked: editing.line.marked(),
                                refused: editing.error.is_some(),
                            }),
                            bounds: at(*x, *y, width, *height),
                            text,
                            line,
                            key,
                            right: numeric[position] && !*group && editing.is_none(),
                            band: layer,
                            header: false,
                            opaque: *row_pinned || *column_pinned,
                            within: clip(*column_pinned, *row_pinned),
                            selected: !*group && selection.contains(cell),
                            active: !*group && selection.active == cell,
                        });
                    }
                }
            }
            // One line under each row, kept out from under pinned rows.
            let hairline = f32::from(HAIRLINE);
            let (lead_end, tail_start) = (top, bottom);
            let across = right_edge.min(viewport.width);
            rules.push(at(0.0, row_height - hairline, across, hairline));
            // A line along each pinned band's inner edge, where the rest
            // scrolls under it.
            let down = viewport.height.min(row_height + layout.rows.total() as f32);
            if !column_bands[0].indices.is_empty() {
                rules.push(at(left - hairline, 0.0, hairline, down));
            }
            if !column_bands[2].indices.is_empty() {
                rules.push(at(right, 0.0, hairline, down));
            }
            if !row_bands[2].indices.is_empty() {
                rules.push(at(0.0, bottom - hairline, across, hairline));
            }
            for (_, y, height, pinned) in &rows {
                let line = y + height - hairline;
                if *pinned || (line >= lead_end && line < tail_start) {
                    rules.push(at(0.0, line, across, hairline));
                }
            }
            // Titles: those of scrolling columns first, then pinned ones
            // over them.
            for pinned in [false, true] {
                for (column, x, width, _) in columns.iter().filter(|slot| slot.3 == pinned) {
                    cells.push(Painted {
                        bounds: at(*x, 0.0, *width, row_height),
                        text: state.header_text(*column),
                        line: None,
                        key: None,
                        right: state.is_numeric(*column),
                        band: 3 + u8::from(pinned),
                        header: true,
                        opaque: true,
                        within: clip(pinned, true),
                        selected: false,
                        active: false,
                        editing: None,
                    });
                }
            }
            (cells, layout, viewport, page)
        });
        let body = Bounds::new(
            bounds.origin + point(px(0.), px(layout.header)),
            size(bounds.size.width, (bounds.size.height - px(layout.header)).max(px(0.))),
        );
        self.scroll.record(body, &layout, &viewport);
        frame.painted = cells.len();
        frame.rules = rules;
        frame.page = page;
        frame.origin = bounds.origin;
        frame.layout = Some(layout);
        frame.viewport = Some(viewport);
        (cells, window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        (cells, hitbox): &mut (Vec<Painted>, Hitbox),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.resizing {
            window.set_cursor_style(CursorStyle::ResizeColumn, hitbox);
        }
        let colors = self.colors;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let padding = window.rem_size() * PADDING_REMS;
        let brush = Brush { colors, font: style.font(), font_size, line_height, padding };
        // A drag of a column edge follows the pointer wherever it goes,
        // so the view hears every move and release in the window.
        let view = self.view.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                view.update(cx, |view, cx| view.mouse_move(event, window, cx));
            }
        });
        let view = self.view.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                view.update(cx, |view, cx| view.mouse_up(cx));
            }
        });
        let mut frame = self.frame.borrow_mut();
        let frame = &mut *frame;
        frame.lines.set_face(Face {
            font: style.font(),
            size: font_size,
            text: colors.foreground,
            header: colors.muted,
        });
        frame.lines.begin_frame(NEW_LINES_PER_FRAME, cells.len());
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, colors.background));
            for band in cells.chunk_by(|one, next| one.band == next.band) {
                paint_band(band, &mut frame.lines, &brush, bounds, window);
            }
            frame.lines.end_frame();
            if frame.lines.deferred() > 0 {
                // Some text is still to be shaped: draw again.
                window.request_animation_frame();
            }
            for rule in &frame.rules {
                window.paint_quad(fill(*rule, colors.border));
            }
            // The active cell's outline goes over the rules around it.
            let focused = self.focus.is_focused(window);
            for cell in cells.iter().filter(|cell| cell.active && cell.editing.is_none()) {
                let color = if focused { colors.caret } else { colors.muted };
                let clear = ui::transparent_black();
                let outline = quad(cell.bounds, px(0.), clear, CARET, color, Default::default());
                let mask = Some(ContentMask { bounds: cell.within.intersect(&bounds) });
                window.with_content_mask(mask, |window| window.paint_quad(outline));
            }
            let handler = ui::ElementInputHandler::new(bounds, self.view.clone());
            window.handle_input(&self.focus, handler, cx);
        });
    }
}
