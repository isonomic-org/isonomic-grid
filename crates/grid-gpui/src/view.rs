// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use grid_interact::{Cell, Move};
use grid_layout::Hit;
use ui::component::ActiveTheme as _;
use ui::{
    App, Bounds, ClipboardItem, Context, Entity, EntityInputHandler, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, Pixels, Point, Render, ScrollEvent, Styled as _, Subscription,
    UTF16Selection, Window, div, point, px, size,
};

use crate::actions::*;
use crate::element::{Colors, GridElement, SharedFrame};
use crate::state::GridState;

/// How close to a column's right edge, in rems, a press in the header
/// takes hold of the edge instead of sorting.
const GRIP_REMS: f32 = 0.3125;

/// A column edge being dragged.
struct Resize {
    column: usize,
    from: Pixels,
    /// The column's width when the drag began, in rems.
    width: f32,
}

/// Draws a [`GridState`] and routes input to it.
pub struct GridView {
    state: Entity<GridState>,
    focus: FocusHandle,
    frame: SharedFrame,
    resize: Option<Resize>,
    _observe: Subscription,
}

impl GridView {
    pub fn new(state: Entity<GridState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            focus: cx.focus_handle(),
            frame: SharedFrame::default(),
            resize: None,
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

    /// A point on a column's right edge in the header.
    pub fn header_edge(&self, column: usize) -> Option<Point<Pixels>> {
        let frame = self.frame.borrow();
        let width = frame.layout.as_ref()?.columns.size(column);
        drop(frame);
        Some(self.header_point(column)? + point(px(width / 2.0), px(0.)))
    }

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
        let Some(resize) = &self.resize else { return };
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

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
            caret: theme.caret,
            error: theme.danger,
        };
        let element = GridElement {
            view: cx.entity(),
            focus: self.focus.clone(),
            state: self.state.clone(),
            frame: self.frame.clone(),
            colors,
        };
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
                    Some(_) => state.edit_backspace(cx),
                    None => state.begin_edit(Some(""), cx),
                });
            }))
            .on_action(cx.listener(|this, _: &ClearCell, _, cx| {
                this.state.update(cx, |state, cx| state.clear_cell(cx));
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_scroll(cx.listener(Self::scroll))
            .child(element)
    }
}

/// Typing starts an edit of the active cell, or adds to the one in
/// progress. Composition is not supported yet: composed text arrives
/// as it is committed.
impl EntityInputHandler for GridView {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let at = self.state.read(cx).editing().map_or(0, |e| e.text.encode_utf16().count());
        Some(UTF16Selection { range: at..at, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| match state.editing() {
            Some(_) => state.edit_insert(text, cx),
            None => state.begin_edit(Some(text), cx),
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
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
