// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use grid_layout::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn a_uniform_axis_is_arithmetic() {
    let axis = Axis::uniform(1_000_000, 24.0);
    assert_eq!(axis.total(), 24_000_000.0);
    assert_eq!(axis.offset(500_000), 12_000_000.0);
    assert_eq!(axis.index_at(12_000_023.9), Some(500_000));
    assert_eq!(axis.index_at(1e12), Some(999_999));
    assert_eq!(axis.range(48.0, 96.0), 2..4);
    assert_eq!(axis.range(47.0, 97.0), 1..5);
    assert_eq!(Axis::uniform(0, 24.0).index_at(0.0), None);
    assert_eq!(Axis::uniform(0, 24.0).range(0.0, 100.0), 0..0);
}

#[test]
fn a_variable_axis_matches_plain_sums() {
    let mut rng = Rng(42);
    let mut sizes: Vec<f32> = (0..500).map(|_| (rng.next() % 60 + 10) as f32).collect();
    let mut axis = Axis::variable(sizes.clone());
    for _ in 0..200 {
        let index = (rng.next() % 500) as usize;
        let size = (rng.next() % 90) as f32;
        sizes[index] = size;
        axis.set_size(index, size);
    }
    let mut offset = 0.0f64;
    for (index, size) in sizes.iter().enumerate() {
        assert_eq!(axis.offset(index), offset);
        if *size > 0.0 {
            assert_eq!(axis.index_at(offset), Some(index));
            assert_eq!(axis.index_at(offset + f64::from(*size) - 0.5), Some(index));
        }
        offset += f64::from(*size);
    }
    assert_eq!(axis.total(), offset);
    let mut uniform = Axis::uniform(10, 20.0);
    uniform.set_size(3, 50.0);
    assert_eq!(uniform.offset(4), 110.0);
    assert_eq!(uniform.total(), 230.0);
}

fn layout() -> GridLayout {
    GridLayout {
        rows: Axis::uniform(100, 30.0),
        columns: Axis::variable(vec![80.0, 120.0, 100.0, 100.0, 100.0, 100.0, 60.0]),
        row_pins: Pins { leading: 1, trailing: 2 },
        column_pins: Pins { leading: 1, trailing: 1 },
        header: 32.0,
    }
}

#[test]
fn pinned_bands_stay_put_while_the_middle_scrolls() {
    let grid = layout();
    let view = Viewport { width: 400.0, height: 332.0, scroll_x: 130.0, scroll_y: 45.0 };
    let [top, middle, bottom] = grid.visible_rows(&view);
    assert_eq!((top.indices.clone(), top.start), (0..1, 32.0));
    assert_eq!((bottom.indices.clone(), bottom.start), (98..100, 272.0));
    assert_eq!(
        (middle.indices.clone(), middle.start),
        (2..10, 47.0),
        "row 2 is half hidden under the pinned row"
    );
    let [left, center, right] = grid.visible_columns(&view);
    assert_eq!((left.indices, left.start), (0..1, 0.0));
    assert_eq!((right.indices, right.start), (6..7, 340.0));
    assert_eq!((center.indices, center.start), (2..5, 70.0));
    assert_eq!(grid.max_scroll(&view), (260.0, 2700.0));
}

#[test]
fn hit_testing_agrees_with_cell_rects() {
    let grid = layout();
    let view = Viewport { width: 400.0, height: 332.0, scroll_x: 130.0, scroll_y: 45.0 };
    assert_eq!(grid.hit(&view, 10.0, 10.0), Hit::Header { column: 0 });
    assert_eq!(grid.hit(&view, 350.0, 40.0), Hit::Cell { row: 0, column: 6 });
    assert_eq!(grid.hit(&view, 100.0, 300.0), Hit::Cell { row: 98, column: 2 });
    assert_eq!(grid.hit(&view, 500.0, 40.0), Hit::Outside);
    assert_eq!(grid.hit(&view, 10.0, -5.0), Hit::Outside);
    // Every visible cell is hit at a point not covered by a pinned band.
    let clip = |start: f32, size: f32, pinned: bool, low: f32, high: f32| {
        if pinned { start + 0.5 } else { start.max(low) + 0.5_f32.min(size) }.min(if pinned {
            f32::MAX
        } else {
            high - 0.5
        })
    };
    let mut checked = 0;
    for rows in grid.visible_rows(&view) {
        for columns in grid.visible_columns(&view) {
            for row in rows.indices.clone() {
                for column in columns.indices.clone() {
                    let rect = grid.cell_rect(&view, row, column).unwrap();
                    let x = clip(rect.x, rect.width, columns.pinned, 80.0, 340.0);
                    let y = clip(rect.y, rect.height, rows.pinned, 62.0, 272.0);
                    assert_eq!(grid.hit(&view, x, y), Hit::Cell { row, column }, "at {x},{y}");
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 11 * 5);
}

#[test]
fn reveal_scrolls_the_least_needed() {
    let grid = layout();
    let mut view = Viewport { width: 400.0, height: 332.0, scroll_x: 0.0, scroll_y: 0.0 };
    assert_eq!(grid.reveal(&view, 3, 1), (0.0, 0.0), "already visible");
    assert_eq!(grid.reveal(&view, 0, 0), (0.0, 0.0), "pinned cells never scroll");
    (view.scroll_x, view.scroll_y) = grid.reveal(&view, 50, 5);
    let rect = grid.cell_rect(&view, 50, 5).unwrap();
    assert_eq!(
        (rect.x + rect.width, rect.y + rect.height),
        (340.0, 272.0),
        "flush with the pinned edges"
    );
    (view.scroll_x, view.scroll_y) = grid.reveal(&view, 2, 1);
    let rect = grid.cell_rect(&view, 2, 1).unwrap();
    assert_eq!((rect.x, rect.y), (80.0, 62.0));
}

#[test]
fn ten_million_rows_scroll_exactly() {
    let rows = Axis::uniform(10_000_000, 24.0);
    let grid = GridLayout {
        rows: rows.clone(),
        columns: Axis::uniform(5, 100.0),
        row_pins: Pins::default(),
        column_pins: Pins::default(),
        header: 0.0,
    };
    let mut view = Viewport { width: 500.0, height: 600.0, scroll_x: 0.0, scroll_y: 0.0 };
    view.scroll_y = grid.max_scroll(&view).1;
    let [_, middle, _] = grid.visible_rows(&view);
    assert_eq!(middle.indices, 9_999_975..10_000_000);
    assert_eq!(middle.start, 0.0);

    let scale = ScrollScale::new(rows.total(), 600.0, 1_000_000.0);
    assert_eq!(scale.scaled_content(), 1_000_000.0);
    assert_eq!(scale.to_scaled(view.scroll_y), 999_400.0);
    assert_eq!(scale.from_scaled(999_400.0), view.scroll_y);
    let halfway = scale.from_scaled(499_700.0);
    assert!((halfway - (rows.total() - 600.0) / 2.0).abs() < 1.0);
    let small = ScrollScale::new(900.0, 600.0, 1_000_000.0);
    assert_eq!((small.scaled_content(), small.to_scaled(150.0)), (900.0, 150.0));

    let mut axis = Axis::uniform(1_000, 24.0);
    let anchor = ScrollAnchor::at(&axis, 24.0 * 500.0 + 7.0);
    axis.set_size(10, 240.0);
    assert_eq!(
        anchor.offset(&axis),
        24.0 * 500.0 + 7.0 + 216.0,
        "the view holds its row when one above grows"
    );
}
