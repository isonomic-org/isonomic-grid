// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The search bar: a field that narrows the rows to those that hold
//! what is typed in it.
//!
//! The search is part of the view's query, so it sorts, groups and
//! pins like any other view of the data, and a host can set it without
//! the bar.

use ui::base::input::{InputEvent, InputState};
use ui::component::ActiveTheme as _;
use ui::component::input::Input;
use ui::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Styled as _, Window, div,
};

use crate::actions::{Cancel, SEARCH_CONTEXT};
use crate::state::GridState;
use crate::view::GridView;

impl GridState {
    /// The text every shown row has to hold in some text column.
    pub fn search(&self) -> &str {
        &self.query().search
    }

    /// Shows only the rows that hold `text` in some text column,
    /// ignoring case and accents. Empty text shows them all.
    pub fn set_search(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.search() != text {
            let mut query = self.query().clone();
            query.search = text.to_owned();
            self.set_query(query, cx);
        }
    }
}

impl GridView {
    /// Opens the search bar, or goes back to it.
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.update(cx, |state, cx| state.commit_edit(cx)) {
            return;
        }
        let input = match &self.search_input {
            Some(input) => input.clone(),
            None => {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
                cx.subscribe_in(&input, window, Self::search_typed).detach();
                self.search_input = Some(input.clone());
                input
            }
        };
        let text = self.state.read(cx).search().to_owned();
        input.update(cx, |input, cx| input.set_value(text, window, cx));
        self.searching = true;
        input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Closes the bar, shows every row again and gives the keyboard
    /// back to the cells.
    pub(crate) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.searching = false;
        self.state.update(cx, |state, cx| state.set_search("", cx));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn search_typed(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change if self.searching => {
                let text = input.read(cx).value().to_string();
                self.state.update(cx, |state, cx| state.set_search(&text, cx));
            }
            // Enter keeps the rows found and goes to them.
            InputEvent::PressEnter { .. } => window.focus(&self.focus, cx),
            _ => {}
        }
    }

    /// The bar, while it is open or a search narrows the rows.
    pub(crate) fn search_bar(
        &mut self,
        top: Pixels,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let input = self.search_input.clone()?;
        let (active, rows) =
            self.state.update(cx, |state, _| (!state.search().is_empty(), state.row_count()));
        if !self.searching && !active {
            return None;
        }
        let count = match (active, rows) {
            (false, _) => String::new(),
            (true, 0) => "No rows".to_owned(),
            (true, 1) => "1 row".to_owned(),
            (true, rows) => format!("{rows} rows"),
        };
        let theme = cx.theme();
        Some(
            div()
                .absolute()
                .top(top)
                .right_6()
                .mt_2()
                .flex()
                .items_center()
                .gap_2()
                .p_1()
                .pr_3()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_md()
                .occlude()
                .key_context(SEARCH_CONTEXT)
                .on_action(
                    cx.listener(|this, _: &Cancel, window, cx| this.close_search(window, cx)),
                )
                .child(div().w_48().child(Input::new(&input).appearance(false)))
                .child(div().text_sm().text_color(theme.muted_foreground).child(count)),
        )
    }
}
