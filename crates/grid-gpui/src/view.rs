// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use grid_interact::{Cell, Move};
use grid_layout::Hit;
use ui::component::ActiveTheme as _;
use ui::{
    App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render,
    ScrollEvent, Styled as _, Subscription, Window, div, point, px, size,
};

use crate::actions::*;
use crate::element::{Colors, GridElement, SharedFrame};
use crate::state::GridState;

/// Draws a [`GridState`] and routes input to it.
pub struct GridView {
    state: Entity<GridState>,
    focus: FocusHandle,
    frame: SharedFrame,
    _observe: Subscription,
}

impl GridView {
    pub fn new(state: Entity<GridState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        Self { state, focus: cx.focus_handle(), frame: SharedFrame::default(), _observe: observe }
    }

    pub fn state(&self) -> &Entity<GridState> {
        &self.state
    }

    /// How many cells the last frame painted, header included.
    pub fn painted_cells(&self) -> usize {
        self.frame.borrow().painted
    }

    /// The rows in the scrolling band of the last frame.
    pub fn visible_rows(&self) -> Range<usize> {
        let frame = self.frame.borrow();
        match (&frame.layout, &frame.viewport) {
            (Some(layout), Some(viewport)) => layout.visible_rows(viewport)[1].indices.clone(),
            _ => 0..0,
        }
    }

    /// Where a cell was laid out in the last frame, in the window.
    pub fn cell_bounds(&self, row: usize, column: usize) -> Option<Bounds<Pixels>> {
        self.cell_bounds_in(&self.frame.borrow(), row, column)
    }

    /// A point inside a column's header cell.
    pub fn header_point(&self, column: usize) -> Option<Point<Pixels>> {
        let frame = self.frame.borrow();
        let layout = frame.layout.as_ref()?;
        let first = self.cell_bounds_in(&frame, 0, column).map(|bounds| bounds.origin.x);
        let x = first.unwrap_or(frame.origin.x);
        Some(point(
            x + px(layout.columns.size(column) / 2.0),
            frame.origin.y + px(layout.header / 2.0),
        ))
    }

    fn cell_bounds_in(
        &self,
        frame: &crate::element::Frame,
        row: usize,
        column: usize,
    ) -> Option<Bounds<Pixels>> {
        let rect = frame.layout.as_ref()?.cell_rect(frame.viewport.as_ref()?, row, column)?;
        Some(Bounds::new(
            frame.origin + point(px(rect.x), px(rect.y)),
            size(px(rect.width), px(rect.height)),
        ))
    }

    fn hit(&self, at: Point<Pixels>) -> Hit {
        let frame = self.frame.borrow();
        match (&frame.layout, &frame.viewport) {
            (Some(layout), Some(viewport)) => {
                let local = at - frame.origin;
                layout.hit(viewport, f32::from(local.x), f32::from(local.y))
            }
            _ => Hit::Outside,
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let (extend, add) = (event.modifiers.shift, event.modifiers.secondary(cx));
        match self.hit(event.position) {
            Hit::Header { column } => self.state.update(cx, |state, cx| state.sort_by(column, cx)),
            Hit::Cell { row, column } => self.state.update(cx, |state, cx| {
                if state.is_group(row) {
                    state.toggle_group(row, cx);
                } else {
                    state.click(Cell { row, column }, extend, add, cx);
                }
            }),
            Hit::Outside => {}
        }
    }

    fn scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    /// Moves the selection and scrolls it into view.
    fn go(&mut self, step: Move, extend: bool, cx: &mut Context<Self>) {
        let page = self.frame.borrow().page.max(1);
        let to = self.state.update(cx, |state, cx| state.navigate(step, extend, page, cx));
        let reveal = {
            let frame = self.frame.borrow();
            match (&frame.layout, &frame.viewport) {
                (Some(layout), Some(viewport)) => Some(layout.reveal(viewport, to.row, to.column)),
                _ => None,
            }
        };
        if let Some((x, y)) = reveal {
            self.state.update(cx, |state, cx| state.scroll_to(x, y, cx));
        }
    }
}

impl Focusable for GridView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GridView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let colors = Colors {
            background: theme.background,
            foreground: theme.foreground,
            muted: theme.muted_foreground,
            border: theme.border,
            selection: theme.selection,
        };
        let element = GridElement { state: self.state.clone(), frame: self.frame.clone(), colors };
        div()
            .id("isonomic-grid")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.go(Move::Up, false, cx)))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.go(Move::Down, false, cx)))
            .on_action(cx.listener(|this, _: &MoveLeft, _, cx| this.go(Move::Left, false, cx)))
            .on_action(cx.listener(|this, _: &MoveRight, _, cx| this.go(Move::Right, false, cx)))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| this.go(Move::PageUp, false, cx)))
            .on_action(cx.listener(|this, _: &PageDown, _, cx| this.go(Move::PageDown, false, cx)))
            .on_action(
                cx.listener(|this, _: &MoveToRowStart, _, cx| this.go(Move::Home, false, cx)),
            )
            .on_action(cx.listener(|this, _: &MoveToRowEnd, _, cx| this.go(Move::End, false, cx)))
            .on_action(
                cx.listener(|this, _: &MoveToFirstCell, _, cx| this.go(Move::FirstCell, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveToLastCell, _, cx| this.go(Move::LastCell, false, cx)),
            )
            .on_action(cx.listener(|this, _: &NextCell, _, cx| this.go(Move::Next, false, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousCell, _, cx| this.go(Move::Previous, false, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.go(Move::Up, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.go(Move::Down, true, cx)))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| this.go(Move::Left, true, cx)))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| this.go(Move::Right, true, cx)))
            .on_action(cx.listener(|this, _: &SelectPageUp, _, cx| this.go(Move::PageUp, true, cx)))
            .on_action(
                cx.listener(|this, _: &SelectPageDown, _, cx| this.go(Move::PageDown, true, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.state.update(cx, |state, cx| state.select_all(cx));
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                let text = this.state.update(cx, |state, _| state.copy_text());
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }))
            .on_action(cx.listener(|this, _: &Undo, _, cx| {
                this.state.update(cx, |state, cx| state.undo(cx))
            }))
            .on_action(cx.listener(|this, _: &Redo, _, cx| {
                this.state.update(cx, |state, cx| state.redo(cx))
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_scroll(cx.listener(Self::scroll))
            .child(element)
    }
}
