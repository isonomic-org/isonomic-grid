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

#[ui::test]
fn a_grid_with_nothing_in_it_takes_every_key_and_click(cx: &mut TestAppContext) {
    use grid_core::{ColumnSpec, ColumnType, Table};
    use grid_gpui::GridState;
    use ui::{AppContext as _, ClipboardItem, Focusable as _, point, px};

    cx.update(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
    });
    for columns in [0, 2] {
        let (view, cx) = cx.add_window_view(move |window, cx| {
            let specs = (0..columns)
                .map(|column| ColumnSpec::new(&format!("c{column}"), "C", ColumnType::Int))
                .collect();
            let state = cx.new(|cx| GridState::new(Table::new(specs), cx));
            let view = GridView::new(state, cx);
            window.focus(&view.focus_handle(cx), cx);
            view
        });
        common::settle(cx);
        keys(
            cx,
            "down up left right pagedown pageup home end tab shift-tab enter escape f2 delete \
             backspace escape shift-space ctrl-space secondary-a secondary-c secondary-x \
             secondary-down secondary-shift-up secondary-z secondary-shift-z",
        );
        cx.simulate_input("7");
        keys(cx, "enter escape");
        for at in [point(px(5.), px(5.)), point(px(300.), px(300.)), point(px(-5.), px(9_000.))] {
            cx.simulate_click(at, Modifiers::none());
        }
        view.update(cx, |view, cx| {
            view.state().update(cx, |state, cx| {
                state.move_column(0, 1, cx);
                state.hide_column(0, cx);
                state.pin_columns(3, 3, cx);
                state.pin_rows(3, 3, cx);
                state.set_width(0, f32::NAN, cx);
                state.set_width(9, 1.0, cx);
                state.sort_by(0, cx);
                state.toggle_group(0, cx);
                state.scroll_to(f64::MAX, f64::MAX, cx);
            })
        });
        common::settle(cx);

        // Pasting into a table with columns and no rows makes rows.
        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("1\t2\n3\t4\n".into())));
        keys(cx, "secondary-v");
        let rows =
            view.update(cx, |view, cx| view.state().update(cx, |state, _| state.row_count()));
        assert_eq!(rows, if columns == 0 { 0 } else { 2 }, "with {columns} columns");
    }
}
