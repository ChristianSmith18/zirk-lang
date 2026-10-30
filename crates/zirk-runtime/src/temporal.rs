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
use zirk_runtime_core::locale as core_locale;
use zirk_runtime_core::temporal as core_temporal;
use zirk_runtime_core::zoned_temporal::{
    DstPolicy, GapPolicy, Instant as CoreInstant, OverlapPolicy, Period as CorePeriod,
    TemporalError as ZonedTemporalError, TimeShift as CoreTimeShift, TimeZone as CoreTimeZone,
    ZonedDateTime as CoreZonedDateTime,
};

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const NANOS_PER_MINUTE: i64 = 60 * NANOS_PER_SECOND;
const NANOS_PER_HOUR: i64 = 60 * NANOS_PER_MINUTE;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_period_new(
    out: *mut i128,
    years: i32,
    months: i32,
    weeks: i32,
    days: i32,
) {
    unsafe { out.write(CorePeriod::new(years, months, weeks, days).encode()) };
}

/// Component index: years=0, months=1, weeks=2, days=3.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_period_component(value: *const i128, index: i32) -> i32 {
    let period = CorePeriod::decode(unsafe { value.read_unaligned() });
    match index {
        0 => period.years(),
        1 => period.months(),
        2 => period.weeks(),
        3 => period.days(),
        _ => crate::failure::zirk_rt_overflow(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_period_parse_ok(text: *const c_void) -> bool {
    CorePeriod::parse_iso(unsafe { unit_str(text) }).is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_period_parse_value(out: *mut i128, text: *const c_void) {
    let packed = CorePeriod::parse_iso(unsafe { unit_str(text) })
        .map(CorePeriod::encode)
        .unwrap_or(0);
    unsafe { out.write(packed) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_period_to_string(value: *const i128) -> *mut c_void {
    alloc_owned(&CorePeriod::decode(unsafe { value.read_unaligned() }).to_iso_string())
}

unsafe fn timeshift_from_handle(value: *const c_void) -> Result<CoreTimeShift, ZonedTemporalError> {
    CoreTimeShift::from_runtime_token(unsafe { unit_str(value) })
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_timeshift_clock(
    time_nanoseconds: i64,
    duration_nanoseconds: i64,
    subtract: bool,
) -> *mut c_void {
    let delta = if subtract {
        -i128::from(duration_nanoseconds)
    } else {
        i128::from(duration_nanoseconds)
    };
    let value = CoreTimeShift::clock(time_nanoseconds, delta)
        .expect("an i64 duration always has an i32 day carry");
    alloc_owned(&value.to_runtime_token())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_calendar_ok(
    _years: i32,
    _months: i32,
    _weeks: i32,
    _days: i32,
    overlap: *const c_void,
    gap: *const c_void,
) -> bool {
    let overlap = unsafe { unit_str(overlap) };
    let gap = unsafe { unit_str(gap) };
    matches!(overlap, "reject" | "earlier" | "later")
        && matches!(gap, "reject" | "forward" | "backward")
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_calendar_value(
    years: i32,
    months: i32,
    weeks: i32,
    days: i32,
    duration_nanoseconds: i64,
    overlap: *const c_void,
    gap: *const c_void,
) -> *mut c_void {
    let overlap = match unsafe { unit_str(overlap) } {
        "reject" => OverlapPolicy::Reject,
        "earlier" => OverlapPolicy::EarlierInstant,
        "later" => OverlapPolicy::LaterInstant,
        _ => return alloc_owned(""),
    };
    let gap = match unsafe { unit_str(gap) } {
        "reject" => GapPolicy::Reject,
        "forward" => GapPolicy::ShiftForwardByGap,
        "backward" => GapPolicy::ShiftBackwardByGap,
        _ => return alloc_owned(""),
    };
    let value = CoreTimeShift::new(
        CorePeriod::new(years, months, weeks, days),
        i128::from(duration_nanoseconds),
        DstPolicy { overlap, gap },
    );
    alloc_owned(&value.to_runtime_token())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_is_clock(value: *const c_void) -> bool {
    unsafe { timeshift_from_handle(value) }
        .map(|shift| shift.kind() == "clock")
        .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_to_string(value: *const c_void) -> *mut c_void {
    let rendered = unsafe { timeshift_from_handle(value) }
        .map(CoreTimeShift::to_display_string)
        .unwrap_or_else(|_| "TimeShift(<invalid>)".into());
    alloc_owned(&rendered)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_kind(value: *const c_void) -> *mut c_void {
    let kind = unsafe { timeshift_from_handle(value) }
        .map(CoreTimeShift::kind)
        .unwrap_or("invalid");
    alloc_owned(kind)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_duration(value: *const c_void) -> i64 {
    unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(|shift| i64::try_from(shift.duration_nanoseconds()).ok())
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_time(value: *const c_void) -> i64 {
    unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(CoreTimeShift::time_nanoseconds)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_day_offset(value: *const c_void) -> i32 {
    unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(CoreTimeShift::day_offset)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_period_component(
    value: *const c_void,
    index: i32,
) -> i32 {
    unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(CoreTimeShift::period)
        .map(|period| match index {
            0 => period.years(),
            1 => period.months(),
            2 => period.weeks(),
            3 => period.days(),
            _ => 0,
        })
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_overlap_policy(value: *const c_void) -> *mut c_void {
    let text = unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(CoreTimeShift::dst_policy)
        .map(|policy| match policy.overlap {
            OverlapPolicy::Reject => "reject",
            OverlapPolicy::EarlierInstant => "earlier",
            OverlapPolicy::LaterInstant => "later",
        })
        .unwrap_or("");
    alloc_owned(text)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_gap_policy(value: *const c_void) -> *mut c_void {
    let text = unsafe { timeshift_from_handle(value) }
        .ok()
        .and_then(CoreTimeShift::dst_policy)
        .map(|policy| match policy.gap {
            GapPolicy::Reject => "reject",
            GapPolicy::ShiftForwardByGap => "forward",
            GapPolicy::ShiftBackwardByGap => "backward",
        })
        .unwrap_or("");
    alloc_owned(text)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_apply_ok(
    shift: *const c_void,
    zoned: *const c_void,
) -> bool {
    let shift = unsafe { timeshift_from_handle(shift) };
    let zoned = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(zoned) });
    matches!((shift, zoned), (Ok(shift), Ok(zoned)) if shift.apply_to(zoned).is_ok())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timeshift_apply_value(
    shift: *const c_void,
    zoned: *const c_void,
) -> *mut c_void {
    let result = unsafe { timeshift_from_handle(shift) }
        .and_then(|shift| {
            CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(zoned) })
                .and_then(|zoned| shift.apply_to(zoned))
        })
        .map(|value| value.encode_runtime_value())
        .unwrap_or_default();
    alloc_owned(&result)
}

/// Whether `year` is a leap year in the proleptic Gregorian calendar.
fn is_leap_year(year: i32) -> bool {
    core_temporal::is_leap_year(year)
}

/// Days in `month` of `year` (`month` in `1..=12`).
fn days_in_month(year: i32, month: i32) -> i32 {
    core_temporal::days_in_month(year, month)
}

/// Days since 1970-01-01 for the civil `year`-`month`-`day` (Hinnant's
/// `days_from_civil`). Assumes the triple was already validated.
fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    core_temporal::days_from_civil(year, month, day)
}

/// The civil `year`-`month`-`day` of a day count since 1970-01-01
/// (Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i32, i32, i32) {
    core_temporal::civil_from_days_checked(days)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

#[inline]
fn ensure_public_date(days: i64) {
    if core_temporal::civil_from_days_checked(days).is_none() {
        crate::failure::zirk_rt_overflow();
    }
}

/// Renders a `Time` as `HH:MM:SS[.fffffffff]` — the fraction is trimmed of
/// trailing zeros and omitted entirely when it is zero.
fn format_time(nanos: i64) -> String {
    core_temporal::format_time(nanos)
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
    ensure_public_date(days);
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
    core_temporal::time_add_nanos(time, delta)
}

/// `Time - Time`: the `Duration` nanoseconds between them, measured
/// forward around the day (`Time(1, 0) - Time(23, 0)` is `2h`).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_time_diff(left: i64, right: i64) -> i64 {
    core_temporal::time_diff(left, right)
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

/// The `i128` civil timestamp `DateTime(date, time)`/`Date + Time` pack —
/// factored out so every Rust call site here (which never crosses the C
/// boundary) keeps calling it as an ordinary by-value function; only the
/// `extern "C"` entry point below takes the pointer-based ABI a 128-bit
/// value needs at the boundary (see that function's own doc for why).
fn datetime_pack(days: i64, nanos: i64) -> i128 {
    let _ = civil_from_days(days);
    core_temporal::datetime_pack(days, nanos)
}

/// The `Date` half of a `DateTime` (its day count) — the by-value Rust
/// helper every call site in this file uses directly.
fn datetime_days(value: i128) -> i64 {
    let days = core_temporal::datetime_days_checked(value)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow());
    let _ = civil_from_days(days);
    days
}

/// The `Time` half of a `DateTime` (its day's nanoseconds) — see
/// [`datetime_days`].
fn datetime_nanos(value: i128) -> i64 {
    let _ = datetime_days(value);
    core_temporal::datetime_nanos(value)
}

/// `DateTime(date, time)` / `Date + Time`: packs the day count and the
/// day's nanoseconds into the `i128` civil timestamp.
///
/// # Safety
///
/// `out` must point to a valid, writable `i128`-sized, 16-byte-aligned
/// location — this is what `zirk-codegen-llvm`'s `declare_extern_fn`/
/// `emit_decimal_call` always allocate for a by-pointer 128-bit result
/// (design note there: Rust's own `i128` C ABI is not guaranteed to agree
/// with LLVM's default lowering for a plain 128-bit value on every target,
/// so every such value crosses this boundary through an explicit pointer
/// on both sides instead of relying on either one's by-value convention).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_new(out: *mut i128, days: i64, nanos: i64) {
    unsafe { out.write(datetime_pack(days, nanos)) };
}

/// The `Date` half of a `DateTime` (its day count).
///
/// # Safety
///
/// `value` must point to a valid, readable `i128` — see
/// [`zirk_rt_datetime_new`]'s own safety note for why this crosses by
/// pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_days(value: *const i128) -> i64 {
    datetime_days(unsafe { value.read_unaligned() })
}

/// The `Time` half of a `DateTime` (its day's nanoseconds).
///
/// # Safety
///
/// See [`zirk_rt_datetime_days`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_nanos(value: *const i128) -> i64 {
    datetime_nanos(unsafe { value.read_unaligned() })
}

/// `DateTime.now_utc()` — the current UTC civil timestamp.
///
/// # Safety
///
/// See [`zirk_rt_datetime_new`]'s own safety note.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_now_utc(out: *mut i128) {
    let (days, nanos) = utc_civil_now();
    unsafe { out.write(datetime_pack(days, nanos)) };
}

/// `DateTime.now_local()` — the host's current civil timestamp.
///
/// # Safety
///
/// See [`zirk_rt_datetime_new`]'s own safety note.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_now_local(out: *mut i128) {
    let (days, nanos) = local_civil_now();
    unsafe { out.write(datetime_pack(days, nanos)) };
}

/// ISO 8601 `YYYY-MM-DDTHH:MM:SS[.fffffffff]`.
///
/// # Safety
///
/// `value` must point to a valid, readable `i128` — see
/// [`zirk_rt_datetime_new`]'s own safety note.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_to_string(value: *const i128) -> *mut c_void {
    let value = unsafe { value.read_unaligned() };
    let (year, month, day) = civil_from_days(datetime_days(value));
    let time = format_time(datetime_nanos(value));
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
    ensure_public_date(days);
    core_temporal::date_day_of_week(days)
}

/// The 1-based ordinal day within `days`' year.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_day_of_year(days: i64) -> i32 {
    core_temporal::date_day_of_year_checked(days)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

/// The ISO 8601 week number: week 1 is the week containing the year's
/// first Thursday.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_week_of_year(days: i64) -> i32 {
    core_temporal::date_week_of_year_checked(days)
        .unwrap_or_else(|| crate::failure::zirk_rt_overflow())
}

/// The quarter of `days`' month, `1..=4`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_quarter(days: i64) -> i32 {
    ensure_public_date(days);
    (zirk_rt_date_month(days) - 1) / 3 + 1
}

/// The length of `days`' month.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_days_in_month(days: i64) -> i32 {
    ensure_public_date(days);
    let (year, month, _) = civil_from_days(days);
    days_in_month(year, month)
}

/// The length of `days`' year — `366` when leap, `365` otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_days_in_year(days: i64) -> i32 {
    ensure_public_date(days);
    if is_leap_year(civil_from_days(days).0) {
        366
    } else {
        365
    }
}

/// Whether `days`' year is leap.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_date_is_leap(days: i64) -> bool {
    ensure_public_date(days);
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
    core_temporal::date_unit_ok(unit)
}

/// The unit names `Time` accepts.
fn time_unit_ok(unit: &str) -> bool {
    core_temporal::time_unit_ok(unit)
}

/// The unit names `DateTime` accepts — the union of both.
fn datetime_unit_ok(unit: &str) -> bool {
    core_temporal::datetime_unit_ok(unit)
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
    let _ = civil_from_days(days);
    core_temporal::date_start_of(days, unit)
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
    let _ = civil_from_days(days);
    core_temporal::date_end_of(days, unit)
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
    core_temporal::time_start_of(nanos, unit)
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
    core_temporal::time_end_of(nanos, unit)
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
    let _ = datetime_days(value);
    core_temporal::datetime_start_of(value, unit)
}

/// # Safety
///
/// `out` must point to a valid, writable `i128` and `value` to a valid,
/// readable one — see [`zirk_rt_datetime_new`]'s own safety note.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_start_of_value(
    out: *mut i128,
    value: *const i128,
    unit: *const c_void,
) {
    let value = unsafe { value.read_unaligned() };
    let result = datetime_start_of(value, unsafe { unit_str(unit) });
    unsafe { out.write(result) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_end_of_ok(unit: *const c_void) -> bool {
    unsafe { zirk_rt_datetime_start_of_ok(unit) }
}

fn datetime_end_of(value: i128, unit: &str) -> i128 {
    let _ = datetime_days(value);
    core_temporal::datetime_end_of(value, unit)
}

/// # Safety
///
/// See [`zirk_rt_datetime_start_of_value`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_end_of_value(
    out: *mut i128,
    value: *const i128,
    unit: *const c_void,
) {
    let value = unsafe { value.read_unaligned() };
    let result = datetime_end_of(value, unsafe { unit_str(unit) });
    unsafe { out.write(result) };
}

// --- ISO 8601 parsing (`temporal-rich-api`) --------------------------------
//
// Strict canonical forms only — `YYYY-MM-DD`, `HH:MM[:SS[.frac]]`, and the
// date-time composition with `T` or a space. The `ok`/`value` split mirrors
// `zirk_int_parse_ok`/`zirk_int_parse_value`: the probe answers whether the
// text parses so `zirk-ir` can build the `Error` arm, and the value
// entrypoint re-parses on the passing branch.

/// `YYYY-MM-DD` — strict: exactly ten bytes, dashes at 4 and 7, and the
/// constructor's own range validation.
fn parse_date_text(text: &str) -> Option<i64> {
    core_temporal::parse_date_text(text)
}

/// `HH:MM[:SS[.fffffffff]]` — the fraction accepts one to nine digits.
fn parse_time_text(text: &str) -> Option<i64> {
    core_temporal::parse_time_text(text)
}

/// `YYYY-MM-DD[T ]HH:MM[:SS[.frac]]`.
fn parse_datetime_text(text: &str) -> Option<i128> {
    core_temporal::parse_datetime_text(text)
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
/// `out` must point to a valid, writable `i128` (see
/// [`zirk_rt_datetime_new`]'s own safety note) and `text` must be a
/// `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_value(out: *mut i128, text: *const c_void) {
    let result = parse_datetime_text(unsafe { unit_str(text) }).unwrap_or(0);
    unsafe { out.write(result) };
}

/// Localized parse probes share the pinned CLDR parser and return one of the
/// existing civil scalar layouts. Error kinds 10 and 11 identify invalid and
/// ambiguous text; all locale/data errors use the base LocaleError.
fn localized_parse_error_kind(error: core_locale::LocaleError) -> i32 {
    match error {
        core_locale::LocaleError::InvalidInput => 10,
        core_locale::LocaleError::AmbiguousInput => 11,
        _ => 1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_parse_locale_ok(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    core_locale::parse_date_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_parse_locale_value(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> i64 {
    core_locale::parse_date_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_parse_locale_error_kind(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> i32 {
    match core_locale::parse_date_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    ) {
        Ok(_) => 0,
        Err(error) => localized_parse_error_kind(error),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_parse_locale_ok(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    core_locale::parse_time_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_parse_locale_value(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> i64 {
    core_locale::parse_time_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_parse_locale_error_kind(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> i32 {
    match core_locale::parse_time_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    ) {
        Ok(_) => 0,
        Err(error) => localized_parse_error_kind(error),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_locale_ok(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    core_locale::parse_datetime_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_locale_value(
    out: *mut i128,
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) {
    let value = core_locale::parse_datetime_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
    .map(|(year, month, day, hour, minute, second, nanosecond)| {
        i128::from(days_from_civil(year, i32::from(month), i32::from(day)))
            * 86_400_000_000_000_i128
            + i128::from(hour) * i128::from(NANOS_PER_HOUR)
            + i128::from(minute) * i128::from(NANOS_PER_MINUTE)
            + i128::from(second) * i128::from(NANOS_PER_SECOND)
            + i128::from(nanosecond)
    })
    .unwrap_or_default();
    unsafe { out.write(value) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_parse_locale_error_kind(
    text: *const c_void,
    locale: *const c_void,
    style: *const c_void,
) -> i32 {
    match core_locale::parse_datetime_named(
        unsafe { unit_str(text) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    ) {
        Ok(_) => 0,
        Err(error) => localized_parse_error_kind(error),
    }
}

/// `Instant.parse(text)` probes an RFC 3339 absolute timestamp.
///
/// # Safety
/// `text` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_parse_ok(text: *const c_void) -> bool {
    CoreInstant::parse_rfc3339(unsafe { unit_str(text) }).is_ok()
}

/// Writes a successfully parsed RFC 3339 instant to `out`.
///
/// # Safety
/// `out` must point to a writable `i128`; `text` must be a runtime String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_parse_value(out: *mut i128, text: *const c_void) {
    let value = CoreInstant::parse_rfc3339(unsafe { unit_str(text) })
        .map(CoreInstant::unix_nanoseconds)
        .unwrap_or(0);
    unsafe { out.write(value) };
}

/// `TimeZone.parse(text)` accepts a pinned IANA name, `UTC`, or an ISO fixed
/// offset and returns its canonical identifier.
///
/// # Safety
/// `text` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_parse_ok(text: *const c_void) -> bool {
    CoreTimeZone::parse_identifier(unsafe { unit_str(text) }).is_ok()
}

/// Returns the stable core error code for a rejected zone identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_parse_error_kind(text: *const c_void) -> i32 {
    CoreTimeZone::parse_identifier(unsafe { unit_str(text) })
        .err()
        .map_or(0, |error| error.stable_code())
}

/// Returns the canonical IANA identifier after the matching `_ok` probe.
///
/// # Safety
/// `text` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_parse_value(text: *const c_void) -> *mut c_void {
    let identifier = unsafe { unit_str(text) };
    let canonical = CoreTimeZone::parse_identifier(identifier)
        .map(CoreTimeZone::canonical_identifier)
        .unwrap_or_default();
    alloc_owned(&canonical)
}

/// Returns the IANA rule-data version compiled into the runtime.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_timezone_database_version() -> *mut c_void {
    alloc_owned(CoreTimeZone::database_version())
}

/// `ZonedDateTime.parse` probes RFC 3339 text with a bracketed zone name.
///
/// # Safety
/// `text` must be a runtime String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_parse_ok(text: *const c_void) -> bool {
    CoreZonedDateTime::parse_rfc3339(unsafe { unit_str(text) }).is_ok()
}

/// Returns the canonical immutable zoned-value representation after the
/// matching `_ok` probe.
///
/// # Safety
/// `text` must be a runtime String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_parse_value(text: *const c_void) -> *mut c_void {
    let value = CoreZonedDateTime::parse_rfc3339(unsafe { unit_str(text) })
        .map(CoreZonedDateTime::encode_runtime_value)
        .unwrap_or_default();
    alloc_owned(&value)
}

/// Reads a canonical zone identifier from its immutable String handle.
unsafe fn timezone_from_handle(zone: *const c_void) -> CoreTimeZone {
    CoreTimeZone::parse_identifier(unsafe { unit_str(zone) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Reads a checked absolute instant from its FFI pointer.
unsafe fn instant_from_pointer(value: *const i128) -> CoreInstant {
    CoreInstant::from_unix_nanoseconds(unsafe { value.read_unaligned() })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Returns the zone's UTC offset at an instant, in seconds east of UTC.
///
/// # Safety
/// `zone` must be a runtime String handle and `instant` a valid pointer to an
/// `i128` instant value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_offset_seconds(
    zone: *const c_void,
    instant: *const i128,
) -> i32 {
    unsafe { timezone_from_handle(zone) }
        .offset_seconds_at(unsafe { instant_from_pointer(instant) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Whether the zone observes daylight-saving adjustment at an instant.
///
/// # Safety
/// `zone` must be a runtime String handle and `instant` a valid pointer to an
/// `i128` instant value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_is_dst(
    zone: *const c_void,
    instant: *const i128,
) -> bool {
    unsafe { timezone_from_handle(zone) }
        .is_dst_at(unsafe { instant_from_pointer(instant) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Returns the zone abbreviation in effect at an instant.
///
/// # Safety
/// `zone` must be a runtime String handle and `instant` a valid pointer to an
/// `i128` instant value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_abbreviation(
    zone: *const c_void,
    instant: *const i128,
) -> *mut c_void {
    let abbreviation = unsafe { timezone_from_handle(zone) }
        .abbreviation_at(unsafe { instant_from_pointer(instant) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    alloc_owned(&abbreviation)
}

/// Projects an instant into a zone and packs its local civil components as a
/// `DateTime` value.
///
/// # Safety
/// `out` must point to writable `i128` storage; `zone` must be a runtime
/// String handle and `instant` a valid pointer to an `i128` instant value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timezone_local_datetime(
    out: *mut i128,
    zone: *const c_void,
    instant: *const i128,
) {
    let zone = unsafe { timezone_from_handle(zone) };
    let instant = unsafe { instant_from_pointer(instant) };
    let packed = zirk_runtime_core::zoned_temporal::ZonedDateTime::from_instant(instant, zone)
        .and_then(|value| value.local_datetime_unix_nanoseconds())
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    unsafe { out.write(packed) };
}

/// Creates a canonical zoned value from a checked instant and zone.
///
/// # Safety
/// `instant` must point to a valid `i128`; `zone` must be a String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_from_instant(
    instant: *const i128,
    zone: *const c_void,
) -> *mut c_void {
    let instant = unsafe { instant_from_pointer(instant) };
    let zone = unsafe { timezone_from_handle(zone) };
    let value = CoreZonedDateTime::from_instant(instant, zone)
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    alloc_owned(&value.encode_runtime_value())
}

/// Reprojects a zoned value into another zone while preserving its instant.
///
/// # Safety
/// Both arguments must be String handles produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_with_zone(
    value: *const c_void,
    zone: *const c_void,
) -> *mut c_void {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    let zone = unsafe { timezone_from_handle(zone) };
    let projected = value
        .with_zone(zone)
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    alloc_owned(&projected.encode_runtime_value())
}

/// Writes the instant component of a zoned value through the i128 output ABI.
///
/// # Safety
/// `out` must be writable `i128` storage; `value` must be a String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_instant(out: *mut i128, value: *const c_void) {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    unsafe { out.write(value.instant().unix_nanoseconds()) };
}

/// Returns the canonical zone identifier of a zoned value.
///
/// # Safety
/// `value` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_zone(value: *const c_void) -> *mut c_void {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    alloc_owned(&value.zone().canonical_identifier())
}

/// Returns the packed local civil DateTime of a zoned value.
///
/// # Safety
/// `out` must be writable `i128` storage; `value` must be a String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_local_datetime(
    out: *mut i128,
    value: *const c_void,
) {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    let local = value
        .local_datetime_unix_nanoseconds()
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    unsafe { out.write(local) };
}

/// Returns the zone offset at the value's instant.
///
/// # Safety
/// `value` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_offset_seconds(value: *const c_void) -> i32 {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    value
        .zone()
        .offset_seconds_at(value.instant())
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Returns whether daylight saving is active at the value's instant.
///
/// # Safety
/// `value` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_is_dst(value: *const c_void) -> bool {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    value
        .zone()
        .is_dst_at(value.instant())
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Returns the zone abbreviation at the value's instant.
///
/// # Safety
/// `value` must be a String handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_zoned_datetime_abbreviation(value: *const c_void) -> *mut c_void {
    let value = CoreZonedDateTime::parse_runtime_value(unsafe { unit_str(value) })
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    let abbreviation = value
        .zone()
        .abbreviation_at(value.instant())
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    alloc_owned(&abbreviation)
}

/// Checks whether the Unix-second count is representable by the supported
/// instant timeline.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_instant_from_unix_seconds_ok(value: i64) -> bool {
    CoreInstant::from_unix_seconds(value).is_ok()
}

/// Writes a validated Unix-second count as an instant's epoch nanoseconds.
/// The caller must first observe a true result from the matching `_ok` probe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_from_unix_seconds_value(out: *mut i128, value: i64) {
    let nanos = CoreInstant::from_unix_seconds(value)
        .map(CoreInstant::unix_nanoseconds)
        .unwrap_or(0);
    unsafe { out.write(nanos) };
}

/// Checks whether the Unix-millisecond count is representable by the
/// supported instant timeline.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_instant_from_unix_milliseconds_ok(value: i64) -> bool {
    CoreInstant::from_unix_milliseconds(value).is_ok()
}

/// Writes a validated Unix-millisecond count as epoch nanoseconds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_from_unix_milliseconds_value(out: *mut i128, value: i64) {
    let nanos = CoreInstant::from_unix_milliseconds(value)
        .map(CoreInstant::unix_nanoseconds)
        .unwrap_or(0);
    unsafe { out.write(nanos) };
}

/// Checks whether the epoch-nanosecond count is representable by the
/// supported instant timeline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_from_unix_nanoseconds_ok(value: *const i128) -> bool {
    CoreInstant::from_unix_nanoseconds(unsafe { value.read_unaligned() }).is_ok()
}

/// Writes a validated epoch-nanosecond count unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_from_unix_nanoseconds_value(
    out: *mut i128,
    value: *const i128,
) {
    let nanos = CoreInstant::from_unix_nanoseconds(unsafe { value.read_unaligned() })
        .map(CoreInstant::unix_nanoseconds)
        .unwrap_or(0);
    unsafe { out.write(nanos) };
}

/// `Instant.now()` — the current system-clock instant on the Unix timeline.
///
/// # Safety
/// `out` must be a valid, writable pointer to an `i128`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_now(out: *mut i128) {
    let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => i128::try_from(elapsed.as_nanos())
            .unwrap_or_else(|_| crate::failure::zirk_rt_overflow()),
        Err(before_epoch) => i128::try_from(before_epoch.duration().as_nanos())
            .ok()
            .and_then(i128::checked_neg)
            .unwrap_or_else(|| crate::failure::zirk_rt_overflow()),
    };
    let nanos = CoreInstant::from_unix_nanoseconds(nanos)
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
        .unix_nanoseconds();
    unsafe { out.write(nanos) };
}

/// Whole signed Unix seconds, rounded down for pre-epoch fractional values.
///
/// # Safety
/// `value` must point to a valid `i128` instant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_unix_seconds(value: *const i128) -> i64 {
    let nanos = unsafe { value.read_unaligned() };
    let instant = CoreInstant::from_unix_nanoseconds(nanos)
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    i64::try_from(instant.unix_nanoseconds().div_euclid(1_000_000_000))
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Whole signed Unix milliseconds, rounded down for pre-epoch fractions.
///
/// # Safety
/// `value` must point to a valid `i128` instant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_unix_milliseconds(value: *const i128) -> i64 {
    let nanos = unsafe { value.read_unaligned() };
    let instant = CoreInstant::from_unix_nanoseconds(nanos)
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow());
    i64::try_from(instant.unix_nanoseconds().div_euclid(1_000_000))
        .unwrap_or_else(|_| crate::failure::zirk_rt_overflow())
}

/// Renders an absolute instant as canonical UTC RFC 3339 text.
///
/// # Safety
/// `value` must point to a valid `i128` instant and remains subject to the
/// pinned temporal runtime's supported range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_instant_to_string(value: *const i128) -> *mut c_void {
    let value = unsafe { value.read_unaligned() };
    let text = CoreInstant::from_unix_nanoseconds(value)
        .and_then(CoreInstant::to_rfc3339)
        .unwrap_or_else(|error| match error {
            ZonedTemporalError::OutOfRange => crate::failure::zirk_rt_overflow(),
            _ => unreachable!("validated instant formatting has no other error"),
        });
    alloc_owned(&text)
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
    ensure_public_date(days);
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

unsafe fn localized_date(
    days: i64,
    locale: *const c_void,
    style: *const c_void,
) -> Result<String, core_locale::LocaleError> {
    let (year, month, day) = civil_from_days(days);
    core_locale::format_date_named(
        year,
        month as u8,
        day as u8,
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_format_locale_ok(
    days: i64,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    unsafe { localized_date(days, locale, style) }.is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_date_format_locale_value(
    days: i64,
    locale: *const c_void,
    style: *const c_void,
) -> *mut c_void {
    alloc_owned(&unsafe { localized_date(days, locale, style) }.unwrap_or_default())
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

unsafe fn localized_time(
    nanos: i64,
    locale: *const c_void,
    style: *const c_void,
) -> Result<String, core_locale::LocaleError> {
    core_locale::format_time_named(
        zirk_rt_time_hour(nanos) as u8,
        zirk_rt_time_minute(nanos) as u8,
        zirk_rt_time_second(nanos) as u8,
        zirk_rt_time_nanosecond(nanos) as u32,
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_format_locale_ok(
    nanos: i64,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    unsafe { localized_time(nanos, locale, style) }.is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_time_format_locale_value(
    nanos: i64,
    locale: *const c_void,
    style: *const c_void,
) -> *mut c_void {
    alloc_owned(&unsafe { localized_time(nanos, locale, style) }.unwrap_or_default())
}

/// `dt.format(pattern)` — the pattern applied to all fields.
///
/// # Safety
///
/// `value` must point to a valid, readable `i128` (see
/// [`zirk_rt_datetime_new`]'s own safety note) and `pattern` must be a
/// `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format(
    value: *const i128,
    pattern: *const c_void,
) -> *mut c_void {
    let value = unsafe { value.read_unaligned() };
    let (year, month, day) = civil_from_days(datetime_days(value));
    let nanos = datetime_nanos(value);
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

unsafe fn localized_datetime(
    value: *const i128,
    locale: *const c_void,
    style: *const c_void,
) -> Result<String, core_locale::LocaleError> {
    let value = unsafe { value.read_unaligned() };
    let (year, month, day) = civil_from_days(datetime_days(value));
    let nanos = datetime_nanos(value);
    core_locale::format_datetime_named(
        year,
        month as u8,
        day as u8,
        zirk_rt_time_hour(nanos) as u8,
        zirk_rt_time_minute(nanos) as u8,
        zirk_rt_time_second(nanos) as u8,
        zirk_rt_time_nanosecond(nanos) as u32,
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
    )
}

unsafe fn localized_datetime_in_zone(
    value: *const i128,
    zone: *const c_void,
    locale: *const c_void,
    style: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> Result<String, core_locale::LocaleError> {
    let value = unsafe { value.read_unaligned() };
    let (year, month, day) = civil_from_days(datetime_days(value));
    let nanos = datetime_nanos(value);
    core_locale::format_local_datetime_in_zone(
        year,
        month as u8,
        day as u8,
        zirk_rt_time_hour(nanos) as u8,
        zirk_rt_time_minute(nanos) as u8,
        zirk_rt_time_second(nanos) as u8,
        zirk_rt_time_nanosecond(nanos) as u32,
        unsafe { unit_str(zone) },
        unsafe { unit_str(locale) },
        unsafe { unit_str(style) },
        unsafe { unit_str(overlap) },
        unsafe { unit_str(gap) },
    )
}

unsafe fn zoned_datetime_from_local(
    value: *const i128,
    zone: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> Result<CoreZonedDateTime, ZonedTemporalError> {
    let overlap = match unsafe { unit_str(overlap) } {
        "reject" => OverlapPolicy::Reject,
        "earlier" => OverlapPolicy::EarlierInstant,
        "later" => OverlapPolicy::LaterInstant,
        _ => return Err(ZonedTemporalError::InvalidDstPolicy),
    };
    let gap = match unsafe { unit_str(gap) } {
        "reject" => GapPolicy::Reject,
        "forward" => GapPolicy::ShiftForwardByGap,
        "backward" => GapPolicy::ShiftBackwardByGap,
        _ => return Err(ZonedTemporalError::InvalidDstPolicy),
    };
    let packed = unsafe { value.read_unaligned() };
    let zone = CoreTimeZone::parse_identifier(unsafe { unit_str(zone) })?;
    CoreZonedDateTime::from_packed_local_datetime(packed, zone, DstPolicy { overlap, gap })
}

/// Checks conversion of a local DateTime using explicit DST resolution.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_in_zone_ok(
    value: *const i128,
    zone: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> bool {
    unsafe { zoned_datetime_from_local(value, zone, overlap, gap) }.is_ok()
}

/// Returns the stable core temporal error code for a failed local-time
/// resolution. Zero means the inputs are valid and resolution succeeds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_in_zone_error_kind(
    value: *const i128,
    zone: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> i32 {
    unsafe { zoned_datetime_from_local(value, zone, overlap, gap) }
        .err()
        .map_or(0, |error| error.stable_code())
}

/// Encodes a local DateTime resolved by the matching `_ok` probe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_in_zone_value(
    value: *const i128,
    zone: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> *mut c_void {
    let value = unsafe { zoned_datetime_from_local(value, zone, overlap, gap) }
        .map(|value| value.encode_runtime_value())
        .unwrap_or_default();
    alloc_owned(&value)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format_locale_in_zone_ok(
    value: *const i128,
    zone: *const c_void,
    locale: *const c_void,
    style: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> bool {
    unsafe { localized_datetime_in_zone(value, zone, locale, style, overlap, gap) }.is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format_locale_in_zone_value(
    value: *const i128,
    zone: *const c_void,
    locale: *const c_void,
    style: *const c_void,
    overlap: *const c_void,
    gap: *const c_void,
) -> *mut c_void {
    alloc_owned(
        &unsafe { localized_datetime_in_zone(value, zone, locale, style, overlap, gap) }
            .unwrap_or_default(),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format_locale_ok(
    value: *const i128,
    locale: *const c_void,
    style: *const c_void,
) -> bool {
    unsafe { localized_datetime(value, locale, style) }.is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_datetime_format_locale_value(
    value: *const i128,
    locale: *const c_void,
    style: *const c_void,
) -> *mut c_void {
    alloc_owned(&unsafe { localized_datetime(value, locale, style) }.unwrap_or_default())
}
