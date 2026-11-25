// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use grid_interact::{LineEdit, Move};
use grid_layout::Hit;
use ui::base::Scrollbar;
use ui::component::ActiveTheme as _;
use ui::{
    App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Pixels, Point, Render, Styled as _, Subscription,
    Window, div, point, px, size,
};

use crate::actions::*;
use crate::element::{Colors, GridElement, SharedFrame};
use crate::pointer::Drag;
use crate::scroll::GridScrollHandle;
use crate::state::GridState;

/// Draws a [`GridState`] and routes input to it.
pub struct GridView {
    pub(crate) state: Entity<GridState>,
    pub(crate) focus: FocusHandle,
    pub(crate) frame: SharedFrame,
    pub(crate) drag: Option<Drag>,
    /// Whether a drag held past an edge scrolled on the last step, and
    /// so should take another when the grid is next drawn.
    pub(crate) autoscroll: bool,
    /// Whether the pointer is over a column edge it could drag.
    pub(crate) over_edge: bool,
    scroll: GridScrollHandle,
    _observe: Subscription,
}

impl GridView {
    pub fn new(state: Entity<GridState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            focus: cx.focus_handle(),
            frame: SharedFrame::default(),
            drag: None,
            autoscroll: false,
            over_edge: false,
            scroll: GridScrollHandle::default(),
            _observe: observe,
        }
    }

    pub fn state(&self) -> &Entity<GridState> {
        &self.state
    }

    /// How many cells the last frame painted, header included.
    pub fn painted_cells(&self) -> usize {
        self.frame.borrow().painted
    }

    /// How many cell texts were served from the shaped-line cache, and
    /// how many had to be shaped, since the last call.
    pub fn take_line_counts(&self) -> (usize, usize) {
        self.frame.borrow_mut().lines.take_counts()
    }

    /// The most cell texts any one frame shaped since the last call.
    pub fn take_line_peak(&self) -> usize {
        self.frame.borrow_mut().lines.take_peak()
    }

    /// How many cells the last frame left empty for a later frame to
    /// shape and draw.
    pub fn deferred_lines(&self) -> usize {
        self.frame.borrow().lines.deferred()
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

    pub(crate) fn hit(&self, at: Point<Pixels>) -> Hit {
        let frame = self.frame.borrow();
        match (&frame.layout, &frame.viewport) {
            (Some(layout), Some(viewport)) => {
                let local = at - frame.origin;
                layout.hit(viewport, f32::from(local.x), f32::from(local.y))
            }
            _ => Hit::Outside,
        }
    }

    /// A point on a column's right edge in the header.
    pub fn header_edge(&self, column: usize) -> Option<Point<Pixels>> {
        let frame = self.frame.borrow();
        let width = frame.layout.as_ref()?.columns.size(column);
        drop(frame);
        Some(self.header_point(column)? + point(px(width / 2.0), px(0.)))
    }

    /// Moves the selection and scrolls it into view.
    pub(crate) fn go(&mut self, step: Move, extend: bool, cx: &mut Context<Self>) {
        // In an edit begun from the cell's own text, the keys that
        // move along a row move the caret along the line instead.
        let amending = self.state.read(cx).editing().is_some_and(|editing| editing.amend);
        let along: Option<fn(&mut LineEdit)> = match step {
            Move::Left => Some(LineEdit::move_left),
            Move::Right => Some(LineEdit::move_right),
            Move::Home => Some(LineEdit::move_to_start),
            Move::End => Some(LineEdit::move_to_end),
            _ => None,
        };
        if let (true, false, Some(along)) = (amending, extend, along) {
            return self.state.update(cx, |state, cx| state.edit_line(along, cx));
        }
        // Moving away from a cell being edited commits it first.
        if !self.state.update(cx, |state, cx| state.commit_edit(cx)) {
            return;
        }
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

impl GridView {
    /// Selects the active cell's row or column, unless a cell is being
    /// edited, where the keys for it type a space.
    fn select_line(&mut self, row: bool, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| match state.editing() {
            Some(_) => state.edit_line(|line| line.insert(" "), cx),
            None => state.select_line(row, cx),
        });
    }
}

impl Focusable for GridView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GridView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.autoscroll) {
            self.drag_select(cx);
            if self.autoscroll {
                window.request_animation_frame();
            }
        }
        let theme = cx.theme();
        let colors = Colors {
            background: theme.background,
            foreground: theme.foreground,
            muted: theme.muted_foreground,
            border: theme.border,
            selection: theme.selection,
            caret: theme.caret,
            error: theme.danger,
        };
        let element = GridElement {
            view: cx.entity(),
            focus: self.focus.clone(),
            state: self.state.clone(),
            frame: self.frame.clone(),
            scroll: self.scroll.clone(),
            resizing: self.over_edge || matches!(self.drag, Some(Drag::Resize { .. })),
            colors,
        };
        let header = px(self.frame.borrow().layout.as_ref().map_or(0.0, |layout| layout.header));
        let bar = Scrollbar::width();
        div()
            .id("isonomic-grid")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
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
            .on_action(cx.listener(|this, _: &MoveToTop, _, cx| this.go(Move::Top, false, cx)))
            .on_action(
                cx.listener(|this, _: &MoveToBottom, _, cx| this.go(Move::Bottom, false, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectToTop, _, cx| this.go(Move::Top, true, cx)))
            .on_action(
                cx.listener(|this, _: &SelectToBottom, _, cx| this.go(Move::Bottom, true, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectToRowStart, _, cx| this.go(Move::Home, true, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectToRowEnd, _, cx| this.go(Move::End, true, cx)))
            .on_action(cx.listener(|this, _: &SelectRow, _, cx| this.select_line(true, cx)))
            .on_action(cx.listener(|this, _: &SelectColumn, _, cx| this.select_line(false, cx)))
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
            .on_action(cx.listener(|this, _: &Confirm, _, cx| {
                if this.state.read(cx).editing().is_some() {
                    this.go(Move::Down, false, cx);
                } else {
                    this.state.update(cx, |state, cx| state.begin_edit(None, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &EditCell, _, cx| {
                this.state.update(cx, |state, cx| state.begin_edit(None, cx));
            }))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                this.state.update(cx, |state, cx| state.cancel_edit(cx));
            }))
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                this.state.update(cx, |state, cx| match state.editing() {
                    Some(_) => state.edit_line(|line| _ = line.backspace(), cx),
                    None => state.begin_edit(Some(""), cx),
                });
            }))
            .on_action(cx.listener(|this, _: &ClearCell, _, cx| {
                this.state.update(cx, |state, cx| match state.editing() {
                    Some(_) => state.edit_line(|line| _ = line.delete(), cx),
                    None => state.clear_selection(cx),
                });
            }))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                if this.state.read(cx).editing().is_some() {
                    return;
                }
                let text = this.state.update(cx, |state, _| state.copy_text());
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                this.state.update(cx, |state, cx| state.clear_selection(cx));
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                    return;
                };
                this.state.update(cx, |state, cx| match state.editing() {
                    // Into a cell being edited goes the first line only.
                    Some(_) => {
                        let line = text.lines().next().unwrap_or_default().replace('\t', " ");
                        state.edit_line(|edit| edit.insert(&line), cx);
                    }
                    None => state.paste_text(&text, cx),
                });
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_scroll(cx.listener(Self::scroll))
            .child(element)
            .child(
                div()
                    .absolute()
                    .top(header)
                    .right_0()
                    .bottom_0()
                    .w(bar)
                    .child(Scrollbar::vertical(&self.scroll).viewport_from_layout()),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(bar)
                    .child(Scrollbar::horizontal(&self.scroll).viewport_from_layout()),
            )
    }
}
