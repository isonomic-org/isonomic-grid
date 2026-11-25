// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Calendar arithmetic for the proleptic Gregorian calendar, in UTC.

const MICROS_PER_DAY: i64 = 86_400_000_000;

/// Days since 1970-01-01 for a calendar date, if the date exists.
pub fn days_from_date(year: i32, month: u32, day: u32) -> Option<i32> {
    if !(1..=12).contains(&month) || day == 0 || !(-9999..=9999).contains(&year) {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let length = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day > length {
        return None;
    }
    let y = i64::from(if month <= 2 { year - 1 } else { year });
    let era = y.div_euclid(400);
    let year_of_era = y.rem_euclid(400);
    let shifted = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let day_of_year = (153 * shifted + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097 + day_of_era - 719_468) as i32)
}

/// The calendar date for a day count.
pub fn date_from_days(days: i32) -> (i32, u32, u32) {
    let z = i64::from(days) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted + 2) / 5 + 1) as u32;
    let month = if shifted < 10 { shifted + 3 } else { shifted - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year as i32, month, day)
}

/// Microseconds since the epoch for a UTC date and time of day.
pub fn micros_from_datetime(
    days: i32,
    hour: u32,
    minute: u32,
    second: u32,
    micros: u32,
) -> Option<i64> {
    if hour > 23 || minute > 59 || second > 59 || micros > 999_999 {
        return None;
    }
    let time = (i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second)) * 1_000_000;
    Some(i64::from(days) * MICROS_PER_DAY + time + i64::from(micros))
}

/// The UTC day count and the microseconds into that day.
pub fn datetime_from_micros(micros: i64) -> (i32, i64) {
    (micros.div_euclid(MICROS_PER_DAY) as i32, micros.rem_euclid(MICROS_PER_DAY))
}
