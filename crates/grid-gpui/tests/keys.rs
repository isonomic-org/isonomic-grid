// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keys that jump and keys that select whole rows and columns, in a
//! headless window.

extern crate ui as gpui;

mod common;

use common::{active, click, keys, open, text};
use grid_gpui::GridView;
use grid_interact::Cell;
use ui::{Entity, Modifiers, TestAppContext, VisualTestContext};

fn range(view: &Entity<GridView>, cx: &mut VisualTestContext) -> ((usize, usize), (usize, usize)) {
    let selection = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    let last = selection.ranges.last().copied().unwrap();
    (last.rows(), last.columns())
}

#[ui::test]
fn keys_jump_to_the_ends_and_select_rows_and_columns(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 5_000);
    click(&view, cx, 3, 1, Modifiers::none());

    // To the bottom of the column, scrolled into view, and back.
    keys(cx, "secondary-down");
    assert_eq!(active(&view, cx), Cell { row: 4_999, column: 1 });
    let rows = view.read_with(cx, |view, _| view.visible_rows());
    assert!(rows.contains(&4_999), "the last row is in view: {rows:?}");
    keys(cx, "secondary-up secondary-right");
    assert_eq!(active(&view, cx), Cell { row: 0, column: 2 });
    keys(cx, "secondary-left");
    assert_eq!(active(&view, cx), Cell { row: 0, column: 0 });

    // With Shift they extend the range instead.
    click(&view, cx, 3, 1, Modifiers::none());
    keys(cx, "secondary-shift-down shift-end");
    assert_eq!(active(&view, cx), Cell { row: 3, column: 1 });
    assert_eq!(range(&view, cx), ((3, 4_999), (1, 2)));

    // A whole row, then a whole column, around the active cell.
    keys(cx, "secondary-up");
    click(&view, cx, 2, 1, Modifiers::none());
    keys(cx, "shift-space");
    assert_eq!(range(&view, cx), ((2, 2), (0, 2)));
    keys(cx, "ctrl-space");
    assert_eq!(range(&view, cx), ((0, 4_999), (1, 1)));
    assert_eq!(active(&view, cx), Cell { row: 2, column: 1 });

    // In a cell being edited the same keys type a space.
    keys(cx, "f2 shift-space");
    keys(cx, "enter");
    assert_eq!(text(&view, cx, 2, 1), "infra ");
}
