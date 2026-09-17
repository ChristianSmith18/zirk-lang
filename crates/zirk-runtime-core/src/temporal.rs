//! Platform-neutral civil temporal operations for Zirk `Date`, `Time` and
//! `DateTime` values.
//!
//! The representation is shared with the native runtime:
//!
//! - `Date`: an `i64` day count since 1970-01-01 in the proleptic
//!   Gregorian calendar (Howard Hinnant's civil algorithms).
//! - `Time`: an `i64` nanosecond count since midnight, always in
//!   `0..NANOS_PER_DAY`.
//! - `DateTime`: an `i128` count of `days * NANOS_PER_DAY + nanos`.
//!
//! This module contains only value operations; the host clock entry points
//! (`today`, `now_utc`, `now_local`) stay in the native runtime.

/// Nanoseconds in one civil day: `24 * 60 * 60 * 1_000_000_000`.
pub const NANOS_PER_DAY: i64 = 86_400_000_000_000;
/// Nanoseconds in one second.
pub const NANOS_PER_SECOND: i64 = 1_000_000_000;
/// Nanoseconds in one minute.
pub const NANOS_PER_MINUTE: i64 = 60 * NANOS_PER_SECOND;
/// Nanoseconds in one hour.
pub const NANOS_PER_HOUR: i64 = 60 * NANOS_PER_MINUTE;

/// Whether `year` is a leap year in the proleptic Gregorian calendar.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days in `month` of `year` (`month` in `1..=12`).
pub fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 for the civil `year`-`month`-`day` (Hinnant's
/// `days_from_civil`). Assumes the triple was already validated.
pub fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let y = (year as i64) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400; // [0, 399]
    let mp = (month as i64 + 9) % 12; // [0, 11], March = 0
    let doy = (153 * mp + 2) / 5 + (day as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil `year`-`month`-`day` of a day count since 1970-01-01
/// (Hinnant's `civil_from_days`).
pub fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((y + i64::from(m <= 2)) as i32, m as i32, d as i32)
}

/// Whether `year`-`month`-`day` is a real calendar day — never normalized.
pub fn date_is_valid(year: i32, month: i32, day: i32) -> bool {
    (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

/// ISO 8601 `YYYY-MM-DD` rendering of a day count.
pub fn date_to_string(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Whether `hour`-`minute`-`second`-`nanosecond` is a real clock time:
/// `0..24` hours, `0..60` minutes and seconds, `0..1_000_000_000`
/// nanoseconds — `Time(25, 0)` is `false`, never wrapped.
pub fn time_is_valid(hour: i32, minute: i32, second: i32, nanosecond: i64) -> bool {
    (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..60).contains(&second)
        && (0..NANOS_PER_SECOND).contains(&nanosecond)
}

/// The `Time` nanosecond count of a validated clock time.
pub fn time_nanos(hour: i32, minute: i32, second: i32, nanosecond: i64) -> i64 {
    hour as i64 * NANOS_PER_HOUR
        + minute as i64 * NANOS_PER_MINUTE
        + second as i64 * NANOS_PER_SECOND
        + nanosecond
}

/// The hour component of a `Time`.
pub fn time_hour(nanos: i64) -> i32 {
    (nanos / NANOS_PER_HOUR) as i32
}

/// The minute component of a `Time`.
pub fn time_minute(nanos: i64) -> i32 {
    ((nanos % NANOS_PER_HOUR) / NANOS_PER_MINUTE) as i32
}

/// The second component of a `Time`.
pub fn time_second(nanos: i64) -> i32 {
    ((nanos % NANOS_PER_MINUTE) / NANOS_PER_SECOND) as i32
}

/// The sub-second nanosecond fraction (`0..1_000_000_000`).
pub fn time_nanosecond(nanos: i64) -> i64 {
    nanos % NANOS_PER_SECOND
}

/// `Time`(+|-)`Duration`, wrapping modulo one day — `Time(23, 30) + 2h` is
/// `Time(1, 30)`, civil-style.
pub fn time_add_nanos(time: i64, delta: i64) -> i64 {
    (time + delta).rem_euclid(NANOS_PER_DAY)
}

/// `Time - Time`: the `Duration` nanoseconds between them, measured
/// forward around the day (`Time(1, 0) - Time(23, 0)` is `2h`).
pub fn time_diff(left: i64, right: i64) -> i64 {
    (left - right).rem_euclid(NANOS_PER_DAY)
}

/// Renders a `Time` as `HH:MM:SS[.fffffffff]` — the fraction is trimmed of
/// trailing zeros and omitted entirely when it is zero.
pub fn format_time(nanos: i64) -> String {
    let hour = nanos / NANOS_PER_HOUR;
    let minute = (nanos % NANOS_PER_HOUR) / NANOS_PER_MINUTE;
    let second = (nanos % NANOS_PER_MINUTE) / NANOS_PER_SECOND;
    let fraction = nanos % NANOS_PER_SECOND;
    let mut text = format!("{hour:02}:{minute:02}:{second:02}");
    if fraction != 0 {
        let digits = format!("{fraction:09}");
        text.push('.');
        text.push_str(digits.trim_end_matches('0'));
    }
    text
}

/// The `i128` civil timestamp `DateTime(date, time)`/`Date + Time` pack.
pub fn datetime_pack(days: i64, nanos: i64) -> i128 {
    days as i128 * NANOS_PER_DAY as i128 + nanos as i128
}

/// The `Date` half of a `DateTime` (its day count).
pub fn datetime_days(value: i128) -> i64 {
    value.div_euclid(NANOS_PER_DAY as i128) as i64
}

/// The `Time` half of a `DateTime` (its day's nanoseconds).
pub fn datetime_nanos(value: i128) -> i64 {
    value.rem_euclid(NANOS_PER_DAY as i128) as i64
}

/// ISO 8601 `YYYY-MM-DDTHH:MM:SS[.fffffffff]` rendering of a `DateTime`.
pub fn datetime_to_string(value: i128) -> String {
    let (year, month, day) = civil_from_days(datetime_days(value));
    let time = format_time(datetime_nanos(value));
    format!("{year:04}-{month:02}-{day:02}T{time}")
}

/// ISO 8601 weekday: `1` = Monday … `7` = Sunday. Day 0 (1970-01-01) was
/// a Thursday, so `(days + 3) mod 7` counts from Monday.
pub fn date_day_of_week(days: i64) -> i32 {
    ((days + 3).rem_euclid(7) + 1) as i32
}

/// The 1-based ordinal day within `days`' year.
pub fn date_day_of_year(days: i64) -> i32 {
    let (year, _, _) = civil_from_days(days);
    (days - days_from_civil(year, 1, 1) + 1) as i32
}

/// Whether an ISO year holds 53 weeks: it does when January 1 falls on a
/// Thursday, or on a Wednesday of a leap year.
pub fn weeks_in_iso_year(year: i32) -> i32 {
    let jan1 = date_day_of_week(days_from_civil(year, 1, 1));
    if jan1 == 4 || (jan1 == 3 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

/// The ISO 8601 week number: week 1 is the week containing the year's
/// first Thursday.
pub fn date_week_of_year(days: i64) -> i32 {
    let (year, _, _) = civil_from_days(days);
    let week = (i64::from(date_day_of_year(days)) - i64::from(date_day_of_week(days)) + 10) / 7;
    if week < 1 {
        weeks_in_iso_year(year - 1)
    } else if week > i64::from(weeks_in_iso_year(year)) {
        1
    } else {
        week as i32
    }
}

/// The quarter of `days`' month, `1..=4`.
pub fn date_quarter(days: i64) -> i32 {
    (civil_from_days(days).1 - 1) / 3 + 1
}

/// The length of `days`' month.
pub fn date_days_in_month(days: i64) -> i32 {
    let (year, month, _) = civil_from_days(days);
    days_in_month(year, month)
}

/// The length of `days`' year — `366` when leap, `365` otherwise.
pub fn date_days_in_year(days: i64) -> i32 {
    if is_leap_year(civil_from_days(days).0) {
        366
    } else {
        365
    }
}

/// Whether `days`' year is leap.
pub fn date_is_leap(days: i64) -> bool {
    is_leap_year(civil_from_days(days).0)
}

/// The unit names `Date` accepts for `start_of`/`end_of`.
pub fn date_unit_ok(unit: &str) -> bool {
    matches!(unit, "year" | "month" | "week" | "day")
}

/// The unit names `Time` accepts.
pub fn time_unit_ok(unit: &str) -> bool {
    matches!(unit, "day" | "hour" | "minute" | "second")
}

/// The unit names `DateTime` accepts — the union of both.
pub fn datetime_unit_ok(unit: &str) -> bool {
    date_unit_ok(unit) || time_unit_ok(unit)
}

/// The `Date` start-of boundary for a recognized unit.
pub fn date_start_of(days: i64, unit: &str) -> i64 {
    let (_, _, day) = civil_from_days(days);
    match unit {
        "year" => {
            let (year, _, _) = civil_from_days(days);
            days_from_civil(year, 1, 1)
        }
        "month" => days - i64::from(day) + 1,
        // ISO week: back to the preceding Monday.
        "week" => days - i64::from(date_day_of_week(days)) + 1,
        _ => days,
    }
}

/// The `Date` end-of boundary for a recognized unit.
pub fn date_end_of(days: i64, unit: &str) -> i64 {
    let (year, month, day) = civil_from_days(days);
    match unit {
        "year" => days_from_civil(year, 12, 31),
        "month" => days - i64::from(day) + i64::from(days_in_month(year, month)),
        "week" => days + (7 - i64::from(date_day_of_week(days))),
        _ => days,
    }
}

/// The `Time` start-of boundary for a recognized unit.
pub fn time_start_of(nanos: i64, unit: &str) -> i64 {
    match unit {
        "hour" => nanos - nanos % NANOS_PER_HOUR,
        "minute" => nanos - nanos % NANOS_PER_MINUTE,
        "second" => nanos - nanos % NANOS_PER_SECOND,
        _ => 0,
    }
}

/// The `Time` end-of boundary for a recognized unit.
pub fn time_end_of(nanos: i64, unit: &str) -> i64 {
    match unit {
        "hour" => nanos - nanos % NANOS_PER_HOUR + NANOS_PER_HOUR - 1,
        "minute" => nanos - nanos % NANOS_PER_MINUTE + NANOS_PER_MINUTE - 1,
        "second" => nanos - nanos % NANOS_PER_SECOND + NANOS_PER_SECOND - 1,
        _ => NANOS_PER_DAY - 1,
    }
}

/// The `DateTime` start-of boundary: date units zero the clock, time units
/// keep the day.
pub fn datetime_start_of(value: i128, unit: &str) -> i128 {
    let days = datetime_days(value);
    let nanos = datetime_nanos(value);
    if date_unit_ok(unit) {
        datetime_pack(date_start_of(days, unit), 0)
    } else {
        datetime_pack(days, time_start_of(nanos, unit))
    }
}

/// The `DateTime` end-of boundary: date units close the day, time units
/// keep the day.
pub fn datetime_end_of(value: i128, unit: &str) -> i128 {
    let days = datetime_days(value);
    let nanos = datetime_nanos(value);
    if date_unit_ok(unit) {
        datetime_pack(date_end_of(days, unit), NANOS_PER_DAY - 1)
    } else {
        datetime_pack(days, time_end_of(nanos, unit))
    }
}

/// The digit-only `i64` of `text`, or `None` when a non-digit appears.
fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// `YYYY-MM-DD` — strict: exactly ten bytes, dashes at 4 and 7, and the
/// constructor's own range validation.
pub fn parse_date_text(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year = digits(&text[0..4])? as i32;
    let month = digits(&text[5..7])? as i32;
    let day = digits(&text[8..10])? as i32;
    date_is_valid(year, month, day).then(|| days_from_civil(year, month, day))
}

/// `HH:MM[:SS[.fffffffff]]` — the fraction accepts one to nine digits.
pub fn parse_time_text(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() < 5 || b[2] != b':' {
        return None;
    }
    let hour = digits(&text[0..2])? as i32;
    let minute = digits(&text[3..5])? as i32;
    let (second, nanos) = if b.len() == 5 {
        (0, 0)
    } else if b.len() >= 8 && b[5] == b':' {
        let second = digits(&text[6..8])? as i32;
        let nanos = if b.len() == 8 {
            0
        } else if b[8] == b'.' && b.len() <= 18 {
            let frac = &text[9..];
            let mut v = digits(frac)?;
            // Right-pad the fraction to nanosecond digits.
            for _ in frac.len()..9 {
                v *= 10;
            }
            v
        } else {
            return None;
        };
        (second, nanos)
    } else {
        return None;
    };
    time_is_valid(hour, minute, second, nanos).then(|| time_nanos(hour, minute, second, nanos))
}

/// `YYYY-MM-DD[T ]HH:MM[:SS[.frac]]`.
pub fn parse_datetime_text(text: &str) -> Option<i128> {
    let b = text.as_bytes();
    if b.len() < 16 || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let days = parse_date_text(&text[..10])?;
    let nanos = parse_time_text(&text[11..])?;
    Some(datetime_pack(days, nanos))
}

/// Renders the civil components under `pattern`. Recognized tokens are
/// `YYYY`, `MM`, `DD`, `HH`, `mm`, `ss` and `SSS` (milliseconds); any
/// other text passes through literally — `format` never fails.
#[allow(clippy::too_many_arguments)]
pub fn format_pattern(
    pattern: &str,
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    nanos: i64,
) -> String {
    const TOKENS: [&str; 7] = ["YYYY", "SSS", "MM", "DD", "HH", "mm", "ss"];
    let mut out = String::new();
    let mut i = 0;
    while i < pattern.len() {
        let rest = &pattern[i..];
        match TOKENS.iter().find(|t| rest.starts_with(**t)) {
            Some(&"YYYY") => out.push_str(&format!("{year:04}")),
            Some(&"SSS") => out.push_str(&format!("{:03}", nanos / 1_000_000)),
            Some(&"MM") => out.push_str(&format!("{month:02}")),
            Some(&"DD") => out.push_str(&format!("{day:02}")),
            Some(&"HH") => out.push_str(&format!("{hour:02}")),
            Some(&"mm") => out.push_str(&format!("{minute:02}")),
            Some(&"ss") => out.push_str(&format!("{second:02}")),
            _ => {
                // Literal text: copy the whole (possibly multi-byte) char.
                let ch = rest.chars().next().expect("rest is not empty");
                out.push(ch);
                i += ch.len_utf8();
                continue;
            }
        }
        i += TOKENS
            .iter()
            .find(|t| rest.starts_with(**t))
            .expect("matched above")
            .len();
    }
    out
}

/// `d.format(pattern)` — the pattern applied to the date fields; time
/// tokens render `00`.
pub fn date_format(days: i64, pattern: &str) -> String {
    let (year, month, day) = civil_from_days(days);
    format_pattern(pattern, year, month, day, 0, 0, 0, 0)
}

/// `t.format(pattern)` — the pattern applied to the time fields; date
/// tokens render `00`/`0000`.
pub fn time_format(nanos: i64, pattern: &str) -> String {
    format_pattern(
        pattern,
        0,
        0,
        0,
        time_hour(nanos),
        time_minute(nanos),
        time_second(nanos),
        time_nanosecond(nanos),
    )
}

/// `dt.format(pattern)` — the pattern applied to all fields.
pub fn datetime_format(value: i128, pattern: &str) -> String {
    let (year, month, day) = civil_from_days(datetime_days(value));
    let nanos = datetime_nanos(value);
    format_pattern(
        pattern,
        year,
        month,
        day,
        time_hour(nanos),
        time_minute(nanos),
        time_second(nanos),
        time_nanosecond(nanos),
    )
}
