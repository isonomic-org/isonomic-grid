// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one element that paints a grid. It asks the layout which rows
//! and columns are visible, reads just those cells, and paints them.

use std::cell::RefCell;
use std::rc::Rc;

use grid_interact::Cell;
use grid_layout::{Axis, Band, GridLayout, Viewport};
use ui::{
    App, Bounds, ContentMask, DispatchPhase, Element, ElementId, Entity, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    SharedString, Style, TextAlign, TextRun, Window, fill, point, px, relative, size,
};

use crate::state::GridState;

/// The line between rows.
const HAIRLINE: Pixels = px(1.);
/// The caret, and the rule under a cell being edited.
const CARET: Pixels = px(2.);
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
    /// Being edited: paint a caret after the text, and whether the
    /// last commit was refused.
    editing: Option<bool>,
}

pub struct GridElement {
    pub view: Entity<crate::view::GridView>,
    pub focus: ui::FocusHandle,
    pub state: Entity<GridState>,
    pub frame: SharedFrame,
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
    type PrepaintState = Vec<Painted>;

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
    ) -> Vec<Painted> {
        let rem = f32::from(window.rem_size());
        let row_height = f32::from(window.line_height()) + 2.0 * PADDING_REMS * rem;
        let at = |x: f32, y: f32, w: f32, h: f32| {
            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h)))
        };
        let (cells, layout, viewport, page) = self.state.update(cx, |state, _| {
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
                            Some(editing) => editing.text.clone(),
                            None => state.cell_text(*row, *column),
                        };
                        cells.push(Painted {
                            editing: editing.map(|editing| editing.error.is_some()),
                            bounds: at(*x, *y, width, *height),
                            text: text.replace(['\n', '\r'], " ").into(),
                            right: state.is_numeric(*column)
                                && !group
                                && state.editing().is_none_or(|e| e.cell != cell),
                            header: *row_pinned || *column_pinned,
                            selected: !group && state.selection().contains(cell),
                        });
                    }
                }
            }
            for (column, x, width, _) in &columns {
                cells.push(Painted {
                    bounds: at(*x, 0.0, *width, row_height),
                    text: state.header_text(*column).into(),
                    right: state.is_numeric(*column),
                    header: true,
                    selected: false,
                    editing: None,
                });
            }
            (cells, layout, viewport, page)
        });
        let mut frame = self.frame.borrow_mut();
        frame.painted = cells.len();
        frame.page = page;
        frame.origin = bounds.origin;
        frame.layout = Some(layout);
        frame.viewport = Some(viewport);
        cells
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        cells: &mut Vec<Painted>,
        window: &mut Window,
        cx: &mut App,
    ) {
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
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, colors.background));
            for cell in cells.iter() {
                if cell.header {
                    window.paint_quad(fill(cell.bounds, colors.background));
                }
                if cell.selected {
                    window.paint_quad(fill(cell.bounds, colors.selection));
                }
                let rule = Bounds::new(
                    cell.bounds.origin + point(px(0.), cell.bounds.size.height - HAIRLINE),
                    size(cell.bounds.size.width, HAIRLINE),
                );
                window.paint_quad(fill(rule, colors.border));
                let origin = cell.bounds.origin + point(padding, padding);
                let mut caret_x = origin.x;
                if !cell.text.is_empty() {
                    let color = if cell.header { colors.muted } else { colors.foreground };
                    let run = TextRun {
                        len: cell.text.len(),
                        font: style.font(),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line =
                        window.text_system().shape_line(cell.text.clone(), font_size, &[run], None);
                    caret_x += line.width;
                    let inner = cell.bounds.size.width - padding * 2.0;
                    let align = if cell.right { TextAlign::Right } else { TextAlign::Left };
                    window.with_content_mask(Some(ContentMask { bounds: cell.bounds }), |window| {
                        let _ = line.paint(origin, line_height, align, Some(inner), window, cx);
                    });
                }
                if let Some(refused) = cell.editing {
                    let caret = Bounds::new(point(caret_x, origin.y), size(CARET, line_height));
                    window.paint_quad(fill(caret, colors.caret));
                    let mark = Bounds::new(
                        cell.bounds.origin + point(px(0.), cell.bounds.size.height - CARET),
                        size(cell.bounds.size.width, CARET),
                    );
                    window
                        .paint_quad(fill(mark, if refused { colors.error } else { colors.caret }));
                }
            }
            let handler = ui::ElementInputHandler::new(bounds, self.view.clone());
            window.handle_input(&self.focus, handler, cx);
        });
    }
}
