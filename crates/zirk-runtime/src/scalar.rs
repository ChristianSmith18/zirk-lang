//! Runtime helpers for the integer and float member surface
//! (`native-type-member-surface`).
//!
//! Integers travel as `i128` — wide enough for every `Int`/`UInt` width —
//! with `bits` and `signed` parameters carrying the width semantics the
//! receiver's static type implies. Narrow-width results are masked or
//! sign-wrapped here; lowering truncates the `i128` back to the receiver's
//! width, so the helper only has to produce the mathematically right bits
//! in the low `bits` positions.
//!
//! `checked_*` and `parse` are split into an `ok` status call and a
//! `value` call: the helpers are pure, so computing them twice is cheap
//! and keeps the `Result<T, E>` construction in lowering, where the
//! `Ok`/`Error` variants live.

use std::ffi::c_void;

use crate::string::{alloc_owned, borrow};

/// Masks `value` to `bits` and sign-extends when `signed` is set, so
/// arithmetic on narrow widths observes the same wrap points the type
/// system promises.
fn normalize(value: i128, bits: i32, signed: i32) -> i128 {
    let bits = bits.clamp(1, 128) as u32;
    if bits == 128 {
        return value;
    }
    let mask: u128 = (1u128 << bits) - 1;
    let narrowed = (value as u128) & mask;
    if signed != 0 && (narrowed >> (bits - 1)) & 1 == 1 {
        (narrowed | !mask) as i128
    } else {
        narrowed as i128
    }
}

/// The minimum value of the `(bits, signed)` width.
fn width_min(bits: i32, signed: i32) -> i128 {
    if signed != 0 {
        -(1i128 << (bits - 1))
    } else {
        0
    }
}

/// The maximum value of the `(bits, signed)` width.
fn width_max(bits: i32, signed: i32) -> i128 {
    if signed != 0 {
        (1i128 << (bits - 1)) - 1
    } else if bits >= 128 {
        -1 // u128::MAX reinterpreted
    } else {
        (1i128 << bits) - 1
    }
}

/// `v.abs()` — the magnitude. The `MIN` case never reaches here: lowering
/// throws `ArithmeticOverflowError` first.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_abs(value: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).wrapping_abs()
}

/// `v.sign()` — `-1`, `0`, or `1`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_sign(value: i128, bits: i32, signed: i32) -> i32 {
    normalize(value, bits, signed).signum() as i32
}

/// `v.min(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_min(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).min(normalize(other, bits, signed))
}

/// `v.max(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_max(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).max(normalize(other, bits, signed))
}

/// `v.clamp(lo, hi)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_clamp(value: i128, lo: i128, hi: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).clamp(normalize(lo, bits, signed), normalize(hi, bits, signed))
}

/// `v.is_zero()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_is_zero(value: i128) -> bool {
    value == 0
}

/// `v.is_even()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_is_even(value: i128) -> bool {
    value % 2 == 0
}

/// `v.is_odd()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_is_odd(value: i128) -> bool {
    value % 2 != 0
}

/// `v.bit_count()` — one bits within the width.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_bit_count(value: i128, bits: i32) -> i32 {
    let bits = bits.clamp(1, 128) as u32;
    let mask: u128 = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    ((value as u128) & mask).count_ones() as i32
}

/// `v.leading_zeros()` — width-scoped, so `(0: UInt8)` answers `8`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_leading_zeros(value: i128, bits: i32) -> i32 {
    let bits = bits.clamp(1, 128) as u32;
    let mask: u128 = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let narrowed = (value as u128) & mask;
    (narrowed.leading_zeros() - (128 - bits)) as i32
}

/// `v.trailing_zeros()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_trailing_zeros(value: i128, bits: i32) -> i32 {
    let bits = bits.clamp(1, 128) as u32;
    let mask: u128 = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let narrowed = (value as u128) & mask;
    narrowed.trailing_zeros().min(bits) as i32
}

/// `v.rotate_left(n)` — wraps bits within the width.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_rotate_left(value: i128, n: i64, bits: i32) -> i128 {
    let bits = bits.clamp(1, 128) as u32;
    let mask: u128 = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let narrowed = (value as u128) & mask;
    let n = (n.rem_euclid(bits as i64)) as u32;
    if n == 0 {
        return narrowed as i128;
    }
    let rotated = (narrowed << n) | (narrowed >> (bits - n));
    (rotated & mask) as i128
}

/// `v.rotate_right(n)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_rotate_right(value: i128, n: i64, bits: i32) -> i128 {
    let width = bits.clamp(1, 128) as i64;
    zirk_int_rotate_left(value, width - n.rem_euclid(width), bits)
}

/// `v.wrapping_add(other)` — and `sub`/`mul` below: arithmetic that wraps
/// at the width, the deliberate form of what `+` traps on.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_wrapping_add(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_add(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.wrapping_sub(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_wrapping_sub(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_sub(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.wrapping_mul(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_wrapping_mul(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_mul(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.saturating_add(other)` — clamps at the width's bounds.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_saturating_add(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    let result = normalize(value, bits, signed).saturating_add(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// `v.saturating_sub(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_saturating_sub(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    let result = normalize(value, bits, signed).saturating_sub(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// `v.saturating_mul(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_saturating_mul(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    let result = normalize(value, bits, signed).saturating_mul(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// Whether the operation fits the width — the `ok` half of `checked_*`.
fn checked_ok(result: Option<i128>, bits: i32, signed: i32) -> bool {
    match result {
        Some(value) => value >= width_min(bits, signed) && value <= width_max(bits, signed),
        None => false,
    }
}

/// `v.checked_add(other)` — `ok` flag; `zirk_int_checked_add_value` reads
/// the wrapped result.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_add_ok(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    checked_ok(a.checked_add(b), bits, signed)
}

/// The wrapped `v + other` — only read when `checked_add_ok` is true.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_add_value(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    zirk_int_wrapping_add(value, other, bits, signed)
}

/// `v.checked_sub(other)` — `ok` flag.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_sub_ok(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    checked_ok(a.checked_sub(b), bits, signed)
}

/// The wrapped `v - other`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_sub_value(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    zirk_int_wrapping_sub(value, other, bits, signed)
}

/// `v.checked_mul(other)` — `ok` flag.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_mul_ok(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    checked_ok(a.checked_mul(b), bits, signed)
}

/// The wrapped `v * other`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_mul_value(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    zirk_int_wrapping_mul(value, other, bits, signed)
}

/// `v.checked_div(other)` — `ok` flag; `other == 0` and the `MIN / -1`
/// wrap both fail.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_div_ok(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return false;
    }
    checked_ok(a.checked_div(b), bits, signed)
}

/// `v / other` — only read when `checked_div_ok` is true.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_div_value(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return 0;
    }
    normalize(a.wrapping_div(b), bits, signed)
}

/// `v.checked_rem(other)` — `ok` flag.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_rem_ok(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return false;
    }
    checked_ok(a.checked_rem(b), bits, signed)
}

/// `v % other` — only read when `checked_rem_ok` is true.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_rem_value(
    value: i128,
    other: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return 0;
    }
    normalize(a.wrapping_rem(b), bits, signed)
}

/// `IntN.parse(text)` — `ok` flag: the text is a valid decimal literal in
/// the width's range.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_int_parse_ok(text: *const c_void, bits: i32, signed: i32) -> bool {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    let parsed = if signed != 0 {
        text.parse::<i128>().ok()
    } else {
        text.parse::<u128>().ok().map(|v| v as i128)
    };
    checked_ok(parsed, bits, signed)
}

/// `IntN.parse(text)` — the wrapped value; only read when
/// `zirk_int_parse_ok` is true.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_int_parse_value(text: *const c_void, bits: i32, signed: i32) -> i128 {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    let parsed = if signed != 0 {
        text.parse::<i128>().unwrap_or(0)
    } else {
        text.parse::<u128>().map(|v| v as i128).unwrap_or(0)
    };
    normalize(parsed, bits, signed)
}

/// `v.abs()` on a float.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_abs(value: f64) -> f64 {
    value.abs()
}

/// `v.sign()` — `-1`, `0`, or `1` (there is no `NaN` in Zirk, so no
/// fourth answer). The result is `Int32` to match the language surface.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// `v.min(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_min(value: f64, other: f64) -> f64 {
    value.min(other)
}

/// `v.max(other)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_max(value: f64, other: f64) -> f64 {
    value.max(other)
}

/// `v.clamp(lo, hi)`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_clamp(value: f64, lo: f64, hi: f64) -> f64 {
    value.clamp(lo, hi)
}

/// `v.is_zero()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_is_zero(value: f64) -> bool {
    value == 0.0
}

/// `v.floor()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_floor(value: f64) -> f64 {
    value.floor()
}

/// `v.ceil()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_ceil(value: f64) -> f64 {
    value.ceil()
}

/// `v.round()` — half away from zero, like the doc's `round` contract.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_round(value: f64) -> f64 {
    value.round()
}

/// `v.truncate()` — toward zero.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_truncate(value: f64) -> f64 {
    value.trunc()
}

/// `v.fraction()` — the part after the decimal point.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_fraction(value: f64) -> f64 {
    value.fract()
}

/// `v.is_finite()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_is_finite(value: f64) -> bool {
    value.is_finite()
}

/// `v.is_infinite()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_is_infinite(value: f64) -> bool {
    value.is_infinite()
}

/// `v.is_negative()`.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_is_negative(value: f64) -> bool {
    value.is_sign_negative()
}

/// `v.pow(n)` — an integer exponent.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_pow(value: f64, n: i64) -> f64 {
    value.powi(n as i32)
}

/// `v.sqrt()` — the argument's non-negativity is checked in lowering,
/// which throws `FloatNanError` before this runs.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_float_sqrt(value: f64) -> f64 {
    value.sqrt()
}

/// `FloatN.parse(text)` — `ok` flag.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_float_parse_ok(text: *const c_void) -> bool {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    // A NaN result is not a valid parse: Zirk's Float family has no NaN.
    text.parse::<f64>().is_ok_and(|v| !v.is_nan())
}

/// `v.to_string(radix: n)` — integer rendering in radix 2–36.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_int_to_string_radix(value: i128, radix: i32) -> *mut c_void {
    let radix = radix.clamp(2, 36) as u32;
    let unsigned = if value < 0 {
        (-value) as u128
    } else {
        value as u128
    };
    let mut digits = Vec::new();
    let mut remainder = unsigned;
    if remainder == 0 {
        return crate::string::alloc_owned("0");
    }
    while remainder > 0 {
        let d = (remainder % radix as u128) as u32;
        digits.push(std::char::from_digit(d, radix).unwrap_or('?'));
        remainder /= radix as u128;
    }
    digits.reverse();
    let mut text: String = digits.into_iter().collect();
    if value < 0 {
        text.insert(0, '-');
    }
    crate::string::alloc_owned(&text)
}

/// `v.checked_pow(exp)` — `ok` flag.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_pow_ok(value: i128, exp: i128, bits: i32, signed: i32) -> bool {
    if exp < 0 || exp > i32::MAX as i128 {
        return false;
    }
    let a = normalize(value, bits, signed);
    let e = exp as u32;
    checked_ok(a.checked_pow(e), bits, signed)
}

/// `v.checked_pow(exp)` value — only read when `ok` is true.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_int_checked_pow_value(
    value: i128,
    exp: i128,
    bits: i32,
    signed: i32,
) -> i128 {
    if exp < 0 {
        return 0;
    }
    let a = normalize(value, bits, signed);
    let e = exp as u32;
    normalize(a.wrapping_pow(e), bits, signed)
}

/// `IntN.parse(text, radix: n)` — `ok` flag.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_int_parse_radix_ok(
    text: *const c_void,
    radix: i32,
    bits: i32,
    signed: i32,
) -> bool {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    let radix = radix.clamp(2, 36) as u32;
    let parsed = if signed != 0 {
        i128::from_str_radix(text, radix).ok()
    } else {
        u128::from_str_radix(text, radix).ok().map(|v| v as i128)
    };
    checked_ok(parsed, bits, signed)
}

/// `IntN.parse(text, radix: n)` — the wrapped value.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_int_parse_radix_value(
    text: *const c_void,
    radix: i32,
    bits: i32,
    signed: i32,
) -> i128 {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    let radix = radix.clamp(2, 36) as u32;
    let parsed = if signed != 0 {
        i128::from_str_radix(text, radix).unwrap_or(0)
    } else {
        u128::from_str_radix(text, radix)
            .map(|v| v as i128)
            .unwrap_or(0)
    };
    normalize(parsed, bits, signed)
}

/// `FloatN.parse(text)` — the value; only read when
/// `zirk_float_parse_ok` is true.
///
/// # Safety
///
/// `text` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_float_parse_value(text: *const c_void) -> f64 {
    let text = unsafe { borrow(text) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("")
        .trim();
    text.parse::<f64>().unwrap_or(0.0)
}

/// Parses a small format spec and formats `value` accordingly.
///
/// Supported shape: `[0][width][.precision][e|E]`.
/// Examples: `".2"`, `"08.2"`, `"8"`, `"e"`, `"10.4E"`.
fn format_float(value: f64, spec: &str) -> String {
    let spec = spec.trim();
    let mut chars = spec.chars().peekable();

    let mut zero_pad = false;
    if chars.peek() == Some(&'0') {
        zero_pad = true;
        chars.next();
    }

    let mut width = 0usize;
    while let Some(&c) = chars.peek() {
        if let Some(d) = c.to_digit(10) {
            width = width.saturating_mul(10).saturating_add(d as usize);
            chars.next();
        } else {
            break;
        }
    }

    let mut precision = None::<usize>;
    if chars.peek() == Some(&'.') {
        chars.next();
        let mut p = 0usize;
        while let Some(&c) = chars.peek() {
            if let Some(d) = c.to_digit(10) {
                p = p.saturating_mul(10).saturating_add(d as usize);
                chars.next();
            } else {
                break;
            }
        }
        precision = Some(p);
    }

    let mut exp = None::<char>;
    if let Some(&c) = chars.peek()
        && (c == 'e' || c == 'E')
    {
        exp = Some(c);
        chars.next();
    }

    // Ignore trailing characters; they cannot be expressed by Rust's
    // dynamic formatting and are outside the supported subset.
    let _ = chars;

    if zero_pad {
        match (precision, exp) {
            (None, None) => format!("{value:0width$}"),
            (None, Some('e')) => format!("{value:0width$e}"),
            (None, Some('E')) => format!("{value:0width$E}"),
            (None, Some(_)) => unreachable!(),
            (Some(p), None) => format!("{value:0width$.p$}"),
            (Some(p), Some('e')) => format!("{value:0width$.p$e}"),
            (Some(p), Some('E')) => format!("{value:0width$.p$E}"),
            (Some(_), Some(_)) => unreachable!(),
        }
    } else {
        match (precision, exp) {
            (None, None) => format!("{value:width$}"),
            (None, Some('e')) => format!("{value:width$e}"),
            (None, Some('E')) => format!("{value:width$E}"),
            (None, Some(_)) => unreachable!(),
            (Some(p), None) => format!("{value:width$.p$}"),
            (Some(p), Some('e')) => format!("{value:width$.p$e}"),
            (Some(p), Some('E')) => format!("{value:width$.p$E}"),
            (Some(_), Some(_)) => unreachable!(),
        }
    }
}

/// `FloatN.format(spec)` — formats a float according to `spec`.
///
/// # Safety
///
/// `spec` must be a `String` handle produced by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_float_format(value: f64, spec: *const c_void) -> *mut c_void {
    let spec = unsafe { borrow(spec) }
        .map(|string| unsafe { string.as_str() })
        .unwrap_or("");
    alloc_owned(&format_float(value, spec))
}
