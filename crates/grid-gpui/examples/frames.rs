// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Times the grid's own share of a frame while it scrolls: reading the
//! visible cells, laying them out and painting them into a scene. The
//! time the renderer then takes to draw that scene is not included.
//!
//! `cargo run --release -p grid-gpui --example frames [rows] [frames]`,
//! and with `--features grid-gpui/fast` for the optional faster backend.

extern crate ui as gpui;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::{ColumnSpec, ColumnType, Table, Value};
use grid_gpui::{GridState, GridView};
use ui::assets::Assets;
use ui::{AppContext as _, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, px, size};

const COLUMNS: usize = 64;
/// Narrow columns, so all sixty-four fit across the window: with the
/// header, about 2,500 cells in view, the size the budget is set for.
const WIDTH_REMS: f32 = 3.2;

fn table(rows: usize) -> Table {
    let specs = (0..COLUMNS)
        .map(|column| {
            let kind = [ColumnType::Text, ColumnType::Int, ColumnType::Float][column % 3];
            ColumnSpec::new(&format!("c{column}"), &format!("C{column}"), kind)
        })
        .collect();
    let mut table = Table::new(specs);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut row = Vec::with_capacity(COLUMNS);
    for _ in 0..rows {
        row.clear();
        for column in 0..COLUMNS {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            row.push(match column % 3 {
                0 => Value::text(&format!("t{:04}", seed % 5_000)),
                1 => Value::Int((seed >> 16) as i64 % 100_000),
                _ => Value::Float((seed >> 20) as f64 % 90_000.0 / 100.0),
            });
        }
        let _ = table.push(&row);
    }
    table
}

fn report(label: &str, times: &mut [Duration], cells: usize, (hits, misses): (usize, usize)) {
    times.sort();
    let mean = times.iter().sum::<Duration>() / times.len() as u32;
    let worst = times[times.len() * 99 / 100];
    let cached = 100.0 * hits as f64 / (hits + misses).max(1) as f64;
    println!(
        "{label:<32} {cells:>5} cells   mean {:>6.2} ms   99th {:>6.2} ms   {cached:>5.1}% of text from the cache",
        mean.as_secs_f64() * 1e3,
        worst.as_secs_f64() * 1e3
    );
}

fn main() -> ExitCode {
    let number = |place: usize, or: usize| {
        std::env::args().nth(place).and_then(|text| text.parse().ok()).unwrap_or(or)
    };
    let (rows, frames) = (number(1, 100_000), number(2, 300).max(20));
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
            size: size(px(3264.), px(1440.)),
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    let mut made = None;
    let opened = cx.update(|cx| {
        ui::open_window(options, cx, |_, cx| {
            let state = cx.new(|cx| GridState::new(table(rows), cx));
            state.update(cx, |state, cx| {
                for column in 0..COLUMNS {
                    state.set_width(column, WIDTH_REMS, cx);
                }
            });
            made = Some(state.clone());
            cx.new(|cx| GridView::new(state, cx))
        })
    });
    let (Ok((handle, grid)), Some(state)) = (opened, made) else {
        eprintln!("could not open an offscreen window");
        return ExitCode::FAILURE;
    };

    // Each pass scrolls a different distance per frame: a few rows, as
    // a wheel does, and a whole screen, as a dragged scrollbar does.
    let passes = [
        ("three rows a frame", 84.0, false),
        ("a screen a frame", 1400.0, false),
        ("a screen a frame, back and forth", 1400.0, true),
    ];
    for (label, step, back) in passes {
        let mut times = Vec::with_capacity(frames);
        let mut cells = 0;
        cx.update(|cx| grid.read(cx).take_line_counts());
        for frame in 0..frames {
            let y = if back { (frame % 2) as f64 * step } else { frame as f64 * step };
            let drawn = cx.update_window(handle, |_, window, cx| {
                state.update(cx, |state, cx| state.scroll_to(0.0, y, cx));
                let started = Instant::now();
                window.draw(cx).clear(cx);
                started.elapsed()
            });
            let Ok(elapsed) = drawn else { return ExitCode::FAILURE };
            // The first frames fill the caches; leave them out.
            if frame >= 10 {
                times.push(elapsed);
            }
            cells = cx.update(|cx| grid.read(cx).painted_cells());
        }
        let counts = cx.update(|cx| grid.read(cx).take_line_counts());
        report(label, &mut times, cells, counts);
    }
    ExitCode::SUCCESS
}
