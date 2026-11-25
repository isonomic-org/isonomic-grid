// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Columns in a headless window: order, visibility, pins and fitting.

extern crate ui as gpui;

mod common;

use common::{active, click, open, settle, text};
use grid_core::ColumnId;
use grid_gpui::GridView;
use grid_interact::Cell;
use ui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta,
    ScrollEvent, TestAppContext, VisualTestContext, point, px,
};

fn shown(view: &Entity<GridView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, cx| {
        let columns = view.state().read(cx).shown_columns();
        columns.iter().map(|id| id.as_str().to_owned()).collect()
    })
}

#[ui::test]
fn columns_move_hide_and_show_and_the_selection_goes_with_them(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 20);
    click(&view, cx, 2, 1, Modifiers::none());
    assert_eq!(text(&view, cx, 2, 1), "infra");

    view.update(cx, |view, cx| view.state().update(cx, |state, cx| state.move_column(1, 2, cx)));
    settle(cx);
    assert_eq!(shown(&view, cx), ["name", "n", "team"]);
    assert_eq!(active(&view, cx), Cell { row: 2, column: 2 }, "still on the team column");
    assert_eq!(text(&view, cx, 2, 2), "infra");

    view.update(cx, |view, cx| view.state().update(cx, |state, cx| state.hide_column(0, cx)));
    settle(cx);
    assert_eq!(shown(&view, cx), ["n", "team"]);
    assert_eq!(active(&view, cx), Cell { row: 2, column: 1 });
    let painted = view.read_with(cx, |view, _| view.painted_cells());
    let rows = view.read_with(cx, |view, _| view.visible_rows()).len();
    assert_eq!(painted, (rows + 1) * 2, "two columns are painted, not three");

    // The last column cannot be hidden; a column that is not there is ignored.
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| {
            state.hide_column(0, cx);
            state.hide_column(0, cx);
            state.hide_column(7, cx);
            state.move_column(0, 9, cx);
            state.show_column(&ColumnId::new("nope"), 0, cx);
        })
    });
    settle(cx);
    assert_eq!(shown(&view, cx), ["team"]);
    assert_eq!(active(&view, cx), Cell { row: 2, column: 0 });

    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| {
            state.show_column(&ColumnId::new("name"), 0, cx);
            state.show_column(&ColumnId::new("name"), 1, cx);
        })
    });
    settle(cx);
    assert_eq!(shown(&view, cx), ["name", "team"]);
    assert_eq!(text(&view, cx, 2, 0), "row 000002");
}

#[ui::test]
fn a_header_drag_moves_the_column_and_a_click_still_sorts(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 20);
    let header = |view: &Entity<GridView>, cx: &mut VisualTestContext, column: usize| {
        view.read_with(cx, |view, _| view.header_point(column)).unwrap()
    };
    let sorted = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| view.state().read(cx).query().sort.len())
    };
    let left = MouseButton::Left;
    let from = header(&view, cx, 0);
    let to = header(&view, cx, 2);
    cx.simulate_event(MouseDownEvent {
        position: from,
        button: left,
        click_count: 1,
        ..Default::default()
    });
    let over = header(&view, cx, 1);
    cx.simulate_event(MouseMoveEvent {
        position: over,
        pressed_button: Some(left),
        ..Default::default()
    });
    settle(cx);
    assert_eq!(shown(&view, cx), ["team", "name", "n"], "it moves as the pointer does");
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(left),
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent { position: to, button: left, ..Default::default() });
    settle(cx);
    assert_eq!(shown(&view, cx), ["team", "n", "name"]);
    assert_eq!(sorted(&view, cx), 0, "a drag does not sort");

    let second = header(&view, cx, 1);
    cx.simulate_click(second, Modifiers::none());
    settle(cx);
    assert_eq!(sorted(&view, cx), 1, "a click does");
    assert_eq!(shown(&view, cx), ["team", "n", "name"]);
}

#[ui::test]
fn pinned_columns_and_rows_stay_while_the_rest_scrolls(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 500);
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| {
            // Wider than the window, so there is something to scroll across.
            for column in 0..3 {
                state.set_width(column, 60.0, cx);
            }
            state.pin_columns(1, 0, cx);
            state.pin_rows(1, 1, cx);
        })
    });
    settle(cx);
    let at = |view: &Entity<GridView>, cx: &mut VisualTestContext, row: usize, column: usize| {
        view.read_with(cx, |view, _| view.cell_bounds(row, column)).unwrap().origin
    };
    let window = cx.update(|window, _| window.viewport_size());
    let (pinned, first, second) = (at(&view, cx, 0, 0), at(&view, cx, 5, 0), at(&view, cx, 5, 1));
    let last = at(&view, cx, 499, 0);
    assert!(last.y < window.height, "the last row is pinned at the bottom");

    cx.simulate_event(ScrollEvent {
        position: point(px(300.), px(300.)),
        delta: ScrollDelta::Pixels(point(px(-200.), px(-90.))),
        ..Default::default()
    });
    settle(cx);
    assert_eq!(at(&view, cx, 0, 0), pinned, "the pinned corner does not move");
    assert_eq!(at(&view, cx, 499, 0), last);
    assert_eq!(at(&view, cx, 5, 0), first - point(px(0.), px(90.)), "a pinned column scrolls down");
    assert_eq!(
        at(&view, cx, 5, 1),
        second - point(px(200.), px(90.)),
        "the rest scrolls both ways"
    );
    assert_eq!(
        at(&view, cx, 0, 1),
        point(second.x - px(200.), pinned.y),
        "a pinned row scrolls across"
    );

    // A click lands on what is drawn there: the pinned row, not the row scrolled under it.
    cx.simulate_click(pinned + point(px(4.), px(4.)), Modifiers::none());
    settle(cx);
    assert_eq!(active(&view, cx), Cell { row: 0, column: 0 });

    // More pins than there is anything to pin are cut down, not trusted.
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| {
            state.pin_columns(99, 99, cx);
            state.pin_rows(usize::MAX, usize::MAX, cx);
        })
    });
    settle(cx);
    let (columns, rows) = view.read_with(cx, |view, cx| view.state().read(cx).pins());
    assert_eq!((columns.leading, columns.trailing, rows.leading, rows.trailing), (3, 0, 500, 0));
}

#[ui::test]
fn a_double_click_on_an_edge_fits_the_column(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 20);
    let width = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| view.state().read(cx).widths()[0])
    };
    let before = width(&view, cx);
    let press = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        let edge = view.read_with(cx, |view, _| view.header_edge(0)).unwrap();
        for click_count in 1..=2 {
            cx.simulate_event(MouseDownEvent {
                position: edge,
                button: MouseButton::Left,
                click_count,
                ..Default::default()
            });
            cx.simulate_event(MouseUpEvent {
                position: edge,
                button: MouseButton::Left,
                click_count,
                ..Default::default()
            });
        }
        settle(cx);
    };
    press(&view, cx);
    let fitted = width(&view, cx);
    assert!(fitted < before && fitted > 3.0, "narrower, to fit ten characters: {fitted}");

    // Longer text in view makes the fit wider.
    view.update(cx, |view, cx| {
        let long = "a much longer name than any of the others in this column";
        view.state().update(cx, |state, cx| state.set_cell_text(3, 0, long, cx).unwrap());
    });
    settle(cx);
    press(&view, cx);
    assert!(width(&view, cx) > before, "wider than it started: {}", width(&view, cx));
}
