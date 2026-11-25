// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Renders the grid offscreen with the real renderer and saves a PNG.
//! Needs a GPU or a software Vulkan driver.
//!
//! `cargo run -p grid-gpui --example screenshot -- grid.png [grouped]`

extern crate ui as gpui;

use std::process::ExitCode;
use std::sync::Arc;

use grid_core::{ColumnId, ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use grid_interact::Cell;
use ui::assets::Assets;
use ui::component::ActiveTheme as _;
use ui::test::TestWindowExt as _;
use ui::{
    AppContext as _, Bounds, Context, Entity, Focusable as _, HeadlessAppContext, IntoElement,
    ParentElement as _, Render, Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
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

struct Host {
    grid: Entity<GridView>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.grid.clone())
    }
}

fn main() -> ExitCode {
    let out = std::env::args().nth(1).unwrap_or_else(|| "grid.png".to_owned());
    let grouped = std::env::args().nth(2).is_some();
    let mut cx = HeadlessAppContext::with_platform(
        ui::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        ui::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        ui::init(cx);
        grid_gpui::init(cx);
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Default::default(),
            size: size(px(980.), px(560.)),
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    let opened = cx.update(|cx| {
        ui::open_window(options, cx, |window, cx| {
            let state = cx.new(|cx| GridState::new(table(), cx));
            state.update(cx, |state, cx| {
                if grouped {
                    state.group_by(vec![ColumnId::new("city")], cx);
                    state.toggle_group(1, cx);
                } else {
                    // Revenue, highest first; then a selected range.
                    state.sort_by(3, cx);
                    state.sort_by(3, cx);
                    state.click(Cell { row: 2, column: 1 }, false, false, cx);
                    state.click(Cell { row: 5, column: 3 }, true, false, cx);
                }
            });
            let grid = cx.new(|cx| GridView::new(state, cx));
            let focus = grid.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            cx.new(|_| Host { grid })
        })
    });
    let Ok((handle, _)) = opened else {
        eprintln!("could not open an offscreen window");
        return ExitCode::FAILURE;
    };
    let _ = cx.update_window(handle, |_, window, cx| window.render_frame(cx));
    match cx.capture_screenshot(handle) {
        Ok(image) => match image.save(&out) {
            Ok(()) => {
                println!("wrote {out}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("could not write {out}: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("offscreen rendering is not available: {error}");
            ExitCode::FAILURE
        }
    }
}
