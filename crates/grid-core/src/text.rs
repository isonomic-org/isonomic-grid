// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::fold;

/// A text column, dictionary-encoded: each distinct string is kept
/// once, and a row holds its code.
///
/// Sorting, grouping and filtering work on the dictionary and then on
/// codes, so their cost follows the number of distinct strings, not
/// the number of rows. The dictionary's sort order is computed once
/// and kept until a new string arrives.
#[derive(Clone, Debug, Default)]
pub struct TextColumn {
    codes: Vec<u32>,
    valid: Vec<bool>,
    dict: Vec<Arc<str>>,
    lookup: HashMap<Arc<str>, u32>,
    ranks: OnceLock<Arc<Vec<u32>>>,
}

impl TextColumn {
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    pub fn is_valid(&self, row: usize) -> bool {
        self.valid.get(row).copied().unwrap_or(false)
    }

    /// Each row's code. A null row's code is meaningless.
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    pub fn validity(&self) -> &[bool] {
        &self.valid
    }

    /// The distinct strings, by code. Some may no longer be in use.
    pub fn dictionary(&self) -> &[Arc<str>] {
        &self.dict
    }

    /// The code of a string, if any row has ever held it.
    pub fn code_of(&self, text: &str) -> Option<u32> {
        self.lookup.get(text).copied()
    }

    /// The row's string. Callers have checked the row is in range.
    pub fn get(&self, row: usize) -> &Arc<str> {
        &self.dict[self.codes[row] as usize]
    }

    /// Each code's place in the folded sort order: by folded form,
    /// then by bytes. Distinct strings never share a rank.
    pub fn ranks(&self) -> Arc<Vec<u32>> {
        self.ranks
            .get_or_init(|| {
                let mut order: Vec<(String, &str, u32)> = self
                    .dict
                    .iter()
                    .enumerate()
                    .map(|(code, text)| (fold(text), &**text, code as u32))
                    .collect();
                order.sort_unstable();
                let mut ranks = vec![0; order.len()];
                for (rank, (_, _, code)) in order.iter().enumerate() {
                    ranks[*code as usize] = rank as u32;
                }
                Arc::new(ranks)
            })
            .clone()
    }

    fn intern(&mut self, text: &Arc<str>) -> u32 {
        if let Some(code) = self.lookup.get(text) {
            return *code;
        }
        let code = self.dict.len() as u32;
        self.dict.push(text.clone());
        self.lookup.insert(text.clone(), code);
        self.ranks = OnceLock::new();
        code
    }

    /// Writes a row; `row == len` appends.
    pub(crate) fn put(&mut self, row: usize, text: &Arc<str>, present: bool) {
        let code = if present { self.intern(text) } else { 0 };
        if row >= self.codes.len() {
            self.codes.push(code);
            self.valid.push(present);
        } else {
            self.codes[row] = code;
            self.valid[row] = present;
        }
    }

    pub(crate) fn remove(&mut self, row: usize) {
        self.codes.remove(row);
        self.valid.remove(row);
    }
}
