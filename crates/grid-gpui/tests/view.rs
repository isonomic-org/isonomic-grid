// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view in a headless window: real layout, real key bindings and
//! real pointer events, with no screen.

extern crate ui as gpui;

use grid_core::{ColumnId, ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::{Cell, Selection};
use ui::{
    AppContext as _, Entity, Focusable as _, Modifiers, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext, point, px,
};

fn table(rows: usize) -> Table {
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

fn open(cx: &mut TestAppContext, rows: usize) -> (Entity<GridView>, &mut VisualTestContext) {
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

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Presses keys. `secondary` stands for the platform's command
/// modifier.
fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&keystrokes.replace("secondary", modifier));
    settle(cx);
}

fn active(view: &Entity<GridView>, cx: &mut VisualTestContext) -> Cell {
    view.read_with(cx, |view, cx| view.state().read(cx).selection().active)
}

fn text(view: &Entity<GridView>, cx: &mut VisualTestContext, row: usize, column: usize) -> String {
    view.update(cx, |view, cx| view.state().update(cx, |state, _| state.cell_text(row, column)))
}

fn click(
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

#[ui::test]
fn only_visible_cells_are_painted(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 100_000);
    let (painted, rows) = view.read_with(cx, |view, _| (view.painted_cells(), view.visible_rows()));
    assert_eq!(rows.start, 0);
    assert!(rows.len() > 5 && rows.len() < 200, "a screenful of rows, not {}", rows.len());
    assert_eq!(painted, (rows.len() + 1) * 3, "every visible cell and the header, nothing else");

    // Scroll to the very end: the same number of cells, the last rows.
    let event = ScrollEvent {
        position: point(px(50.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-1.0e8))),
        ..Default::default()
    };
    cx.simulate_event(event);
    settle(cx);
    let (painted_end, rows_end) =
        view.read_with(cx, |view, _| (view.painted_cells(), view.visible_rows()));
    assert_eq!(rows_end.end, 100_000);
    assert!(painted_end <= painted + 3);
    assert_eq!(text(&view, cx, 99_999, 0), "row 099999");
}

#[ui::test]
fn the_pointer_and_keyboard_move_the_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 500);
    click(&view, cx, 3, 1, Modifiers::none());
    assert_eq!(active(&view, cx), Cell { row: 3, column: 1 });
    keys(cx, "down right");
    assert_eq!(active(&view, cx), Cell { row: 4, column: 2 });
    keys(cx, "shift-down shift-down shift-left");
    let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    assert_eq!(selection.active, Cell { row: 4, column: 2 });
    assert_eq!(selection.ranges[0].rows(), (4, 6));
    assert_eq!(selection.ranges[0].columns(), (1, 2));
    click(&view, cx, 8, 0, Modifiers::shift());
    let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    assert_eq!(selection.ranges[0].rows(), (4, 8));

    // Paging past the screen scrolls the active cell into view.
    keys(cx, "pagedown pagedown pagedown");
    let (row, visible) = (active(&view, cx).row, view.read_with(cx, |view, _| view.visible_rows()));
    assert!(row > 20 && visible.contains(&row), "row {row} is within {visible:?}");
    keys(cx, "secondary-end");
    assert_eq!(active(&view, cx), Cell { row: 499, column: 2 });
    assert_eq!(view.read_with(cx, |view, _| view.visible_rows()).end, 500);
    keys(cx, "secondary-home");
    assert_eq!(
        view.read_with(cx, |view, cx| view.state().read(cx).selection().clone()),
        Selection::at(Cell::default())
    );
    assert_eq!(view.read_with(cx, |view, _| view.visible_rows()).start, 0);
}

#[ui::test]
fn a_header_click_sorts_and_a_group_row_toggles(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 30);
    let header = view.read_with(cx, |view, _| view.header_point(2)).unwrap();
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    assert_eq!(text(&view, cx, 0, 0), "row 000029", "ascending by N");
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    assert_eq!(text(&view, cx, 0, 0), "row 000000", "descending by N");
    cx.simulate_click(header, Modifiers::none());
    settle(cx);

    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.group_by(vec![ColumnId::new("team")], cx));
    });
    settle(cx);
    assert_eq!(text(&view, cx, 0, 0), "▾ apps (10)");
    let rows = view.update(cx, |view, cx| view.state().update(cx, |state, _| state.row_count()));
    assert_eq!(rows, 33);
    click(&view, cx, 0, 0, Modifiers::none());
    assert_eq!(text(&view, cx, 0, 0), "▸ apps (10)");
    assert_eq!(text(&view, cx, 1, 0), "▾ core (10)");
    let rows = view.update(cx, |view, cx| view.state().update(cx, |state, _| state.row_count()));
    assert_eq!(rows, 23);
}

#[ui::test]
fn cells_edit_copy_and_undo(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10);
    let edit = |view: &Entity<GridView>, cx: &mut VisualTestContext, text: &str| {
        view.update(cx, |view, cx| {
            view.state().update(cx, |state, cx| state.set_cell_text(1, 2, text, cx))
        })
    };
    assert!(edit(&view, cx, "not a number").is_err());
    assert_eq!(text(&view, cx, 1, 2), "9");
    assert_eq!(edit(&view, cx, "41"), Ok(()));
    settle(cx);
    assert_eq!(text(&view, cx, 1, 2), "41");
    click(&view, cx, 0, 0, Modifiers::none());
    click(&view, cx, 1, 2, Modifiers::shift());
    let copied = view.update(cx, |view, cx| view.state().update(cx, |state, _| state.copy_text()));
    assert_eq!(copied, "row 000000\tcore\t10\nrow 000001\tapps\t41\n");
    keys(cx, "secondary-z");
    assert_eq!(text(&view, cx, 1, 2), "9");
    keys(cx, "secondary-shift-z");
    assert_eq!(text(&view, cx, 1, 2), "41");
}
