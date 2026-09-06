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
