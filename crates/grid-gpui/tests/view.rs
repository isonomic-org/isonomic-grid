// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view in a headless window: real layout, real key bindings and
//! real pointer events, with no screen.

extern crate ui as gpui;

mod common;

use common::{active, click, keys, open, settle, text};
use grid_core::{ColumnId, ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::{Cell, Selection};
use ui::{
    AppContext as _, Entity, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px,
};

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
fn dragging_a_header_edge_resizes_the_column(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 50);
    let widths = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| view.state().read(cx).widths().to_vec())
    };
    let before = widths(&view, cx);
    let rem = cx.update(|window, _| f32::from(window.rem_size()));
    let edge = view.read_with(cx, |view, _| view.header_edge(0)).unwrap();
    let second = view.read_with(cx, |view, _| view.cell_bounds(0, 1)).unwrap().origin.x;

    let left = MouseButton::Left;
    cx.simulate_event(MouseDownEvent {
        position: edge,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    let to = edge + point(px(3.0 * rem), px(0.));
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(left),
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent { position: to, button: left, ..Default::default() });
    settle(cx);

    let after = widths(&view, cx);
    assert!((after[0] - before[0] - 3.0).abs() < 0.01, "three rems wider: {after:?}");
    assert_eq!(after[1..], before[1..]);
    let moved = view.read_with(cx, |view, _| view.cell_bounds(0, 1)).unwrap().origin.x;
    assert!((f32::from(moved - second) - 3.0 * rem).abs() < 0.5, "the next column moved over");
    // Taking hold of an edge does not sort.
    assert!(view.read_with(cx, |view, cx| view.state().read(cx).query().sort.is_empty()));

    // A drag far to the left stops at the minimum width.
    cx.simulate_event(MouseDownEvent {
        position: to,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseMoveEvent {
        position: to - point(px(1000.), px(0.)),
        pressed_button: Some(left),
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent { position: to, button: left, ..Default::default() });
    settle(cx);
    let least = widths(&view, cx)[0];
    assert!(least > 0.0 && least < before[0], "narrow, not gone: {least}");

    // Moving the pointer with no button held changes nothing.
    cx.simulate_event(MouseMoveEvent {
        position: to + point(px(200.), px(0.)),
        ..Default::default()
    });
    settle(cx);
    assert_eq!(widths(&view, cx)[0], least);
}

#[ui::test]
fn dragging_the_scrollbar_crosses_a_million_rows(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 1_000_000);
    let rows = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.visible_rows())
    };
    assert_eq!(rows(&view, cx).start, 0);
    let window = cx.update(|window, _| window.viewport_size());
    let left = MouseButton::Left;
    let top = view.read_with(cx, |view, _| view.cell_bounds(0, 0)).unwrap().origin.y;
    // The thumb starts at the top of the track, just inside the right edge.
    let grab = point(window.width - px(8.), top + px(10.));
    cx.simulate_event(MouseMoveEvent { position: grab, ..Default::default() });
    settle(cx);
    cx.simulate_event(MouseDownEvent {
        position: grab,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    settle(cx);
    // Halfway down the track is the middle of the grid, to within a screen or two.
    let track = window.height - top;
    let half = point(grab.x, grab.y + (track - px(48.)) / 2.0);
    cx.simulate_event(MouseMoveEvent {
        position: half,
        pressed_button: Some(left),
        ..Default::default()
    });
    settle(cx);
    let middle = rows(&view, cx).start;
    assert!((490_000..510_000).contains(&middle), "about the middle: {middle}");
    // Dragged past the end, the last row is in view and exact.
    let end = point(grab.x, window.height + px(500.));
    cx.simulate_event(MouseMoveEvent {
        position: end,
        pressed_button: Some(left),
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent { position: end, button: left, ..Default::default() });
    settle(cx);
    assert_eq!(rows(&view, cx).end, 1_000_000);
    assert_eq!(text(&view, cx, 999_999, 0), "row 999999");
    // No cell was selected by pressing on the scrollbar.
    assert_eq!(active(&view, cx), Cell::default());
}

#[ui::test]
fn text_seen_a_moment_ago_is_not_shaped_again(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10_000);
    let counts = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.take_line_counts())
    };
    let (_, first) = counts(&view, cx);
    assert!(first > 0, "the first frame shapes what it shows");
    let scroll = |cx: &mut VisualTestContext, by: f32| {
        cx.simulate_event(ScrollEvent {
            position: point(px(50.), px(100.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(by))),
            ..Default::default()
        });
        settle(cx);
    };
    // Away by many screens, then back to where it began.
    scroll(cx, -20_000.);
    let (_, away) = counts(&view, cx);
    assert!(away > 0);
    scroll(cx, 20_000.);
    let (hits, misses) = counts(&view, cx);
    assert_eq!(misses, 0, "every line on the way back came from the cache");
    assert!(hits >= first);
}

#[ui::test]
fn a_screen_of_new_text_is_shaped_over_several_frames(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let specs = (0..12)
            .map(|column| ColumnSpec::new(&format!("c{column}"), "C", ColumnType::Text))
            .collect();
        let mut table = Table::new(specs);
        for row in 0..200 {
            let cells: Vec<_> = (0..12).map(|col| Value::text(&format!("r{row}c{col}"))).collect();
            table.push(&cells).unwrap();
        }
        let state = cx.new(|cx| GridState::new(table, cx));
        GridView::new(state, cx)
    });
    let draw = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(cx, |view, _| (view.take_line_counts().1, view.deferred_lines()))
    };
    cx.run_until_parked();
    while draw(cx).1 > 0 {}
    let cells = view.read_with(cx, |view, _| view.painted_cells());
    assert!(cells > 300, "more cells than one frame may shape: {cells}");
    view.read_with(cx, |view, _| (view.take_line_counts(), view.take_line_peak()));

    // Jump to rows never seen: every cell but the headers is new.
    cx.simulate_event(ScrollEvent {
        position: point(px(50.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-3_000.))),
        ..Default::default()
    });
    cx.run_until_parked();
    let mut shaped = 0;
    loop {
        let (new, left) = draw(cx);
        shaped += new;
        if left == 0 {
            break;
        }
    }
    // No frame shaped more than its share, nothing was shaped twice,
    // and it ended with every cell drawn.
    let peak = view.read_with(cx, |view, _| view.take_line_peak());
    assert_eq!(peak, 256, "the busiest frame shaped exactly its share");
    let cells = view.read_with(cx, |view, _| view.painted_cells());
    assert_eq!(shaped, cells - 12, "every cell but the headers, once each");
    assert_eq!(draw(cx), (0, 0), "and then nothing is left to do");
}

#[ui::test]
fn a_drag_selects_a_range_and_scrolls_past_the_edge(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10_000);
    let range = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
        let last = selection.ranges.last().copied().unwrap();
        (selection.active, last.rows(), last.columns())
    };
    let at = |view: &Entity<GridView>, cx: &mut VisualTestContext, row: usize, column: usize| {
        let bounds = view.read_with(cx, |view, _| view.cell_bounds(row, column)).unwrap();
        bounds.origin + point(px(4.), px(4.))
    };
    let left = MouseButton::Left;
    let from = at(&view, cx, 1, 0);
    cx.simulate_event(MouseDownEvent {
        position: from,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    let to = at(&view, cx, 4, 2);
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(left),
        ..Default::default()
    });
    settle(cx);
    assert_eq!(range(&view, cx), (Cell { row: 1, column: 0 }, (1, 4), (0, 2)));

    // Held below the window, the grid scrolls and the range grows with
    // every frame, without the pointer moving again.
    let window = cx.update(|window, _| window.viewport_size());
    let below = point(to.x, window.height + px(40.));
    cx.simulate_event(MouseMoveEvent {
        position: below,
        pressed_button: Some(left),
        ..Default::default()
    });
    settle(cx);
    let first = range(&view, cx).1.1;
    let visible = view.read_with(cx, |view, _| view.visible_rows());
    assert!(visible.start > 0 && first >= visible.end - 2, "scrolled, head on the last row shown");
    settle(cx);
    settle(cx);
    let later = range(&view, cx).1.1;
    assert!(later > first, "it keeps going: {first} then {later}");

    // Letting go stops it.
    cx.simulate_event(MouseUpEvent { position: below, button: left, ..Default::default() });
    settle(cx);
    let stopped = range(&view, cx);
    settle(cx);
    settle(cx);
    assert_eq!(range(&view, cx), stopped);
    assert_eq!(stopped.0, Cell { row: 1, column: 0 }, "the anchor never moved");
}

#[ui::test]
fn the_selection_stays_on_its_rows_when_they_move(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 50);
    let picked = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
        let range = selection.ranges.last().copied().unwrap();
        let mut name = |row| text(view, cx, row, 0);
        (name(selection.active.row), name(range.anchor.row), name(range.head.row))
    };
    // Rows 3 to 5, anchored on row 3.
    click(&view, cx, 3, 0, Modifiers::none());
    click(&view, cx, 5, 1, Modifiers::shift());
    let before = picked(&view, cx);
    assert_eq!(before, ("row 000003".into(), "row 000003".into(), "row 000005".into()));

    // Sorting by N puts the last row first; the selection goes with its rows.
    let header = view.read_with(cx, |view, _| view.header_point(2)).unwrap();
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    assert_eq!(text(&view, cx, 0, 0), "row 000049");
    assert_eq!(picked(&view, cx), before);
    assert_eq!(active(&view, cx), Cell { row: 46, column: 0 });

    // An edit that moves a row in the sort order takes the active cell along.
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.set_cell_text(46, 2, "0", cx).unwrap());
    });
    settle(cx);
    assert_eq!(active(&view, cx), Cell { row: 0, column: 0 });
    assert_eq!(picked(&view, cx).0, "row 000003");
    keys(cx, "secondary-z");
    assert_eq!(picked(&view, cx), before);

    // Everything selected is still everything after the rows change.
    keys(cx, "secondary-a");
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    let all = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    assert!(all.contains(Cell { row: 0, column: 0 }) && all.contains(Cell { row: 49, column: 2 }));
}

#[ui::test]
fn a_selection_whose_row_is_filtered_out_falls_back_in_place(cx: &mut TestAppContext) {
    use grid_core::{Compare, Filter, ViewQuery};
    let (view, cx) = open(cx, 50);
    click(&view, cx, 4, 1, Modifiers::none());
    click(&view, cx, 40, 2, Modifiers::shift());
    // Keep only rows whose N is above 20: row 4 stays, row 40 goes.
    let filter =
        Filter::Is { column: ColumnId::new("n"), compare: Compare::Gt, value: Value::Int(20) };
    view.update(cx, |view, cx| {
        let query = ViewQuery { filter: Some(filter), ..Default::default() };
        view.state().update(cx, |state, cx| state.set_query(query, cx));
    });
    settle(cx);
    let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    assert_eq!(selection.active, Cell { row: 4, column: 1 });
    assert_eq!(selection.ranges, [grid_interact::CellRange::single(Cell { row: 4, column: 1 })]);
    assert_eq!(text(&view, cx, 4, 0), "row 000004");

    // With every row gone, the selection rests on the first cell and
    // nothing panics.
    view.update(cx, |view, cx| {
        let query = ViewQuery { search: "no such text".into(), ..Default::default() };
        view.state().update(cx, |state, cx| state.set_query(query, cx));
    });
    settle(cx);
    keys(cx, "down right enter escape delete");
    assert_eq!(active(&view, cx).row, 0);
}
