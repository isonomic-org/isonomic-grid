// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the pointer does in a grid: select, sort, open groups, drag a
//! column edge, and scroll.

use grid_interact::Cell;
use grid_layout::Hit;
use ui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ScrollEvent, Window,
    point, px,
};

use crate::view::GridView;

/// How close to a column's right edge, in rems, a press in the header
/// takes hold of the edge instead of sorting.
const GRIP_REMS: f32 = 0.3125;

/// A column edge being dragged.
pub(crate) struct Resize {
    column: usize,
    from: Pixels,
    /// The column's width when the drag began, in rems.
    width: f32,
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
        let Some(resize) = &self.resize else {
            // Nothing is being dragged: show whether an edge could be.
            let over = self.edge_at(event.position, window).is_some();
            if over != self.over_edge {
                self.over_edge = over;
                cx.notify();
            }
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.resize = None;
            return;
        }
        let rem = f32::from(window.rem_size());
        let (column, rems) =
            (resize.column, resize.width + f32::from(event.position.x - resize.from) / rem);
        self.state.update(cx, |state, cx| state.set_width(column, rems, cx));
    }

    pub(crate) fn mouse_up(&mut self) {
        self.resize = None;
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
            let width = self.state.read(cx).widths().get(column).copied().unwrap_or_default();
            self.resize = Some(Resize { column, from: event.position.x, width });
            return;
        }
        let (extend, add) = (event.modifiers.shift, event.modifiers.secondary(cx));
        match self.hit(event.position) {
            Hit::Header { column } => self.state.update(cx, |state, cx| state.sort_by(column, cx)),
            Hit::Cell { row, column } => self.state.update(cx, |state, cx| {
                if state.is_group(row) {
                    state.toggle_group(row, cx);
                } else {
                    state.click(Cell { row, column }, extend, add, cx);
                    if event.click_count == 2 && !extend && !add {
                        state.begin_edit(None, cx);
                    }
                }
            }),
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
