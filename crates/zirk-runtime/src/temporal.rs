//! Civil temporal values (roadmap Phase 7, `date-and-time-types`).
//!
//! `Date`, `Time` and `DateTime` are immutable *civil* values — a calendar
//! day, a clock time of day, and their composition — with no zone attached.
//! The representation travels as plain integers across the FFI boundary:
//!
//! - `Date`: an `i64` day count since 1970-01-01 in the proleptic
//!   Gregorian calendar (the same civil algorithms Howard Hinnant's
//!   `date` library uses — `days_from_civil`/`civil_from_days` below).
//! - `Time`: an `i64` nanosecond count since midnight, always in
//!   `0..86_400_000_000_000`.
//! - `DateTime`: an `i128` count of `days * NANOS_PER_DAY + nanos` — epoch
//!   nanoseconds for the civil clock, where the day's nanos alone fit an
//!   `i64` but the composed value does not.
//!
//! Validation is split in two, mirroring `zirk_float_parse_ok`/
//! `zirk_float_parse_value`: `zirk_rt_*_is_valid` answers the question so
//! `zirk-ir` can throw `InvalidDateError`/`InvalidTimeError` on the failing
//! branch, and `zirk_rt_date_days`/`zirk_rt_time_nanos` compute the value
//! itself on the passing one — they never see an out-of-range input.

use std::ffi::c_void;

use crate::string::alloc_owned;

/// Nanoseconds in one civil day: `24 * 60 * 60 * 1_000_000_000`.
const NANOS_PER_DAY: i64 = 86_400_000_000_000;
const NANOS_PER_SECOND: i64 = 1_000_000_000;
const NANOS_PER_MINUTE: i64 = 60 * NANOS_PER_SECOND;
const NANOS_PER_HOUR: i64 = 60 * NANOS_PER_MINUTE;

/// Whether `year` is a leap year in the proleptic Gregorian calendar.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days in `month` of `year` (`month` in `1..=12`).
fn days_in_month(year: i32, month: i32) -> i32 {
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
fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
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
fn civil_from_days(days: i64) -> (i32, i32, i32) {
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

/// Renders a `Time` as `HH:MM:SS[.fffffffff]` — the fraction is trimmed of
/// trailing zeros and omitted entirely when it is zero.
fn format_time(nanos: i64) -> String {
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

/// The host's current civil time, resolved through the C library's
/// `localtime` so `now_local` answers the machine's own clock rather than
/// UTC. On non-Unix targets (which have no `localtime_r`) this falls back
/// to the UTC civil time — the same value `now_utc` reports.
#[cfg(unix)]
fn local_civil_now() -> (i64, i64) {
    // `struct tm`: the first nine POSIX fields are `c_int` on every Unix
    // target; the trailing `tm_gmtoff`/`tm_zone` are never read.
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const c_void,
    }
    unsafe extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }
    let now = unsafe { time(std::ptr::null_mut()) };
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    if unsafe { localtime_r(&now, &mut tm) }.is_null() {
        return utc_civil_now();
    }
    let days = days_from_civil(tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday);
    let nanos = tm.tm_hour as i64 * NANOS_PER_HOUR
        + tm.tm_min as i64 * NANOS_PER_MINUTE
        + tm.tm_sec as i64 * NANOS_PER_SECOND;
    (days, nanos)
}

/// The current UTC civil time from `SystemTime::now()`.
fn utc_civil_now() -> (i64, i64) {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let nanos =
        (seconds.rem_euclid(86_400)) * NANOS_PER_SECOND + i64::from(duration.subsec_nanos());
    (days, nanos)
}

#[cfg(not(unix))]
fn local_civil_now() -> (i64, i64) {
    utc_civil_now()
}

/// Whether `year`-`month`-`day` is a real calendar day. `month` outside
/// `1..=12` or a `day` that does not exist in it (2026-02-30) is `false` —
/// never normalized.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_is_valid(year: i32, month: i32, day: i32) -> bool {
    (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

/// The `Date` day count of a validated civil `year`-`month`-`day`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_days(year: i32, month: i32, day: i32) -> i64 {
    days_from_civil(year, month, day)
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_year(days: i64) -> i32 {
    civil_from_days(days).0
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_month(days: i64) -> i32 {
    civil_from_days(days).1
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_day(days: i64) -> i32 {
    civil_from_days(days).2
}

/// `Date.today()` — the host's current civil date.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_today() -> i64 {
    local_civil_now().0
}

/// ISO 8601 `YYYY-MM-DD`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_to_string(days: i64) -> *mut c_void {
    let (year, month, day) = civil_from_days(days);
    alloc_owned(&format!("{year:04}-{month:02}-{day:02}"))
}

/// Whether `hour`-`minute`-`second`-`nanosecond` is a real clock time:
/// `0..24` hours, `0..60` minutes and seconds, `0..1_000_000_000`
/// nanoseconds — `Time(25, 0)` is `false`, never wrapped.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_is_valid(
    hour: i32,
    minute: i32,
    second: i32,
    nanosecond: i64,
) -> bool {
    (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..60).contains(&second)
        && (0..NANOS_PER_SECOND).contains(&nanosecond)
}

/// The `Time` nanosecond count of a validated clock time.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_nanos(hour: i32, minute: i32, second: i32, nanosecond: i64) -> i64 {
    hour as i64 * NANOS_PER_HOUR
        + minute as i64 * NANOS_PER_MINUTE
        + second as i64 * NANOS_PER_SECOND
        + nanosecond
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_hour(nanos: i64) -> i32 {
    (nanos / NANOS_PER_HOUR) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_minute(nanos: i64) -> i32 {
    ((nanos % NANOS_PER_HOUR) / NANOS_PER_MINUTE) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_second(nanos: i64) -> i32 {
    ((nanos % NANOS_PER_MINUTE) / NANOS_PER_SECOND) as i32
}

/// The sub-second nanosecond fraction (`0..1_000_000_000`).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_nanosecond(nanos: i64) -> i64 {
    nanos % NANOS_PER_SECOND
}

/// `Time`(+|-)`Duration`, wrapping modulo one day — `Time(23, 30) + 2h` is
/// `Time(1, 30)`, civil-style.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_add_nanos(time: i64, delta: i64) -> i64 {
    (time + delta).rem_euclid(NANOS_PER_DAY)
}

/// `Time - Time`: the `Duration` nanoseconds between them, measured
/// forward around the day (`Time(1, 0) - Time(23, 0)` is `2h`).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_diff(left: i64, right: i64) -> i64 {
    (left - right).rem_euclid(NANOS_PER_DAY)
}

/// `Time.now_utc()` — the current UTC clock time.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_now_utc() -> i64 {
    utc_civil_now().1
}

/// `Time.now_local()` — the host's current civil clock time.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_now_local() -> i64 {
    local_civil_now().1
}

/// ISO 8601 `HH:MM:SS[.fffffffff]`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_to_string(nanos: i64) -> *mut c_void {
    alloc_owned(&format_time(nanos))
}

/// `DateTime(date, time)` / `Date + Time`: packs the day count and the
/// day's nanoseconds into the `i128` civil timestamp.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_new(days: i64, nanos: i64) -> i128 {
    days as i128 * NANOS_PER_DAY as i128 + nanos as i128
}

/// The `Date` half of a `DateTime` (its day count).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_days(value: i128) -> i64 {
    value.div_euclid(NANOS_PER_DAY as i128) as i64
}

/// The `Time` half of a `DateTime` (its day's nanoseconds).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_nanos(value: i128) -> i64 {
    value.rem_euclid(NANOS_PER_DAY as i128) as i64
}

/// `DateTime.now_utc()` — the current UTC civil timestamp.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_now_utc() -> i128 {
    let (days, nanos) = utc_civil_now();
    zirk_rt_datetime_new(days, nanos)
}

/// `DateTime.now_local()` — the host's current civil timestamp.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_now_local() -> i128 {
    let (days, nanos) = local_civil_now();
    zirk_rt_datetime_new(days, nanos)
}

/// ISO 8601 `YYYY-MM-DDTHH:MM:SS[.fffffffff]`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_datetime_to_string(value: i128) -> *mut c_void {
    let (year, month, day) = civil_from_days(zirk_rt_datetime_days(value));
    let time = format_time(zirk_rt_datetime_nanos(value));
    alloc_owned(&format!("{year:04}-{month:02}-{day:02}T{time}"))
}

// --- Calendrical properties (`temporal-rich-api`) --------------------------
//
// Everything below is a pure projection of the day count / nanosecond
// representation — no allocation, no validation needed (the value was
// already validated at construction).

/// ISO 8601 weekday: `1` = Monday … `7` = Sunday. Day 0 (1970-01-01) was
/// a Thursday, so `(days + 3) mod 7` counts from Monday.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_day_of_week(days: i64) -> i32 {
    ((days + 3).rem_euclid(7) + 1) as i32
}

/// The 1-based ordinal day within `days`' year.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_day_of_year(days: i64) -> i32 {
    let (year, _, _) = civil_from_days(days);
    (days - days_from_civil(year, 1, 1) + 1) as i32
}

/// Whether an ISO year holds 53 weeks: it does when January 1 falls on a
/// Thursday, or on a Wednesday of a leap year.
fn weeks_in_iso_year(year: i32) -> i32 {
    let jan1 = zirk_rt_date_day_of_week(days_from_civil(year, 1, 1));
    if jan1 == 4 || (jan1 == 3 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

/// The ISO 8601 week number: week 1 is the week containing the year's
/// first Thursday.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_week_of_year(days: i64) -> i32 {
    let (year, _, _) = civil_from_days(days);
    let week = (i64::from(zirk_rt_date_day_of_year(days))
        - i64::from(zirk_rt_date_day_of_week(days))
        + 10)
        / 7;
    if week < 1 {
        weeks_in_iso_year(year - 1)
    } else if week > i64::from(weeks_in_iso_year(year)) {
        1
    } else {
        week as i32
    }
}

/// The quarter of `days`' month, `1..=4`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_quarter(days: i64) -> i32 {
    (zirk_rt_date_month(days) - 1) / 3 + 1
}

/// The length of `days`' month.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_days_in_month(days: i64) -> i32 {
    let (year, month, _) = civil_from_days(days);
    days_in_month(year, month)
}

/// The length of `days`' year — `366` when leap, `365` otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_days_in_year(days: i64) -> i32 {
    if is_leap_year(civil_from_days(days).0) {
        366
    } else {
        365
    }
}

/// Whether `days`' year is leap.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_is_leap(days: i64) -> bool {
    is_leap_year(civil_from_days(days).0)
}

// --- Unit boundaries -------------------------------------------------------
//
// `start_of`/`end_of` share the `parse` convention: an `ok` probe answers
// whether the unit name is recognized so `zirk-ir` can build the `Error`
// arm of the `Result`, and the `value` entrypoint computes the boundary on
// the passing branch — it never sees an unknown unit.

/// The unit names `Date` accepts for `start_of`/`end_of`.
fn date_unit_ok(unit: &str) -> bool {
    matches!(unit, "year" | "month" | "week" | "day")
}

/// The unit names `Time` accepts.
fn time_unit_ok(unit: &str) -> bool {
    matches!(unit, "day" | "hour" | "minute" | "second")
}

/// The unit names `DateTime` accepts — the union of both.
fn datetime_unit_ok(unit: &str) -> bool {
    date_unit_ok(unit) || time_unit_ok(unit)
}

/// Reads a `String` handle as `&str` (empty on a non-string or null).
unsafe fn unit_str<'a>(handle: *const c_void) -> &'a str {
    unsafe { crate::string::borrow(handle) }
        .map(|s| unsafe { s.as_str() })
        .unwrap_or("")
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_start_of_ok(unit: *const c_void) -> bool {
    date_unit_ok(unsafe { unit_str(unit) })
}

fn date_start_of(days: i64, unit: &str) -> i64 {
    let (_, _, day) = civil_from_days(days);
    match unit {
        "year" => {
            let (year, _, _) = civil_from_days(days);
            days_from_civil(year, 1, 1)
        }
        "month" => days - i64::from(day) + 1,
        // ISO week: back to the preceding Monday.
        "week" => days - i64::from(zirk_rt_date_day_of_week(days)) + 1,
        _ => days,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_start_of_value(days: i64, unit: *const c_void) -> i64 {
    date_start_of(days, unsafe { unit_str(unit) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_end_of_ok(unit: *const c_void) -> bool {
    unsafe { zirk_rt_date_start_of_ok(unit) }
}

fn date_end_of(days: i64, unit: &str) -> i64 {
    let (year, month, day) = civil_from_days(days);
    match unit {
        "year" => days_from_civil(year, 12, 31),
        "month" => days - i64::from(day) + i64::from(days_in_month(year, month)),
        "week" => days + (7 - i64::from(zirk_rt_date_day_of_week(days))),
        _ => days,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_end_of_value(days: i64, unit: *const c_void) -> i64 {
    date_end_of(days, unsafe { unit_str(unit) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_start_of_ok(unit: *const c_void) -> bool {
    time_unit_ok(unsafe { unit_str(unit) })
}

fn time_start_of(nanos: i64, unit: &str) -> i64 {
    match unit {
        "hour" => nanos - nanos % NANOS_PER_HOUR,
        "minute" => nanos - nanos % NANOS_PER_MINUTE,
        "second" => nanos - nanos % NANOS_PER_SECOND,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_start_of_value(nanos: i64, unit: *const c_void) -> i64 {
    time_start_of(nanos, unsafe { unit_str(unit) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_end_of_ok(unit: *const c_void) -> bool {
    unsafe { zirk_rt_time_start_of_ok(unit) }
}

fn time_end_of(nanos: i64, unit: &str) -> i64 {
    match unit {
        "hour" => nanos - nanos % NANOS_PER_HOUR + NANOS_PER_HOUR - 1,
        "minute" => nanos - nanos % NANOS_PER_MINUTE + NANOS_PER_MINUTE - 1,
        "second" => nanos - nanos % NANOS_PER_SECOND + NANOS_PER_SECOND - 1,
        _ => NANOS_PER_DAY - 1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_end_of_value(nanos: i64, unit: *const c_void) -> i64 {
    time_end_of(nanos, unsafe { unit_str(unit) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_start_of_ok(unit: *const c_void) -> bool {
    datetime_unit_ok(unsafe { unit_str(unit) })
}

/// The `DateTime` start-of boundary: date units zero the clock, time units
/// keep the day.
fn datetime_start_of(value: i128, unit: &str) -> i128 {
    let days = zirk_rt_datetime_days(value);
    let nanos = zirk_rt_datetime_nanos(value);
    if date_unit_ok(unit) {
        zirk_rt_datetime_new(date_start_of(days, unit), 0)
    } else {
        zirk_rt_datetime_new(days, time_start_of(nanos, unit))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_start_of_value(value: i128, unit: *const c_void) -> i128 {
    datetime_start_of(value, unsafe { unit_str(unit) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_end_of_ok(unit: *const c_void) -> bool {
    unsafe { zirk_rt_datetime_start_of_ok(unit) }
}

fn datetime_end_of(value: i128, unit: &str) -> i128 {
    let days = zirk_rt_datetime_days(value);
    let nanos = zirk_rt_datetime_nanos(value);
    if date_unit_ok(unit) {
        zirk_rt_datetime_new(date_end_of(days, unit), NANOS_PER_DAY - 1)
    } else {
        zirk_rt_datetime_new(days, time_end_of(nanos, unit))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_end_of_value(value: i128, unit: *const c_void) -> i128 {
    datetime_end_of(value, unsafe { unit_str(unit) })
}

// --- ISO 8601 parsing (`temporal-rich-api`) --------------------------------
//
// Strict canonical forms only — `YYYY-MM-DD`, `HH:MM[:SS[.frac]]`, and the
// date-time composition with `T` or a space. The `ok`/`value` split mirrors
// `zirk_int_parse_ok`/`zirk_int_parse_value`: the probe answers whether the
// text parses so `zirk-ir` can build the `Error` arm, and the value
// entrypoint re-parses on the passing branch.

/// The digit-only `i64` of `text`, or `None` when a non-digit appears.
fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// `YYYY-MM-DD` — strict: exactly ten bytes, dashes at 4 and 7, and the
/// constructor's own range validation.
fn parse_date_text(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year = digits(&text[0..4])? as i32;
    let month = digits(&text[5..7])? as i32;
    let day = digits(&text[8..10])? as i32;
    zirk_rt_date_is_valid(year, month, day).then(|| days_from_civil(year, month, day))
}

/// `HH:MM[:SS[.fffffffff]]` — the fraction accepts one to nine digits.
fn parse_time_text(text: &str) -> Option<i64> {
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
    zirk_rt_time_is_valid(hour, minute, second, nanos)
        .then(|| zirk_rt_time_nanos(hour, minute, second, nanos))
}

/// `YYYY-MM-DD[T ]HH:MM[:SS[.frac]]`.
fn parse_datetime_text(text: &str) -> Option<i128> {
    let b = text.as_bytes();
    if b.len() < 16 || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let days = parse_date_text(&text[..10])?;
    let nanos = parse_time_text(&text[11..])?;
    Some(zirk_rt_datetime_new(days, nanos))
}

/// `Date.parse(text)` — `ok` flag: the text is a strict `YYYY-MM-DD`.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_parse_ok(text: *const c_void) -> bool {
    parse_date_text(unsafe { unit_str(text) }).is_some()
}

/// `Date.parse(text)` — the day count; only read when the `ok` probe passed.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_parse_value(text: *const c_void) -> i64 {
    parse_date_text(unsafe { unit_str(text) }).unwrap_or(0)
}

/// `Time.parse(text)` — `ok` flag.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_parse_ok(text: *const c_void) -> bool {
    parse_time_text(unsafe { unit_str(text) }).is_some()
}

/// `Time.parse(text)` — the day's nanoseconds.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_parse_value(text: *const c_void) -> i64 {
    parse_time_text(unsafe { unit_str(text) }).unwrap_or(0)
}

/// `DateTime.parse(text)` — `ok` flag.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_ok(text: *const c_void) -> bool {
    parse_datetime_text(unsafe { unit_str(text) }).is_some()
}

/// `DateTime.parse(text)` — the civil timestamp.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_value(text: *const c_void) -> i128 {
    parse_datetime_text(unsafe { unit_str(text) }).unwrap_or(0)
}

// --- Pattern formatting ----------------------------------------------------

/// Renders the civil components under `pattern`. Recognized tokens are
/// `YYYY`, `MM`, `DD`, `HH`, `mm`, `ss` and `SSS` (milliseconds); any
/// other text passes through literally — `format` never fails.
// The seven civil components arrive as separate locals from the decoded packed
// representation at every call site; boxing them into a struct would only move
// the unpacking around.
#[allow(clippy::too_many_arguments)]
fn format_pattern(
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
///
/// # Safety
///
/// `pattern` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_format(days: i64, pattern: *const c_void) -> *mut c_void {
    let (year, month, day) = civil_from_days(days);
    alloc_owned(&format_pattern(
        unsafe { unit_str(pattern) },
        year,
        month,
        day,
        0,
        0,
        0,
        0,
    ))
}

/// `t.format(pattern)` — the pattern applied to the time fields; date
/// tokens render `00`/`0000`.
///
/// # Safety
///
/// `pattern` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_format(nanos: i64, pattern: *const c_void) -> *mut c_void {
    alloc_owned(&format_pattern(
        unsafe { unit_str(pattern) },
        0,
        0,
        0,
        zirk_rt_time_hour(nanos),
        zirk_rt_time_minute(nanos),
        zirk_rt_time_second(nanos),
        zirk_rt_time_nanosecond(nanos),
    ))
}

/// `dt.format(pattern)` — the pattern applied to all fields.
///
/// # Safety
///
/// `pattern` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format(
    value: i128,
    pattern: *const c_void,
) -> *mut c_void {
    let (year, month, day) = civil_from_days(zirk_rt_datetime_days(value));
    let nanos = zirk_rt_datetime_nanos(value);
    alloc_owned(&format_pattern(
        unsafe { unit_str(pattern) },
        year,
        month,
        day,
        zirk_rt_time_hour(nanos),
        zirk_rt_time_minute(nanos),
        zirk_rt_time_second(nanos),
        zirk_rt_time_nanosecond(nanos),
    ))
}
