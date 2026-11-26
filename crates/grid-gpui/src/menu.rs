// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The menu a secondary click opens: what to do with the column under
//! the pointer, and with the selected cells.
//!
//! Every entry is an action the view also answers from the keyboard or
//! from a host's own menus, so the menu holds no behavior of its own.

use grid_core::{ColumnId, Direction};
use grid_interact::Cell;
use grid_layout::Hit;
use ui::component::menu::PopupMenu;
use ui::{App, Context, MouseDownEvent, Window};

use crate::actions::{
    ClearSort, Copy, Cut, FitColumn, HideColumn, Paste, PinThroughColumn, ShowAllColumns,
    SortAscending, SortDescending, UnpinColumns,
};
use crate::view::GridView;

impl GridView {
    /// A press of the secondary button: remembers the column under it
    /// for the menu, and moves the selection to a cell outside it.
    pub(crate) fn secondary_press(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.menu_column = match self.hit(event.position) {
            Hit::Header { column } => Some(column),
            Hit::Cell { row, column } => {
                let cell = Cell { row, column };
                self.state.update(cx, |state, cx| {
                    if !state.is_group(row) && !state.selection().contains(cell) {
                        state.click(cell, false, false, cx);
                    }
                });
                Some(column)
            }
            Hit::Outside => None,
        };
        cx.notify();
    }

    /// The column the menu's column entries act on: the one last
    /// pressed with the secondary button, else the active cell's.
    pub(crate) fn menu_target(&self, cx: &App) -> usize {
        let state = self.state.read(cx);
        let column = self.menu_column.unwrap_or(state.selection().active.column);
        column.min(state.column_count().saturating_sub(1))
    }

    /// Fills the menu for the column and cells it was opened over.
    pub(crate) fn build_menu(&self, menu: PopupMenu, cx: &App) -> PopupMenu {
        let column = self.menu_target(cx);
        let state = self.state.read(cx);
        let sorted = state.sorted(column);
        let (pins, _) = state.pins();
        let hidden = state.hidden_columns().len();
        menu.action_context(self.focus.clone())
            .menu_with_check(
                "Sort ascending",
                sorted == Some(Direction::Ascending),
                Box::new(SortAscending),
            )
            .menu_with_check(
                "Sort descending",
                sorted == Some(Direction::Descending),
                Box::new(SortDescending),
            )
            .menu_with_disabled("Clear sort", Box::new(ClearSort), state.query().sort.is_empty())
            .separator()
            .menu("Pin columns up to here", Box::new(PinThroughColumn))
            .menu_with_disabled(
                "Unpin columns",
                Box::new(UnpinColumns),
                pins.leading + pins.trailing == 0,
            )
            .separator()
            .menu("Fit column to its text", Box::new(FitColumn))
            .menu_with_disabled("Hide column", Box::new(HideColumn), state.column_count() < 2)
            .menu_with_disabled(
                if hidden == 1 { "Show the hidden column" } else { "Show hidden columns" },
                Box::new(ShowAllColumns),
                hidden == 0,
            )
            .separator()
            .menu("Cut", Box::new(Cut))
            .menu("Copy", Box::new(Copy))
            .menu("Paste", Box::new(Paste))
    }

    pub(crate) fn sort_column(&mut self, to: Option<Direction>, cx: &mut Context<Self>) {
        let column = self.menu_target(cx);
        self.state.update(cx, |state, cx| state.sort(column, to, cx));
    }

    pub(crate) fn pin_through_column(&mut self, cx: &mut Context<Self>) {
        let column = self.menu_target(cx);
        self.state.update(cx, |state, cx| {
            let (pins, _) = state.pins();
            state.pin_columns(column + 1, pins.trailing, cx);
        });
    }

    pub(crate) fn unpin_columns(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.pin_columns(0, 0, cx));
    }

    pub(crate) fn hide_menu_column(&mut self, cx: &mut Context<Self>) {
        let column = self.menu_target(cx);
        self.menu_column = None;
        self.state.update(cx, |state, cx| state.hide_column(column, cx));
    }

    pub(crate) fn show_all_columns(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let hidden: Vec<ColumnId> = state.hidden_columns();
            for id in hidden {
                let end = state.column_count();
                state.show_column(&id, end, cx);
            }
        });
    }
}
