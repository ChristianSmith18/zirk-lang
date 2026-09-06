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
