// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use ui::{App, KeyBinding};

/// The key context the grid view sets; bindings apply only inside.
pub const CONTEXT: &str = "IsonomicGrid";
/// The key context of the search bar.
pub const SEARCH_CONTEXT: &str = "IsonomicGridSearch";

ui::actions!(
    grid,
    [
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        PageUp,
        PageDown,
        MoveToRowStart,
        MoveToRowEnd,
        MoveToFirstCell,
        MoveToLastCell,
        MoveToTop,
        MoveToBottom,
        SelectToTop,
        SelectToBottom,
        SelectToRowStart,
        SelectToRowEnd,
        SelectRow,
        SelectColumn,
        NextCell,
        PreviousCell,
        SelectUp,
        SelectDown,
        SelectLeft,
        SelectRight,
        SelectPageUp,
        SelectPageDown,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Confirm,
        Cancel,
        EditCell,
        Backspace,
        ClearCell,
        SortAscending,
        SortDescending,
        ClearSort,
        PinThroughColumn,
        UnpinColumns,
        FitColumn,
        HideColumn,
        ShowAllColumns,
        Search,
    ]
);

/// The platform's command modifier with a key: Command on macOS,
/// Control elsewhere.
#[cfg(target_os = "macos")]
macro_rules! secondary {
    ($key:literal) => {
        concat!("cmd-", $key)
    };
}

#[cfg(not(target_os = "macos"))]
macro_rules! secondary {
    ($key:literal) => {
        concat!("ctrl-", $key)
    };
}

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("pageup", PageUp, context),
        KeyBinding::new("pagedown", PageDown, context),
        KeyBinding::new("home", MoveToRowStart, context),
        KeyBinding::new("end", MoveToRowEnd, context),
        KeyBinding::new(secondary!("home"), MoveToFirstCell, context),
        KeyBinding::new(secondary!("end"), MoveToLastCell, context),
        KeyBinding::new(secondary!("up"), MoveToTop, context),
        KeyBinding::new(secondary!("down"), MoveToBottom, context),
        KeyBinding::new(secondary!("left"), MoveToRowStart, context),
        KeyBinding::new(secondary!("right"), MoveToRowEnd, context),
        KeyBinding::new(secondary!("shift-up"), SelectToTop, context),
        KeyBinding::new(secondary!("shift-down"), SelectToBottom, context),
        KeyBinding::new(secondary!("shift-left"), SelectToRowStart, context),
        KeyBinding::new(secondary!("shift-right"), SelectToRowEnd, context),
        KeyBinding::new("shift-home", SelectToRowStart, context),
        KeyBinding::new("shift-end", SelectToRowEnd, context),
        KeyBinding::new("shift-space", SelectRow, context),
        KeyBinding::new("ctrl-space", SelectColumn, context),
        KeyBinding::new("tab", NextCell, context),
        KeyBinding::new("shift-tab", PreviousCell, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-pageup", SelectPageUp, context),
        KeyBinding::new("shift-pagedown", SelectPageDown, context),
        KeyBinding::new(secondary!("a"), SelectAll, context),
        KeyBinding::new(secondary!("c"), Copy, context),
        KeyBinding::new(secondary!("x"), Cut, context),
        KeyBinding::new(secondary!("v"), Paste, context),
        KeyBinding::new(secondary!("z"), Undo, context),
        KeyBinding::new(secondary!("shift-z"), Redo, context),
        KeyBinding::new("enter", Confirm, context),
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("f2", EditCell, context),
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", ClearCell, context),
        KeyBinding::new(secondary!("f"), Search, context),
    ]);
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some(SEARCH_CONTEXT))]);
}
