// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A fixed-point number: `mantissa / 10^scale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Decimal {
    pub mantissa: i128,
    pub scale: u8,
}

impl Decimal {
    pub fn to_f64(self) -> f64 {
        self.mantissa as f64 / 10f64.powi(i32::from(self.scale))
    }
}

/// One cell's value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(Decimal),
    Text(Arc<str>),
    /// Days since 1970-01-01.
    Date(i32),
    /// Microseconds since 1970-01-01T00:00:00Z. Always UTC.
    DateTime(i64),
    /// Microseconds.
    Duration(i64),
    /// An index into the column's choices.
    Choice(u32),
    Choices(Vec<u32>),
    /// A reference to a row in another table.
    Ref(u64),
    Json(Arc<str>),
    Bytes(Arc<[u8]>),
    /// A value of a host-defined type, in its text form.
    Custom(Arc<str>),
}

impl Value {
    pub fn text(text: &str) -> Self {
        Self::Text(Arc::from(text))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The value as a number, for numeric types.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(v) | Self::DateTime(v) | Self::Duration(v) => Some(*v as f64),
            Self::Float(v) => Some(*v),
            Self::Decimal(v) => Some(v.to_f64()),
            Self::Date(v) => Some(f64::from(*v)),
            Self::Choice(v) => Some(f64::from(*v)),
            Self::Bool(v) => Some(f64::from(u8::from(*v))),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(v) | Self::Json(v) | Self::Custom(v) => Some(v),
            _ => None,
        }
    }

    /// Where a value falls among the three sort classes: ordinary
    /// values, then not-a-number, then null.
    pub fn class(&self) -> u8 {
        match self {
            Self::Null => 2,
            Self::Float(v) if v.is_nan() => 1,
            _ => 0,
        }
    }
}

/// Orders two values the way every executor must: by class first
/// (values, not-a-number, null), numbers by magnitude across numeric
/// types, text by folded form then by bytes.
pub fn compare(a: &Value, b: &Value) -> Ordering {
    let classes = a.class().cmp(&b.class());
    if classes != Ordering::Equal || a.class() != 0 {
        return classes;
    }
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => fold(x).cmp(&fold(y)).then_with(|| x.cmp(y)),
        (Value::Choices(x), Value::Choices(y)) => x.cmp(y),
        (Value::Bytes(x), Value::Bytes(y)) => x.cmp(y),
        _ => match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => match (a.as_str(), b.as_str()) {
                (Some(x), Some(y)) => x.cmp(y),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
        },
    }
}

/// Text folded for search and for sort keys: lower case, with the
/// common Latin accents removed.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars().flat_map(char::to_lowercase) {
        out.push(match ch {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è'..='ë' | 'ē' | 'ę' | 'ě' => 'e',
            'ì'..='ï' | 'ī' | 'į' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            // Combining accents left by decomposed input.
            '\u{300}'..='\u{36f}' => continue,
            other => other,
        });
    }
    out
}
