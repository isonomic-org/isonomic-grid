// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view in a headless window: real layout, real key bindings and
//! real pointer events, with no screen.

extern crate ui as gpui;

use grid_core::{ColumnId, ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::{Cell, Selection};
use ui::{
    AppContext as _, ClipboardItem, Entity, EntityInputHandler as _, Focusable as _, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollEvent,
    TestAppContext, VisualTestContext, point, px,
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
