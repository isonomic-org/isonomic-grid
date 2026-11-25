// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A stable sort of row positions by one key column.
//!
//! Keys are small fixed-width numbers, so a byte-wise radix sort
//! orders a million rows in a few passes over memory, and skips every
//! byte position where all keys agree: a low-cardinality column or a
//! column of small integers costs one or two passes.

use crate::keys::Key;

/// Below this many rows a comparison sort is as fast and simpler.
const SMALL: usize = 2_048;

/// Reorders `order`, positions into `keys`, by key. Equal keys keep
/// their relative order.
pub fn sort_by(order: &mut Vec<u32>, keys: &[Key]) {
    let len = order.len();
    if len < SMALL {
        order.sort_by_key(|at| keys[*at as usize]);
        return;
    }
    let mut items: Vec<(u64, u32)> = order.iter().map(|at| (keys[*at as usize].1, *at)).collect();
    let mut counts = [[0usize; 256]; 8];
    for (rank, _) in &items {
        for (byte, count) in counts.iter_mut().enumerate() {
            count[(rank >> (byte * 8)) as usize & 0xff] += 1;
        }
    }
    let mut scratch = vec![(0u64, 0u32); len];
    for (byte, count) in counts.iter().enumerate() {
        if count.contains(&len) {
            continue;
        }
        let mut next = [0usize; 256];
        let mut total = 0;
        for (slot, size) in next.iter_mut().zip(count) {
            *slot = total;
            total += size;
        }
        for item in &items {
            let digit = (item.0 >> (byte * 8)) as usize & 0xff;
            scratch[next[digit]] = *item;
            next[digit] += 1;
        }
        std::mem::swap(&mut items, &mut scratch);
    }
    // Values, then not-a-number, then nulls: one more stable pass,
    // skipped when every key is an ordinary value.
    order.clear();
    if items.iter().all(|(_, at)| keys[*at as usize].0 == 0) {
        order.extend(items.iter().map(|(_, at)| *at));
        return;
    }
    for class in 0..=2u8 {
        order.extend(items.iter().map(|(_, at)| *at).filter(|at| keys[*at as usize].0 == class));
    }
}
