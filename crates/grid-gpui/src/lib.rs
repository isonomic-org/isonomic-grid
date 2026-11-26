// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The grid view.
//!
//! [`GridState`] holds a data source, the view query, the selection
//! and the scroll position. [`GridView`] paints the visible cells in
//! one element: no element per cell, no layout pass per row. A grid
//! of a million rows paints the same few hundred cells as a grid of
//! fifty.

// Derives and attributes from the component library expand to paths
// under this name.
extern crate ui as gpui;

mod actions;
mod columns;
mod edit;
mod element;
mod held;
mod input;
mod lines;
mod menu;
mod paint;
mod pointer;
mod scroll;
mod state;
mod view;

pub use actions::*;
pub use edit::Editing;
pub use state::{GridEvent, GridState};
pub use view::GridView;

/// Registers the grid's key bindings. Call once at start-up.
pub fn init(cx: &mut ui::App) {
    actions::bind_keys(cx);
}
