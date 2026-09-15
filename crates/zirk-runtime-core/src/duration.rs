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

pub fn multiply(nanos: i64, scalar: f64) -> i64 {
    (nanos as f64 * scalar) as i64
}

pub fn divide(nanos: i64, scalar: f64) -> i64 {
    (nanos as f64 / scalar) as i64
}

pub fn ratio(left: i64, right: i64) -> f64 {
    left as f64 / right as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_exact_duration_literals() {
        assert_eq!(format(1_500_000_000), "1.5s");
        assert_eq!(format(3_660_000_000_000), "61m");
        assert_eq!(format(-1_500), "-1.5us");
        assert_eq!(format(i64::MIN), "-9223372036.854775808s");
    }
}
