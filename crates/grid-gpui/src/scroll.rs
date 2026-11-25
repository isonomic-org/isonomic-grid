// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The grid's scroll position as the standard scrollbars see it.
//!
//! The grid scrolls in 64-bit offsets; a scrollbar works in pixels that
//! stop being exact long before a large grid ends. The handle reports
//! each axis scaled into a range the scrollbar holds exactly, and turns
//! a dragged thumb back into a 64-bit offset.

use std::cell::RefCell;
use std::rc::Rc;

use grid_layout::{GridLayout, ScrollScale, Viewport};
use ui::base::ScrollbarHandle;
use ui::{Bounds, Pixels, Point, Size, point, px, size};

/// The longest content, in pixels, reported to a scrollbar.
const LIMIT: f64 = 1_000_000.0;

#[derive(Default)]
struct Inner {
    bounds: Bounds<Pixels>,
    scales: Option<(ScrollScale, ScrollScale)>,
    /// The offset last reported to the scrollbars, scaled.
    reported: Point<Pixels>,
    /// Offsets a scrollbar asked for since the last frame, per axis.
    moved: (Option<f64>, Option<f64>),
}

/// Shared between the grid's element, which writes the geometry of each
/// frame, and the scrollbars, which read it and move the thumb.
#[derive(Clone, Default)]
pub struct GridScrollHandle(Rc<RefCell<Inner>>);

impl GridScrollHandle {
    /// Records a frame: the body the scrollbars overlay and where it
    /// is scrolled.
    pub(crate) fn record(&self, body: Bounds<Pixels>, layout: &GridLayout, view: &Viewport) {
        let (max_x, max_y) = layout.max_scroll(view);
        let (width, height) = (f64::from(body.size.width), f64::from(body.size.height));
        let x = ScrollScale::new(max_x + width, width, LIMIT);
        let y = ScrollScale::new(max_y + height, height, LIMIT);
        let mut inner = self.0.borrow_mut();
        inner.bounds = body;
        inner.reported =
            point(px(-x.to_scaled(view.scroll_x) as f32), px(-y.to_scaled(view.scroll_y) as f32));
        inner.scales = Some((x, y));
    }

    /// The offsets a scrollbar asked for since the last call, each axis
    /// on its own: an axis nobody dragged is left exactly where it was.
    pub(crate) fn take_moved(&self) -> (Option<f64>, Option<f64>) {
        std::mem::take(&mut self.0.borrow_mut().moved)
    }
}

impl ScrollbarHandle for GridScrollHandle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }

    fn offset(&self) -> Point<Pixels> {
        self.0.borrow().reported
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let mut inner = self.0.borrow_mut();
        let Some((x, y)) = inner.scales else { return };
        if offset.x != inner.reported.x {
            inner.moved.0 = Some(x.from_scaled(-f64::from(f32::from(offset.x))));
            inner.reported.x = offset.x;
        }
        if offset.y != inner.reported.y {
            inner.moved.1 = Some(y.from_scaled(-f64::from(f32::from(offset.y))));
            inner.reported.y = offset.y;
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        let inner = self.0.borrow();
        match inner.scales {
            Some((x, y)) => size(px(x.scaled_content() as f32), px(y.scaled_content() as f32)),
            None => inner.bounds.size,
        }
    }
}
