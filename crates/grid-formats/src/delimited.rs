// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

/// Reads delimited text into rows of fields. Quoted fields may hold
/// the delimiter, quotes (doubled) and line breaks. Any input is
/// accepted; a quote left open runs to the end.
pub fn read_delimited(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = text.strip_prefix('\u{feff}').unwrap_or(text).chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                other => field.push(other),
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => {
                quoted = true;
                started = true;
            }
            ch if ch == delimiter => {
                row.push(std::mem::take(&mut field));
                started = true;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                if started || !field.is_empty() {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                started = false;
            }
            other => field.push(other),
        }
    }
    if started || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Writes rows of fields as delimited text, quoting where needed.
pub fn write_delimited<R, F>(rows: R, delimiter: char) -> String
where
    R: IntoIterator<Item = F>,
    F: IntoIterator<Item = String>,
{
    let mut out = String::new();
    for row in rows {
        for (index, field) in row.into_iter().enumerate() {
            if index > 0 {
                out.push(delimiter);
            }
            let quote = field.contains([delimiter, '"', '\n', '\r'])
                || field.starts_with(' ')
                || field.ends_with(' ');
            if quote {
                out.push('"');
                out.push_str(&field.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(&field);
            }
        }
        out.push('\n');
    }
    out
}
