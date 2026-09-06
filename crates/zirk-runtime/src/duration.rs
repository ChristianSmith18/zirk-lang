//! Runtime helpers for `Duration`.
//!
//! A `Duration` is represented as an `i64` count of nanoseconds. Operations
//! an `i64` instruction cannot express — arithmetic against a `Float` scalar,
//! the `Duration / Duration` ratio, and `to_string()` — reach this file; the
//! rest (`+`, `-`, comparisons, integer scalars) lower to the same checked
//! `i64` IR instructions `Int64` uses.

use std::ffi::c_void;

use crate::string::alloc_owned;

/// Multiplies a `Duration` by an `f64` scalar.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_duration_mul_f64(nanos: i64, scalar: f64) -> i64 {
    (nanos as f64 * scalar) as i64
}

/// Divides a `Duration` by an `f64` scalar.
///
/// A zero divisor never reaches here: `zirk-ir` throws `DivisionByZeroError`
/// ahead of the call, the same way it guards `Duration / Int` and
/// `Duration / Duration`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_duration_div_f64(nanos: i64, scalar: f64) -> i64 {
    (nanos as f64 / scalar) as i64
}

/// Divides one `Duration` by another, yielding a `Float64` ratio.
///
/// The zero-divisor guard lives in `zirk-ir` for the same reason
/// [`zirk_rt_duration_div_f64`] documents.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_duration_div_duration(left: i64, right: i64) -> f64 {
    left as f64 / right as f64
}

/// Formats a `Duration` as a `String` — `to_string()`, `println` and string
/// interpolation all reach this.
///
/// The rendering picks the largest unit the value reaches and writes the
/// remainder as a decimal fraction, so `1500ms` reads `1.5s` while `250ms`
/// reads `250ms`, and any nanosecond count is reproduced exactly.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_duration_to_string(nanos: i64) -> *mut c_void {
    alloc_owned(&format_duration(nanos))
}

/// `d.abs()` (`native-type-member-surface`) — the magnitude. The `i64::MIN`
/// case never reaches here: lowering throws `ArithmeticOverflowError` first.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_abs(nanos: i64) -> i64 {
    nanos.wrapping_abs()
}

/// `d.sign()` — `-1`, `0`, or `1`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_sign(nanos: i64) -> i32 {
    nanos.signum() as i32
}

const NANOS_PER_WEEK: i128 = 7 * NANOS_PER_DAY;
const NANOS_PER_DAY: i128 = 86_400_000_000_000;
const NANOS_PER_HOUR: i128 = 3_600_000_000_000;
const NANOS_PER_MINUTE: i128 = 60_000_000_000;
const NANOS_PER_SECOND: i128 = 1_000_000_000;
const NANOS_PER_MILLI: i128 = 1_000_000;
const NANOS_PER_MICRO: i128 = 1_000;

fn magnitude(nanos: i64) -> i128 {
    (nanos as i128).wrapping_abs()
}

/// `d.days` — the normalized day component.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_days(nanos: i64) -> i64 {
    (magnitude(nanos) / NANOS_PER_DAY) as i64
}

/// `d.hours` — the hour component (0–23).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_hours(nanos: i64) -> i32 {
    ((magnitude(nanos) % NANOS_PER_DAY) / NANOS_PER_HOUR) as i32
}

/// `d.minutes` — the minute component (0–59).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_minutes(nanos: i64) -> i32 {
    ((magnitude(nanos) % NANOS_PER_HOUR) / NANOS_PER_MINUTE) as i32
}

/// `d.seconds` — the second component (0–59).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_seconds(nanos: i64) -> i32 {
    ((magnitude(nanos) % NANOS_PER_MINUTE) / NANOS_PER_SECOND) as i32
}

/// `d.milliseconds` — the millisecond component (0–999).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_milliseconds(nanos: i64) -> i32 {
    ((magnitude(nanos) % NANOS_PER_SECOND) / NANOS_PER_MILLI) as i32
}

/// `d.microseconds` — the microsecond component (0–999).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_microseconds(nanos: i64) -> i32 {
    (((magnitude(nanos) % NANOS_PER_SECOND) / NANOS_PER_MICRO) % 1000) as i32
}

/// `d.nanoseconds` — the nanosecond component (0–999).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_nanoseconds(nanos: i64) -> i32 {
    ((magnitude(nanos) % NANOS_PER_SECOND) % 1000) as i32
}

/// `d.total_weeks()` — the value as a fractional number of weeks.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_weeks(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_WEEK as f64
}

/// `d.total_days()` — the value as a fractional number of days.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_days(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_DAY as f64
}

/// `d.total_hours()` — the value as a fractional number of hours.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_hours(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_HOUR as f64
}

/// `d.total_minutes()` — the value as a fractional number of minutes.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_minutes(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_MINUTE as f64
}

/// `d.total_seconds()` — the value as a fractional number of seconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_seconds(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_SECOND as f64
}

/// `d.total_milliseconds()` — the value as a fractional number of milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_milliseconds(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_MILLI as f64
}

/// `d.total_microseconds()` — the value as a fractional number of microseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_microseconds(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_MICRO as f64
}

/// `d.total_nanoseconds()` — the value as nanoseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_total_nanoseconds(nanos: i64) -> f64 {
    nanos as f64
}

/// `d.whole_weeks()` — the value truncated toward zero in whole weeks.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_weeks(nanos: i64) -> i64 {
    nanos / NANOS_PER_WEEK as i64
}

/// `d.whole_days()` — the value truncated toward zero in whole days.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_days(nanos: i64) -> i64 {
    nanos / NANOS_PER_DAY as i64
}

/// `d.whole_hours()` — the value truncated toward zero in whole hours.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_hours(nanos: i64) -> i64 {
    nanos / NANOS_PER_HOUR as i64
}

/// `d.whole_minutes()` — the value truncated toward zero in whole minutes.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_minutes(nanos: i64) -> i64 {
    nanos / NANOS_PER_MINUTE as i64
}

/// `d.whole_seconds()` — the value truncated toward zero in whole seconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_seconds(nanos: i64) -> i64 {
    nanos / NANOS_PER_SECOND as i64
}

/// `d.whole_milliseconds()` — the value truncated toward zero in whole milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_milliseconds(nanos: i64) -> i64 {
    nanos / NANOS_PER_MILLI as i64
}

/// `d.whole_microseconds()` — the value truncated toward zero in whole microseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_microseconds(nanos: i64) -> i64 {
    nanos / NANOS_PER_MICRO as i64
}

/// `d.whole_nanoseconds()` — the value truncated toward zero in whole nanoseconds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_whole_nanoseconds(nanos: i64) -> i64 {
    nanos
}

/// `d.round(unit)`, `d.floor(unit)`, `d.ceil(unit)` and `d.truncate(unit)`.
/// All take the current nanosecond count and the `unit` as nanoseconds.
fn divide_rounded(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    let n = nanos as i128;
    let u = unit as i128;
    let q = n / u;
    let r = n % u;
    let q = if 2 * r.abs() >= u.abs() {
        q + r.signum()
    } else {
        q
    };
    q as i64
}

fn divide_floored(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    let n = nanos as i128;
    let u = unit as i128;
    let mut q = n / u;
    let r = n % u;
    if r != 0 && (n < 0) != (u < 0) {
        q -= 1;
    }
    (q * u) as i64
}

fn divide_ceiled(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    let n = nanos as i128;
    let u = unit as i128;
    let mut q = n / u;
    let r = n % u;
    if r != 0 && (n < 0) == (u < 0) {
        q += 1;
    }
    (q * u) as i64
}

fn divide_truncated(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    let n = nanos as i128;
    let u = unit as i128;
    let q = n / u;
    (q * u) as i64
}

/// `d.round(unit)` — nearest multiple of `unit`, halves away from zero.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_round(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    (divide_rounded(nanos, unit) as i128 * unit as i128) as i64
}

/// `d.floor(unit)` — largest multiple of `unit` not greater than the value.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_floor(nanos: i64, unit: i64) -> i64 {
    divide_floored(nanos, unit)
}

/// `d.ceil(unit)` — smallest multiple of `unit` not less than the value.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_ceil(nanos: i64, unit: i64) -> i64 {
    divide_ceiled(nanos, unit)
}

/// `d.truncate(unit)` — multiple of `unit` toward zero.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_duration_truncate(nanos: i64, unit: i64) -> i64 {
    divide_truncated(nanos, unit)
}

/// Renders a nanosecond count as a normalized duration literal.
fn format_duration(nanos: i64) -> String {
    // Largest first: the first unit the magnitude reaches is the one the
    // text is written in. `m` is minutes — a month is a `Period`, not an
    // exact span.
    const UNITS: &[(&str, i128)] = &[
        ("w", 604_800_000_000_000),
        ("d", 86_400_000_000_000),
        ("h", 3_600_000_000_000),
        ("m", 60_000_000_000),
        ("s", 1_000_000_000),
        ("ms", 1_000_000),
        ("us", 1_000),
        ("ns", 1),
    ];

    if nanos == 0 {
        return "0s".to_string();
    }

    // `i128` so `i64::MIN`'s magnitude fits.
    let magnitude = (nanos as i128).abs();

    for &(suffix, unit) in UNITS {
        if magnitude < unit {
            continue;
        }
        let whole = magnitude / unit;
        let rest = magnitude % unit;

        let mut text = String::new();
        if nanos < 0 {
            text.push('-');
        }
        text.push_str(&whole.to_string());
        if rest == 0 {
            text.push_str(suffix);
            return text;
        }

        // The remainder only reads as a finite decimal fraction of the unit
        // when the unit's factor outside 2 and 5 already divides it —
        // otherwise the rendering could not be lossless (`90m` in `h` is
        // `1.5h`, but `61m` has no finite `h` fraction and falls through to
        // `61m`). `ns`, the last unit, always answers.
        let mut non_decimal_factor = unit;
        while non_decimal_factor % 2 == 0 {
            non_decimal_factor /= 2;
        }
        while non_decimal_factor % 5 == 0 {
            non_decimal_factor /= 5;
        }
        if rest % non_decimal_factor != 0 {
            continue;
        }

        let mut scaled = rest;
        let mut digits = 0;
        while scaled % unit != 0 {
            scaled *= 10;
            digits += 1;
        }
        let numerator = scaled / unit;
        text.push('.');
        text.push_str(&format!("{numerator:0digits$}"));
        text.push_str(suffix);
        return text;
    }

    unreachable!("the `ns` unit always renders a nanosecond count")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::string::borrow;

    fn read(handle: *const c_void) -> String {
        let string = unsafe { borrow(handle) }.expect("a valid string handle");
        unsafe { string.as_str() }.to_string()
    }

    #[test]
    fn to_string_normalizes_to_the_largest_reached_unit() {
        assert_eq!(read(zirk_rt_duration_to_string(1_500_000_000)), "1.5s");
        assert_eq!(read(zirk_rt_duration_to_string(250_000_000)), "250ms");
        assert_eq!(read(zirk_rt_duration_to_string(7_200_000_000_000)), "2h");
        assert_eq!(read(zirk_rt_duration_to_string(604_800_000_000_000)), "1w");
        assert_eq!(read(zirk_rt_duration_to_string(1_500)), "1.5us");
        assert_eq!(read(zirk_rt_duration_to_string(1)), "1ns");
        assert_eq!(read(zirk_rt_duration_to_string(0)), "0s");
    }

    #[test]
    fn to_string_is_lossless_for_exact_nanoseconds() {
        assert_eq!(
            read(zirk_rt_duration_to_string(1_500_000_001)),
            "1.500000001s"
        );
        assert_eq!(read(zirk_rt_duration_to_string(-1_500_000_000)), "-1.5s");
        // `90m` is exactly `1.5h`, but `61m` has no finite `h` fraction and
        // a nanosecond offset lands on the decimal `s` rendering.
        assert_eq!(read(zirk_rt_duration_to_string(5_400_000_000_000)), "1.5h");
        assert_eq!(read(zirk_rt_duration_to_string(3_660_000_000_000)), "61m");
        assert_eq!(
            read(zirk_rt_duration_to_string(3_600_000_000_001)),
            "3600.000000001s"
        );
    }
}
