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
    MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Style, TextAlign, TextRun, Window,
    fill, point, px, quad, relative, size,
};

use crate::lines::{Face, LineCache};
use crate::state::GridState;

/// The line between rows.
const HAIRLINE: Pixels = px(1.);
/// The caret, and the rule under a cell being edited.
const CARET: Pixels = px(2.);
/// The most lines of text never seen before that one frame shapes.
/// The rest are left for the frames that follow.
const NEW_LINES_PER_FRAME: usize = 256;
/// Cell padding, as a share of the root font size.
const PADDING_REMS: f32 = 0.375;

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
    bounds: Bounds<Pixels>,
    text: SharedString,
    right: bool,
    header: bool,
    selected: bool,
    /// The cell the keyboard acts on.
    active: bool,
    /// Being edited: where the caret is, what is being composed, and
    /// whether the last commit was refused.
    editing: Option<EditPaint>,
}

struct EditPaint {
    caret: usize,
    marked: Option<std::ops::Range<usize>>,
    refused: bool,
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
            for layer in 0..=2 {
                for (row, y, height, row_pinned) in &rows {
                    let group = state.is_group(*row);
                    for (position, (column, x, width, column_pinned)) in columns.iter().enumerate()
                    {
                        if u8::from(*row_pinned) + u8::from(*column_pinned) != layer {
                            continue;
                        }
                        // A group row is one cell as wide as the grid.
                        if group && position > 0 {
                            continue;
                        }
                        let width = if group { right_edge - x } else { *width };
                        let cell = Cell { row: *row, column: *column };
                        let editing =
                            state.editing().filter(|editing| editing.cell == cell).cloned();
                        let text = match &editing {
                            Some(editing) => editing.text().to_owned(),
                            None => state.cell_text(*row, *column),
                        };
                        cells.push(Painted {
                            editing: editing.map(|editing| EditPaint {
                                caret: editing.line.caret(),
                                marked: editing.line.marked(),
                                refused: editing.error.is_some(),
                            }),
                            bounds: at(*x, *y, width, *height),
                            text: text.replace(['\n', '\r'], " ").into(),
                            right: state.is_numeric(*column)
                                && !group
                                && state.editing().is_none_or(|e| e.cell != cell),
                            header: *row_pinned || *column_pinned,
                            selected: !group && state.selection().contains(cell),
                            active: !group && state.selection().active == cell,
                        });
                    }
                }
            }
            // One line under each row, kept out from under pinned rows.
            let hairline = f32::from(HAIRLINE);
            let lead_end = slots(&row_bands[0], &layout.rows)
                .last()
                .map_or(row_height, |(_, y, height, _)| y + height);
            let tail_start =
                if row_bands[2].indices.is_empty() { viewport.height } else { row_bands[2].start };
            let across = right_edge.min(viewport.width);
            rules.push(at(0.0, row_height - hairline, across, hairline));
            for (_, y, height, pinned) in &rows {
                let line = y + height - hairline;
                if *pinned || (line >= lead_end && line < tail_start) {
                    rules.push(at(0.0, line, across, hairline));
                }
            }
            for (column, x, width, _) in &columns {
                cells.push(Painted {
                    bounds: at(*x, 0.0, *width, row_height),
                    text: state.header_text(*column).into(),
                    right: state.is_numeric(*column),
                    header: true,
                    selected: false,
                    active: false,
                    editing: None,
                });
            }
            (cells, layout, viewport, page)
        });
        let body = Bounds::new(
            bounds.origin + point(px(0.), px(layout.header)),
            size(bounds.size.width, (bounds.size.height - px(layout.header)).max(px(0.))),
        );
        self.scroll.record(body, &layout, &viewport);
        let mut frame = self.frame.borrow_mut();
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
                view.update(cx, |view, _| view.mouse_up());
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
        frame.lines.begin_frame(NEW_LINES_PER_FRAME);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, colors.background));
            for cell in cells.iter() {
                if cell.header {
                    window.paint_quad(fill(cell.bounds, colors.background));
                }
                if cell.selected {
                    window.paint_quad(fill(cell.bounds, colors.selection));
                }
                let mut origin = cell.bounds.origin + point(padding, padding);
                let inner = cell.bounds.size.width - padding * 2.0;
                let mut caret_x = px(0.);
                let mut composed = None;
                let line = if cell.text.is_empty() {
                    None
                } else {
                    frame.lines.line(&cell.text, cell.header, || {
                        let run = TextRun {
                            len: cell.text.len(),
                            font: style.font(),
                            color: if cell.header { colors.muted } else { colors.foreground },
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        window.text_system().shape_line(cell.text.clone(), font_size, &[run], None)
                    })
                };
                if let Some(line) = line {
                    if let Some(edit) = &cell.editing {
                        caret_x = line.x_for_index(edit.caret.min(cell.text.len()));
                        // Text longer than the cell slides left so the
                        // caret stays in it.
                        origin.x -= (caret_x - inner + CARET).max(px(0.));
                        composed = edit.marked.as_ref().map(|marked| {
                            (line.x_for_index(marked.start), line.x_for_index(marked.end))
                        });
                    }
                    let align = if cell.right { TextAlign::Right } else { TextAlign::Left };
                    if line.width > inner || cell.editing.is_some() {
                        // Text wider than its cell is cut at the cell's edge.
                        let mask = Some(ContentMask { bounds: cell.bounds });
                        window.with_content_mask(mask, |window| {
                            let _ = line.paint(origin, line_height, align, Some(inner), window, cx);
                        });
                    } else {
                        let _ = line.paint(origin, line_height, align, Some(inner), window, cx);
                    }
                }
                if let Some(edit) = &cell.editing {
                    let caret =
                        Bounds::new(origin + point(caret_x, px(0.)), size(CARET, line_height));
                    window.paint_quad(fill(caret, colors.caret));
                    if let Some((from, to)) = composed {
                        // What an input method is still composing is underlined.
                        let under = Bounds::new(
                            origin + point(from, line_height - HAIRLINE),
                            size(to - from, HAIRLINE),
                        );
                        window.paint_quad(fill(under, colors.foreground));
                    }
                    let mark = Bounds::new(
                        cell.bounds.origin + point(px(0.), cell.bounds.size.height - CARET),
                        size(cell.bounds.size.width, CARET),
                    );
                    let color = if edit.refused { colors.error } else { colors.caret };
                    window.paint_quad(fill(mark, color));
                }
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
                window.paint_quad(quad(
                    cell.bounds,
                    px(0.),
                    clear,
                    CARET,
                    color,
                    Default::default(),
                ));
            }
            let handler = ui::ElementInputHandler::new(bounds, self.view.clone());
            window.handle_input(&self.focus, handler, cx);
        });
    }
}
