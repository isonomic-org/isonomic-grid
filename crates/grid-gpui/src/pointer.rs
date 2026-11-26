// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the pointer does in a grid: select, drag out a range, sort,
//! open groups, drag a column edge, and scroll.

use grid_interact::Cell;
use grid_layout::{Hit, Viewport};
use ui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ScrollEvent, TextRun,
    Window, point, px,
};

use crate::element::PADDING_REMS;
use crate::view::GridView;

/// How close to a column's right edge, in rems, a press in the header
/// takes hold of the edge instead of sorting.
const GRIP_REMS: f32 = 0.3125;

/// The most rows one frame scrolls while a drag holds the pointer past
/// an edge.
const MOST_ROWS_A_FRAME: f32 = 8.;
/// How far the pointer travels on a header, in rems, before a press
/// becomes a drag that moves the column.
const DRAG_SLACK_REMS: f32 = 0.5;
/// Room left beside the widest text when a column is fitted, in rems.
const FIT_SLACK_REMS: f32 = 0.25;

/// What the pressed pointer is doing.
pub(crate) enum Drag {
    /// Dragging a column's edge.
    Resize {
        column: usize,
        from: Pixels,
        /// The column's width when the drag began, in rems.
        width: f32,
    },
    /// Dragging out a range of cells; the pointer was last here.
    Select { at: Point<Pixels> },
    /// Pressed on a column's header: a click sorts by it, a drag moves
    /// it. `moved` is set once the pointer has gone far enough to be a
    /// drag.
    Column { column: usize, from: Pixels, moved: bool },
}

impl GridView {
    /// The column whose right edge is within reach of a point in the
    /// header.
    fn edge_at(&self, at: Point<Pixels>, window: &Window) -> Option<usize> {
        let grip = window.rem_size() * GRIP_REMS;
        let Hit::Header { column } = self.hit(at - point(grip, px(0.))) else { return None };
        match self.hit(at + point(grip, px(0.))) {
            Hit::Header { column: next } if next == column => None,
            _ => Some(column),
        }
    }

    pub(crate) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
        }
        match self.drag {
            None => {
                // Nothing is being dragged: show whether an edge could be.
                let over = self.edge_at(event.position, window).is_some();
                if over != self.over_edge {
                    self.over_edge = over;
                    cx.notify();
                }
            }
            Some(Drag::Resize { column, from, width }) => {
                let rem = f32::from(window.rem_size());
                let rems = width + f32::from(event.position.x - from) / rem;
                self.state.update(cx, |state, cx| state.set_width(column, rems, cx));
            }
            Some(Drag::Select { .. }) => {
                self.drag = Some(Drag::Select { at: event.position });
                self.drag_select(cx);
            }
            Some(Drag::Column { column, from, moved }) => {
                let slack = window.rem_size() * DRAG_SLACK_REMS;
                let moved = moved || (event.position.x - from).abs() > slack;
                // The column goes where the pointer is, as it moves.
                let over = match self.hit(point(event.position.x, self.frame.borrow().origin.y)) {
                    Hit::Header { column: over } if moved => over,
                    _ => column,
                };
                self.state.update(cx, |state, cx| state.move_column(column, over, cx));
                self.drag = Some(Drag::Column { column: over, from, moved });
            }
        }
    }

    /// Extends the selection to the cell under the pointer. With the
    /// pointer past an edge, scrolls that way and keeps scrolling,
    /// faster the further out it is, for as long as the drag is held.
    pub(crate) fn drag_select(&mut self, cx: &mut Context<Self>) {
        let Some(Drag::Select { at }) = self.drag else { return };
        let target = {
            let frame = self.frame.borrow();
            let (Some(layout), Some(view)) = (&frame.layout, &frame.viewport) else { return };
            let local = at - frame.origin;
            let (x, y) = (f32::from(local.x), f32::from(local.y));
            let (left, top) = (0.0, layout.header);
            let right = view.width.min((layout.columns.total() - view.scroll_x) as f32);
            let bottom = view.height.min(top + (layout.rows.total() - view.scroll_y) as f32);
            let past = |at: f32, low: f32, high: f32| {
                if at < low {
                    at - low
                } else if at > high {
                    at - high
                } else {
                    0.0
                }
            };
            let row = layout.rows.size(0).max(1.0);
            let most = row * MOST_ROWS_A_FRAME;
            let down = past(y, top, view.height);
            let rows = if down == 0.0 { 0.0 } else { down.signum() * row + down };
            let step = (past(x, left, view.width).clamp(-most, most), rows.clamp(-most, most));
            let (max_x, max_y) = layout.max_scroll(view);
            let scroll = (
                (view.scroll_x + f64::from(step.0)).clamp(0.0, max_x),
                (view.scroll_y + f64::from(step.1)).clamp(0.0, max_y),
            );
            let moved = scroll != (view.scroll_x, view.scroll_y);
            let scrolled = Viewport { scroll_x: scroll.0, scroll_y: scroll.1, ..*view };
            let inside =
                (x.clamp(left, (right - 1.0).max(left)), y.clamp(top, (bottom - 1.0).max(top)));
            (scroll, moved, layout.hit(&scrolled, inside.0, inside.1))
        };
        let (scroll, moved, hit) = target;
        self.state.update(cx, |state, cx| {
            state.scroll_to(scroll.0, scroll.1, cx);
            if let Hit::Cell { row, column } = hit
                && !state.is_group(row)
            {
                state.drag_to(Cell { row, column }, cx);
            }
        });
        // The change redraws the grid, and drawing it takes the next step.
        self.autoscroll = moved;
    }

    pub(crate) fn mouse_up(&mut self, cx: &mut Context<Self>) {
        // A press on a header that never became a drag sorts by it.
        if let Some(Drag::Column { column, moved: false, .. }) = self.drag {
            self.state.update(cx, |state, cx| state.sort_by(column, cx));
        }
        self.drag = None;
        self.autoscroll = false;
    }

    /// Sizes a column to the widest text it shows now, its title
    /// included.
    pub(crate) fn fit_column(
        &mut self,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(face) = self.frame.borrow().lines.face().cloned() else { return };
        let rows = self.visible_rows();
        let texts: Vec<String> = self.state.update(cx, |state, _| {
            let mut texts = vec![state.header_text(column)];
            texts.extend(rows.map(|row| state.cell_text(row, column)));
            texts
        });
        let widest =
            texts.into_iter().filter(|text| !text.is_empty()).fold(px(0.), |widest, text| {
                let run = TextRun {
                    len: text.len(),
                    font: face.font.clone(),
                    color: face.text,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line = window.text_system().shape_line(text.into(), face.size, &[run], None);
                widest.max(line.width)
            });
        let rem = window.rem_size();
        let rems = f32::from(widest) / f32::from(rem) + PADDING_REMS * 2.0 + FIT_SLACK_REMS;
        self.state.update(cx, |state, cx| state.set_width(column, rems, cx));
    }

    pub(crate) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if !self.state.update(cx, |state, cx| state.commit_edit(cx)) {
            return;
        }
        if let Some(column) = self.edge_at(event.position, window) {
            if event.click_count == 2 {
                return self.fit_column(column, window, cx);
            }
            let width = self.state.read(cx).widths().get(column).copied().unwrap_or_default();
            self.drag = Some(Drag::Resize { column, from: event.position.x, width });
            return;
        }
        let (extend, add) = (event.modifiers.shift, event.modifiers.secondary(cx));
        match self.hit(event.position) {
            Hit::Header { column } => {
                self.drag = Some(Drag::Column { column, from: event.position.x, moved: false });
            }
            Hit::Cell { row, column } => {
                let group = self.state.update(cx, |state, cx| {
                    if state.is_group(row) {
                        state.toggle_group(row, cx);
                        return true;
                    }
                    state.click(Cell { row, column }, extend, add, cx);
                    if event.click_count == 2 && !extend && !add {
                        state.begin_edit(None, cx);
                    }
                    false
                });
                if !group && event.click_count == 1 {
                    self.drag = Some(Drag::Select { at: event.position });
                }
            }
            Hit::Outside => {}
        }
    }

    pub(crate) fn scroll(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height());
        let limits = {
            let frame = self.frame.borrow();
            match (&frame.layout, &frame.viewport) {
                (Some(layout), Some(viewport)) => layout.max_scroll(viewport),
                _ => (0.0, 0.0),
            }
        };
        self.state.update(cx, |state, cx| {
            let x = (state.scroll.0 - f64::from(f32::from(delta.x))).clamp(0.0, limits.0);
            let y = (state.scroll.1 - f64::from(f32::from(delta.y))).clamp(0.0, limits.1);
            state.scroll_to(x, y, cx);
        });
    }
}
