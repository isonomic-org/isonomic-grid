// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Changing cells in a headless window: the editor in the cell, the
//! clipboard, and clearing.

extern crate ui as gpui;

mod common;

use common::{active, click, keys, open, settle, text};
use grid_core::{ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::Cell;
use ui::{
    AppContext as _, ClipboardItem, Entity, EntityInputHandler as _, Modifiers, MouseButton,
    MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext, point, px,
};

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

#[ui::test]
fn typing_edits_the_active_cell(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10);
    let editing = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| view.state().read(cx).editing().cloned())
    };
    click(&view, cx, 1, 2, Modifiers::none());
    cx.simulate_input("41");
    settle(cx);
    assert_eq!(
        editing(&view, cx).unwrap().text(),
        "41",
        "typing starts an edit that replaces the cell"
    );
    assert_eq!(text(&view, cx, 1, 2), "9", "nothing is written until the edit is confirmed");
    keys(cx, "enter");
    assert_eq!(text(&view, cx, 1, 2), "41");
    assert_eq!(active(&view, cx), Cell { row: 2, column: 2 }, "Enter confirms and moves down");

    // Text that is not a number is refused; the editor stays open.
    cx.simulate_input("abc");
    settle(cx);
    keys(cx, "enter");
    let refused = editing(&view, cx).unwrap();
    assert!(refused.error.is_some() && refused.text() == "abc");
    assert_eq!(active(&view, cx), Cell { row: 2, column: 2 });
    keys(cx, "backspace backspace backspace");
    cx.simulate_input("7");
    settle(cx);
    keys(cx, "up");
    assert_eq!(text(&view, cx, 2, 2), "7", "moving away commits");
    assert_eq!(active(&view, cx), Cell { row: 1, column: 2 });

    // F2 amends, Escape cancels, Delete clears, and undo steps back.
    keys(cx, "f2");
    cx.simulate_input("0");
    settle(cx);
    assert_eq!(editing(&view, cx).unwrap().text(), "410");
    keys(cx, "escape");
    assert_eq!((editing(&view, cx), text(&view, cx, 1, 2)), (None, "41".to_owned()));
    keys(cx, "delete");
    assert_eq!(text(&view, cx, 1, 2), "");
    keys(cx, "secondary-z secondary-z");
    assert_eq!((text(&view, cx, 1, 2), text(&view, cx, 2, 2)), ("41".to_owned(), "8".to_owned()));
}

#[ui::test]
fn an_amended_cell_edits_at_the_caret(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10);
    let line = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| {
            let editing = view.state().read(cx).editing().cloned().unwrap();
            (editing.text().to_owned(), editing.line.caret())
        })
    };
    // A double click opens the cell with its own text.
    let bounds = view.read_with(cx, |view, _| view.cell_bounds(3, 0)).unwrap();
    let at = bounds.origin + point(px(4.), px(4.));
    for clicks in 1..=2 {
        cx.simulate_event(MouseDownEvent {
            position: at,
            button: MouseButton::Left,
            click_count: clicks,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            position: at,
            button: MouseButton::Left,
            click_count: clicks,
            ..Default::default()
        });
    }
    settle(cx);
    assert_eq!(line(&view, cx), ("row 000003".to_owned(), 10));

    // Arrows move the caret along the line, not to the next cell.
    keys(cx, "left left home right");
    assert_eq!(line(&view, cx).1, 1);
    cx.simulate_input("É");
    settle(cx);
    keys(cx, "delete end backspace");
    assert_eq!(line(&view, cx), ("rÉw 00000".to_owned(), 10));
    assert_eq!(active(&view, cx), Cell { row: 3, column: 0 });
    keys(cx, "enter");
    assert_eq!(text(&view, cx, 3, 0), "rÉw 00000");

    // An input method composes in place: each update replaces the last.
    click(&view, cx, 4, 0, Modifiers::none());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.replace_and_mark_text_in_range(None, "k", None, window, cx);
            view.replace_and_mark_text_in_range(None, "か", None, window, cx);
            assert_eq!(view.marked_text_range(window, cx), Some(0..1));
            view.replace_text_in_range(None, "火", window, cx);
            assert_eq!(view.marked_text_range(window, cx), None);
        });
    });
    settle(cx);
    assert_eq!(line(&view, cx), ("火".to_owned(), 3));
    keys(cx, "escape");
    assert_eq!(text(&view, cx, 4, 0), "row 000004");
}

#[ui::test]
fn cut_paste_and_clear_work_on_ranges(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10);
    let column = |view: &Entity<GridView>, cx: &mut VisualTestContext, column: usize| {
        (0..4).map(|row| text(view, cx, row, column)).collect::<Vec<_>>().join(",")
    };
    assert_eq!(column(&view, cx, 2), "10,9,8,7");

    // Copy two cells and paste them two rows down, as one step.
    click(&view, cx, 0, 1, Modifiers::none());
    click(&view, cx, 1, 2, Modifiers::shift());
    keys(cx, "secondary-c");
    click(&view, cx, 2, 1, Modifiers::none());
    keys(cx, "secondary-v");
    assert_eq!(column(&view, cx, 1), "core,apps,core,apps");
    assert_eq!(column(&view, cx, 2), "10,9,10,9");
    let pasted = view.read_with(cx, |view, cx| view.state().read(cx).selection().clone());
    assert!(pasted.contains(Cell { row: 3, column: 2 }), "what was pasted is selected");
    keys(cx, "secondary-z");
    assert_eq!(column(&view, cx, 2), "10,9,8,7", "one undo takes the whole paste back");

    // One value pasted over a range fills it.
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("5".to_owned())));
    click(&view, cx, 0, 2, Modifiers::none());
    click(&view, cx, 2, 2, Modifiers::shift());
    keys(cx, "secondary-v");
    assert_eq!(column(&view, cx, 2), "5,5,5,7");

    // Text a column cannot hold refuses the whole paste.
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("1\nx\n".to_owned())));
    click(&view, cx, 0, 2, Modifiers::none());
    keys(cx, "secondary-v");
    assert_eq!(column(&view, cx, 2), "5,5,5,7");

    // Cut copies, then clears the range; Delete clears a range too.
    click(&view, cx, 0, 2, Modifiers::none());
    click(&view, cx, 1, 2, Modifiers::shift());
    keys(cx, "secondary-x");
    assert_eq!(column(&view, cx, 2), ",,5,7");
    let clipboard = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(clipboard.as_deref(), Some("5\n5\n"));
    click(&view, cx, 2, 1, Modifiers::none());
    click(&view, cx, 3, 2, Modifiers::shift());
    keys(cx, "delete");
    assert_eq!((column(&view, cx, 1), column(&view, cx, 2)), ("core,apps,,".into(), ",,,".into()));
    keys(cx, "secondary-z secondary-z");
    assert_eq!(column(&view, cx, 2), "5,5,5,7");
}

#[ui::test]
fn what_is_drawn_follows_edits_undo_and_sorting(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, 10);
    let drawn = |view: &Entity<GridView>, cx: &mut VisualTestContext, row: usize, column: usize| {
        view.update(cx, |view, cx| view.drawn_text(row, column, cx))
    };
    // Every cell in view was drawn with its own text.
    let all_match = |view: &Entity<GridView>, cx: &mut VisualTestContext| {
        for row in 0..10 {
            for column in 0..3 {
                let shown = drawn(view, cx, row, column);
                assert_eq!(shown, Some(text(view, cx, row, column)), "row {row}, column {column}");
            }
        }
    };
    all_match(&view, cx);

    // An edit is drawn at once, though the cell's line was being kept.
    view.update(cx, |view, cx| {
        view.state().update(cx, |state, cx| state.set_cell_text(1, 2, "41", cx)).unwrap()
    });
    settle(cx);
    assert_eq!(drawn(&view, cx, 1, 2).as_deref(), Some("41"));
    all_match(&view, cx);
    keys(cx, "secondary-z");
    assert_eq!(drawn(&view, cx, 1, 2).as_deref(), Some("9"));
    keys(cx, "secondary-shift-z");
    assert_eq!(drawn(&view, cx, 1, 2).as_deref(), Some("41"));

    // Sorted, each row keeps its own lines and nothing is shaped again.
    view.read_with(cx, |view, _| view.take_line_counts());
    view.update(cx, |view, cx| view.state().update(cx, |state, cx| state.sort_by(2, cx)));
    settle(cx);
    all_match(&view, cx);
    let (_, shaped) = view.read_with(cx, |view, _| view.take_line_counts());
    assert_eq!(shaped, 1, "only the title, which gained its arrow");
}

#[ui::test]
fn a_space_flips_a_yes_or_no_cell(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        let specs = vec![
            ColumnSpec::new("name", "Name", ColumnType::Text),
            ColumnSpec::new("done", "Done", ColumnType::Bool),
        ];
        let mut table = Table::new(specs);
        table.push(&[Value::text("first"), Value::Bool(false)]).unwrap();
        table.push(&[Value::text("second"), Value::Null]).unwrap();
        let state = cx.new(|cx| GridState::new(table, cx));
        let view = GridView::new(state, cx);
        window.focus(&ui::Focusable::focus_handle(&view, cx), cx);
        view
    });
    settle(cx);

    click(&view, cx, 0, 1, Modifiers::none());
    cx.simulate_input(" ");
    settle(cx);
    assert_eq!(text(&view, cx, 0, 1), "true");
    cx.simulate_input(" ");
    settle(cx);
    assert_eq!(text(&view, cx, 0, 1), "false");
    // An empty cell becomes true, and undo empties it again.
    click(&view, cx, 1, 1, Modifiers::none());
    cx.simulate_input(" ");
    settle(cx);
    assert_eq!(text(&view, cx, 1, 1), "true");
    keys(cx, "secondary-z");
    assert_eq!(text(&view, cx, 1, 1), "");

    // In any other cell a space is typed.
    click(&view, cx, 0, 0, Modifiers::none());
    cx.simulate_input(" ");
    cx.simulate_input("x");
    settle(cx);
    keys(cx, "enter");
    assert_eq!(text(&view, cx, 0, 0), " x");
}
