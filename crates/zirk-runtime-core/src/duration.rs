//! Platform-neutral operations for Zirk `Duration` values.
//!
//! A duration is an exact signed count of nanoseconds.  This module contains
//! only value operations; host handles and ABI allocation stay in the native
//! runtime.

pub const NANOS_PER_DAY: i128 = 86_400_000_000_000;
const UNITS: &[(&str, i128)] = &[
    ("w", 604_800_000_000_000),
    ("d", NANOS_PER_DAY),
    ("h", 3_600_000_000_000),
    ("m", 60_000_000_000),
    ("s", 1_000_000_000),
    ("ms", 1_000_000),
    ("us", 1_000),
    ("ns", 1),
];

pub fn format(nanos: i64) -> String {
    if nanos == 0 {
        return "0s".to_owned();
    }
    let magnitude = (nanos as i128).abs();
    for &(suffix, unit) in UNITS {
        if magnitude < unit {
            continue;
        }
        let whole = magnitude / unit;
        let rest = magnitude % unit;
        let sign = if nanos < 0 { "-" } else { "" };
        if rest == 0 {
            return format!("{sign}{whole}{suffix}");
        }
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
        return format!(
            "{sign}{whole}.{numerator:0digits$}{suffix}",
            numerator = scaled / unit
        );
    }
    unreachable!("nanoseconds always render")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarError {
    DivisionByZero,
    NonFiniteResult,
    Overflow,
}

/// Converts the computed scalar result using the existing truncation-toward-
/// zero rule, while rejecting NaN, infinity, and values outside the i64 range.
fn scalar_result(value: f64) -> Result<i64, ScalarError> {
    if value.is_nan() {
        return Err(ScalarError::NonFiniteResult);
    }
    if !value.is_finite() {
        return Err(ScalarError::Overflow);
    }
    // `i64::MAX as f64` rounds up to 2^63, so the upper bound is exclusive.
    let upper_exclusive = 9_223_372_036_854_775_808.0_f64;
    let lower_inclusive = -upper_exclusive;
    if value < lower_inclusive || value >= upper_exclusive {
        return Err(ScalarError::Overflow);
    }
    Ok(value.trunc() as i64)
}

pub fn multiply(nanos: i64, scalar: f64) -> Result<i64, ScalarError> {
    scalar_result(nanos as f64 * scalar)
}

pub fn divide(nanos: i64, scalar: f64) -> Result<i64, ScalarError> {
    if scalar == 0.0 {
        return Err(ScalarError::DivisionByZero);
    }
    scalar_result(nanos as f64 / scalar)
}

pub fn ratio(left: i64, right: i64) -> f64 {
    left as f64 / right as f64
}

/// `d.round(unit)` — nearest multiple of `unit`, halves away from zero.
/// A zero unit returns `nanos` unchanged.
pub fn round(nanos: i64, unit: i64) -> i64 {
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
    (q * u) as i64
}

/// `d.floor(unit)` — largest multiple of `unit` not greater than the value.
pub fn floor(nanos: i64, unit: i64) -> i64 {
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

/// `d.ceil(unit)` — smallest multiple of `unit` not less than the value.
pub fn ceil(nanos: i64, unit: i64) -> i64 {
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

/// `d.truncate(unit)` — multiple of `unit` toward zero.
pub fn truncate(nanos: i64, unit: i64) -> i64 {
    if unit == 0 {
        return nanos;
    }
    let n = nanos as i128;
    let u = unit as i128;
    ((n / u) * u) as i64
}

const NANOS_PER_HOUR: i128 = 3_600_000_000_000;
const NANOS_PER_MINUTE: i128 = 60_000_000_000;
const NANOS_PER_SECOND: i128 = 1_000_000_000;
const NANOS_PER_MILLI: i128 = 1_000_000;
const NANOS_PER_MICRO: i128 = 1_000;

/// Renders a nanosecond count as an ISO 8601 duration, e.g. `PT1H30M5.123S`.
pub fn format_iso(nanos: i64) -> String {
    if nanos == 0 {
        return "PT0S".to_string();
    }
    let sign = if nanos < 0 { "-" } else { "" };
    let m = (nanos as i128).wrapping_abs();
    let days = m / NANOS_PER_DAY;
    let m = m % NANOS_PER_DAY;
    let hours = m / NANOS_PER_HOUR;
    let m = m % NANOS_PER_HOUR;
    let minutes = m / NANOS_PER_MINUTE;
    let m = m % NANOS_PER_MINUTE;
    let seconds = m / NANOS_PER_SECOND;
    let subsecond = (m % NANOS_PER_SECOND) as i64;
    let mut text = if days > 0 {
        format!("{sign}P{days}DT{hours}H{minutes}M{seconds}")
    } else {
        format!("{sign}PT{hours}H{minutes}M{seconds}")
    };
    if subsecond == 0 {
        text.push('S');
    } else {
        let frac = format!("{subsecond:09}");
        let frac = frac.trim_end_matches('0');
        text.push('.');
        text.push_str(frac);
        text.push('S');
    }
    text
}

/// `d.format(template)` — typed `%` placeholders for the duration's
/// components. Supported: `%w` weeks, `%d` days, `%h` hours, `%m` minutes,
/// `%s` seconds, `%ms` milliseconds, `%us` microseconds, `%ns` nanoseconds,
/// `%%` a literal `%`. All component values are from the magnitude.
pub fn format_template(nanos: i64, template: &str) -> String {
    let sign = if nanos < 0 { "-" } else { "" };
    let m = (nanos as i128).wrapping_abs();
    let days_total = m / NANOS_PER_DAY;
    let m = m % NANOS_PER_DAY;
    let hours = m / NANOS_PER_HOUR;
    let m = m % NANOS_PER_HOUR;
    let minutes = m / NANOS_PER_MINUTE;
    let m = m % NANOS_PER_MINUTE;
    let seconds = m / NANOS_PER_SECOND;
    let m = m % NANOS_PER_SECOND;
    let ms = m / NANOS_PER_MILLI;
    let m = m % NANOS_PER_MILLI;
    let us = m / NANOS_PER_MICRO;
    let ns = m % NANOS_PER_MICRO;

    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('%') => {
                out.push('%');
                chars.next();
            }
            Some('w') => {
                out.push_str(&(days_total / 7).to_string());
                chars.next();
            }
            Some('d') => {
                out.push_str(&days_total.to_string());
                chars.next();
            }
            Some('h') => {
                out.push_str(&hours.to_string());
                chars.next();
            }
            Some('m') => {
                chars.next();
                if matches!(chars.peek(), Some('s')) {
                    out.push_str(&ms.to_string());
                    chars.next();
                } else {
                    out.push_str(&minutes.to_string());
                }
            }
            Some('s') => {
                out.push_str(&seconds.to_string());
                chars.next();
            }
            Some('u') => {
                chars.next();
                if matches!(chars.peek(), Some('s')) {
                    out.push_str(&us.to_string());
                    chars.next();
                } else {
                    out.push('?');
                }
            }
            Some('n') => {
                chars.next();
                if matches!(chars.peek(), Some('s')) {
                    out.push_str(&ns.to_string());
                    chars.next();
                } else {
                    out.push('?');
                }
            }
            _ => out.push('?'),
        }
    }
    format!("{sign}{out}")
}

/// `d.humanize(locale, max_units)` — readable, multi-unit description.
/// The `locale` is currently ignored and English-style labels are used.
pub fn humanize(nanos: i64, _locale: &str, max_units: i32) -> String {
    if nanos == 0 {
        return "0s".to_string();
    }
    let sign = if nanos < 0 { "-" } else { "" };
    let mut m = (nanos as i128).wrapping_abs();
    let mut parts = Vec::new();
    for &(name, unit) in UNITS {
        if m == 0 {
            break;
        }
        let whole = m / unit;
        if whole == 0 {
            continue;
        }
        m %= unit;
        parts.push(format!("{whole}{name}"));
        if max_units > 0 && parts.len() == max_units as usize {
            break;
        }
    }
    if parts.is_empty() {
        "0s".to_string()
    } else {
        format!("{sign}{}", parts.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_scalar_conversions_are_truncated_and_range_checked() {
        assert_eq!(multiply(5, 0.5), Ok(2));
        assert_eq!(multiply(-5, 0.5), Ok(-2));
        assert_eq!(multiply(i64::MIN, 1.0), Ok(i64::MIN));
        assert_eq!(multiply(i64::MAX, 2.0), Err(ScalarError::Overflow));
        assert_eq!(
            multiply(0, f64::INFINITY),
            Err(ScalarError::NonFiniteResult)
        );
        assert_eq!(divide(1, 0.0), Err(ScalarError::DivisionByZero));
        assert_eq!(divide(i64::MIN, -1.0), Err(ScalarError::Overflow));
        assert_eq!(divide(7, 2.0), Ok(3));
    }

    #[test]
    fn formats_exact_duration_literals() {
        assert_eq!(format(1_500_000_000), "1.5s");
        assert_eq!(format(3_660_000_000_000), "61m");
        assert_eq!(format(-1_500), "-1.5us");
        assert_eq!(format(i64::MIN), "-9223372036.854775808s");
    }
}
