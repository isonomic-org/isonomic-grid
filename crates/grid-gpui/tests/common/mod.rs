// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A grid in a headless window, and the things a person does to it.
#![allow(dead_code)]

use grid_core::{ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::Cell;
use ui::{
    AppContext as _, Entity, Focusable as _, Modifiers, TestAppContext, VisualTestContext, point,
    px,
};

pub fn table(rows: usize) -> Table {
    let specs = vec![
        ColumnSpec::new("name", "Name", ColumnType::Text),
        ColumnSpec::new("team", "Team", ColumnType::Text),
        ColumnSpec::new("n", "N", ColumnType::Int),
    ];
    let mut table = Table::new(specs);
    for index in 0..rows {
        let team = ["core", "apps", "infra"][index % 3];
        let row = [
            Value::text(&format!("row {index:06}")),
            Value::text(team),
            Value::Int((rows - index) as i64),
        ];
        table.push(&row).unwrap();
    }
    table
}

pub fn open(cx: &mut TestAppContext, rows: usize) -> (Entity<GridView>, &mut VisualTestContext) {
    cx.update(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
    });
    let (view, vcx) = cx.add_window_view(move |window, cx| {
        let state = cx.new(|cx| GridState::new(table(rows), cx));
        let view = GridView::new(state, cx);
        let focus = view.focus_handle(cx);
        window.focus(&focus, cx);
        view
    });
    settle(vcx);
    (view, vcx)
}

pub fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// One more frame of something still moving: a running window asks
/// for it and is drawn again; here the view is told and drawn.
pub fn frame(view: &Entity<GridView>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    settle(cx);
}

/// Presses keys. `secondary` stands for the platform's command
/// modifier.
pub fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&keystrokes.replace("secondary", modifier));
    settle(cx);
}

pub fn active(view: &Entity<GridView>, cx: &mut VisualTestContext) -> Cell {
    view.read_with(cx, |view, cx| view.state().read(cx).selection().active)
}

pub fn text(
    view: &Entity<GridView>,
    cx: &mut VisualTestContext,
    row: usize,
    column: usize,
) -> String {
    view.update(cx, |view, cx| view.state().update(cx, |state, _| state.cell_text(row, column)))
}

pub fn click(
    view: &Entity<GridView>,
    cx: &mut VisualTestContext,
    row: usize,
    column: usize,
    modifiers: Modifiers,
) {
    let bounds = view.read_with(cx, |view, _| view.cell_bounds(row, column)).unwrap();
    cx.simulate_click(bounds.origin + point(px(4.), px(4.)), modifiers);
    settle(cx);
}
