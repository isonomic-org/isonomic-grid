use std::fmt;
use std::sync::Arc;

use grid_core::{ColumnSpec, ColumnType, Decimal, Value, fold};

use crate::date::{date_from_days, datetime_from_micros, days_from_date, micros_from_datetime};

/// Cell text that is not a value of the column's type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub expected: ColumnType,
    pub text: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid {:?}", self.text, self.expected)
    }
}

impl std::error::Error for ParseError {}

fn parse_date(text: &str) -> Option<i32> {
    let mut parts = text.splitn(3, '-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    days_from_date(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
}

/// `2026-11-25T14:30`, with optional seconds, fraction and offset.
/// A time with no offset is taken as UTC.
fn parse_datetime(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once(['T', ' '])?;
    let days = parse_date(date)?;
    let (clock, offset) = match rest.find(['Z', 'z', '+', '-']) {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    let mut parts = clock.splitn(3, ':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let (second, micros) = match parts.next() {
        None => (0, 0),
        Some(seconds) => {
            let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
            if fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let padded = format!("{fraction:0<6}");
            (whole.parse().ok()?, padded[..6].parse().ok()?)
        }
    };
    let shift = match offset {
        "" | "Z" | "z" => 0,
        other => {
            let sign = if other.starts_with('-') { -1 } else { 1 };
            let (hours, minutes) = other[1..].split_once(':')?;
            sign * (hours.parse::<i64>().ok()? * 60 + minutes.parse::<i64>().ok()?)
        }
    };
    Some(micros_from_datetime(days, hour, minute, second, micros)? - shift * 60_000_000)
}

fn parse_decimal(text: &str) -> Option<Decimal> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.len() > 28 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    Some(Decimal { mantissa: digits.parse().ok()?, scale: fraction.len() as u8 })
}

fn parse_bytes(text: &str) -> Option<Arc<[u8]>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect();
    bytes.map(Arc::from)
}

fn choice(spec: &ColumnSpec, label: &str) -> Option<u32> {
    let wanted = fold(label.trim());
    spec.choices.iter().position(|known| fold(known) == wanted).map(|index| index as u32)
}

/// Reads cell text as a value of the column's type. Empty text is
/// null.
pub fn parse_value(text: &str, spec: &ColumnSpec) -> Result<Value, ParseError> {
    let trimmed = text.trim();
    if trimmed.is_empty() && spec.ty != ColumnType::Text {
        return Ok(Value::Null);
    }
    let value = match spec.ty {
        ColumnType::Text if text.is_empty() => Some(Value::Null),
        ColumnType::Text => Some(Value::text(text)),
        ColumnType::Json => Some(Value::Json(Arc::from(trimmed))),
        ColumnType::Custom => Some(Value::Custom(Arc::from(trimmed))),
        ColumnType::Bool => match fold(trimmed).as_str() {
            "true" | "yes" | "1" => Some(Value::Bool(true)),
            "false" | "no" | "0" => Some(Value::Bool(false)),
            _ => None,
        },
        ColumnType::Int => trimmed.parse().ok().map(Value::Int),
        ColumnType::Float => trimmed.parse().ok().map(Value::Float),
        ColumnType::Decimal => parse_decimal(trimmed).map(Value::Decimal),
        ColumnType::Date => parse_date(trimmed).map(Value::Date),
        ColumnType::DateTime => parse_datetime(trimmed).map(Value::DateTime),
        ColumnType::Duration => trimmed
            .parse::<f64>()
            .ok()
            .filter(|seconds| seconds.is_finite())
            .map(|seconds| Value::Duration((seconds * 1_000_000.0).round() as i64)),
        ColumnType::Choice => choice(spec, trimmed).map(Value::Choice),
        ColumnType::Choices => trimmed
            .split(';')
            .map(|label| choice(spec, label))
            .collect::<Option<Vec<u32>>>()
            .map(Value::Choices),
        ColumnType::Ref => trimmed.parse().ok().map(Value::Ref),
        ColumnType::Bytes => parse_bytes(trimmed).map(Value::Bytes),
    };
    value.ok_or_else(|| ParseError { expected: spec.ty, text: text.to_owned() })
}

fn date_text(days: i32) -> String {
    let (year, month, day) = date_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The text form of a value, which [`parse_value`] reads back.
pub fn format_value(value: &Value, spec: &ColumnSpec) -> String {
    let label = |index: &u32| {
        spec.choices.get(*index as usize).cloned().unwrap_or_else(|| index.to_string())
    };
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Decimal(v) if v.scale == 0 => v.mantissa.to_string(),
        Value::Decimal(v) => {
            let scale = usize::from(v.scale);
            let digits = format!("{:0>width$}", v.mantissa.unsigned_abs(), width = scale + 1);
            let sign = if v.mantissa < 0 { "-" } else { "" };
            format!("{sign}{}.{}", &digits[..digits.len() - scale], &digits[digits.len() - scale..])
        }
        Value::Text(v) | Value::Json(v) | Value::Custom(v) => v.to_string(),
        Value::Date(v) => date_text(*v),
        Value::DateTime(v) => {
            let (days, rest) = datetime_from_micros(*v);
            let seconds = rest / 1_000_000;
            let fraction = match rest % 1_000_000 {
                0 => String::new(),
                micros => format!(".{micros:06}"),
            };
            let (h, m, s) = (seconds / 3_600, seconds / 60 % 60, seconds % 60);
            format!("{}T{h:02}:{m:02}:{s:02}{fraction}Z", date_text(days))
        }
        Value::Duration(v) => (*v as f64 / 1_000_000.0).to_string(),
        Value::Choice(v) => label(v),
        Value::Choices(v) => v.iter().map(label).collect::<Vec<_>>().join(";"),
        Value::Ref(v) => v.to_string(),
        Value::Bytes(v) => v.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}
