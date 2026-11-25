// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opens the grid in a window on 100,000 generated rows.
//!
//! `cargo run -p grid-gpui --example playground`

extern crate ui as gpui;

use grid_core::{ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use ui::assets::Assets;
use ui::component::ActiveTheme as _;
use ui::{
    AppContext as _, Bounds, Context, Entity, Focusable as _, IntoElement, ParentElement as _,
    Render, Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
};

fn table() -> Table {
    let specs = vec![
        ColumnSpec::new("customer", "Customer", ColumnType::Text),
        ColumnSpec::new("city", "City", ColumnType::Text),
        ColumnSpec::new("orders", "Orders", ColumnType::Int),
        ColumnSpec::new("revenue", "Revenue", ColumnType::Float),
        ColumnSpec::new("since", "Since", ColumnType::Date),
        ColumnSpec::new("active", "Active", ColumnType::Bool),
    ];
    let cities = ["Rabat", "Tanger", "Toronto", "Miami", "Buffalo", "Lisbon", "Zürich"];
    let mut table = Table::new(specs);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for index in 0..100_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let revenue = if seed.is_multiple_of(29) {
            Value::Null
        } else {
            Value::Float((seed >> 20) as f64 % 90_000.0 / 100.0)
        };
        let row = [
            Value::text(&format!("Customer {:05}", index + 1)),
            Value::text(cities[(seed >> 8) as usize % cities.len()]),
            Value::Int((seed >> 16) as i64 % 400),
            revenue,
            Value::Date(17_000 + (seed >> 32) as i32 % 3_700),
            Value::Bool(!seed.is_multiple_of(3)),
        ];
        let _ = table.push(&row);
    }
    table
}

struct Page {
    grid: Entity<GridView>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.grid.clone())
    }
}

fn main() {
    ui::application().with_assets(Assets).run(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
        cx.activate(true);
        let bounds = Bounds::centered(None, size(px(1000.), px(640.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let opened = ui::open_window(options, cx, |window, cx| {
            let state = cx.new(|cx| GridState::new(table(), cx));
            let grid = cx.new(|cx| GridView::new(state, cx));
            let focus = grid.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            cx.new(|_| Page { grid })
        });
        if let Ok((window, _)) = opened {
            let _ = window.update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title("Isonomic Grid");
            });
        }
    });
}
