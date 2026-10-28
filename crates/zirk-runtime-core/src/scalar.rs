//! Pure scalar parsing and formatting shared by native and interpreted hosts.

/// Parses a Zirk integer literal in the requested radix.
pub fn parse_integer(text: &str, radix: u32, signed: bool) -> Option<i128> {
    let text = text.trim();
    if signed {
        i128::from_str_radix(text, radix).ok()
    } else {
        u128::from_str_radix(text, radix)
            .ok()
            .map(|value| value as i128)
    }
}

/// Parses a finite binary64 value. Zirk deliberately rejects NaN.
pub fn parse_float(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| !value.is_nan())
}

/// Rounds a finite binary64 value to IEEE-754 binary16 and widens it back.
///
/// Rust has no stable `f16`, so this is expressed through the binary16 bit
/// layout.  It keeps the target-width rounding boundary explicit instead of
/// accidentally treating `Float16` as `Float32`.
pub fn round_to_float16(value: f64) -> f64 {
    f16_bits_to_f64(float16_bits(value))
}

/// Converts binary64 to the IEEE-754 binary16 payload with round-to-nearest,
/// ties-to-even. The result can be decoded with [`float16_to_f64`].
pub fn float16_bits(value: f64) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 63) as u16 & 0x8000;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent == 0x7ff {
        return sign | 0x7c00 | if fraction == 0 { 0 } else { 0x0200 };
    }
    if exponent == 0 && fraction == 0 {
        return sign;
    }
    let unbiased = if exponent == 0 {
        -1022
    } else {
        exponent - 1023
    };
    if unbiased > 15 {
        return sign | 0x7c00;
    }
    let significand = if exponent == 0 {
        fraction
    } else {
        (1_u64 << 52) | fraction
    };
    let round_shift = |value: u64, shift: u32| {
        let truncated = value >> shift;
        let remainder = value & ((1_u64 << shift) - 1);
        let halfway = 1_u64 << (shift - 1);
        truncated + u64::from(remainder > halfway || (remainder == halfway && truncated & 1 == 1))
    };
    if unbiased >= -14 {
        let mut target_exponent = unbiased + 15;
        let rounded = round_shift(significand, 42);
        if rounded == 1 << 11 {
            target_exponent += 1;
            if target_exponent >= 31 {
                sign | 0x7c00
            } else {
                sign | ((target_exponent as u16) << 10)
            }
        } else {
            sign | ((target_exponent as u16) << 10) | (rounded as u16 & 0x03ff)
        }
    } else if unbiased < -25 {
        sign
    } else {
        let rounded = round_shift(significand, (28 - unbiased) as u32);
        if rounded >= 1 << 10 {
            sign | 0x0400
        } else {
            sign | rounded as u16
        }
    }
}

/// Widens an IEEE-754 binary16 payload exactly to binary64.
pub fn float16_to_f64(bits: u16) -> f64 {
    f16_bits_to_f64(bits)
}

fn f16_bits_to_f64(bits: u16) -> f64 {
    let sign = (bits as u64 & 0x8000) << 48;
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let fraction = (bits & 0x03ff) as u64;
    let bits64 = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let shift = fraction.leading_zeros() - (u64::BITS - 11);
            let normalized = fraction << shift;
            let exponent = (1023 - 14 - shift as i32) as u64;
            sign | (exponent << 52) | ((normalized & 0x03ff) << 42)
        }
        31 => sign | (0x7ff_u64 << 52) | (fraction << 42),
        _ => sign | (((exponent - 15 + 1023) as u64) << 52) | (fraction << 42),
    };
    f64::from_bits(bits64)
}

/// Masks `value` to `bits` and sign-extends when `signed` is set, so
/// arithmetic on narrow widths observes the same wrap points the type
/// system promises.
pub fn normalize(value: i128, bits: i32, signed: i32) -> i128 {
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
pub fn width_min(bits: i32, signed: i32) -> i128 {
    if signed != 0 {
        -(1i128 << (bits - 1))
    } else {
        0
    }
}

/// The maximum value of the `(bits, signed)` width.
pub fn width_max(bits: i32, signed: i32) -> i128 {
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
pub fn int_abs(value: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).wrapping_abs()
}

/// `v.sign()` — `-1`, `0`, or `1`.
pub fn int_sign(value: i128, bits: i32, signed: i32) -> i32 {
    normalize(value, bits, signed).signum() as i32
}

/// `v.min(other)`.
pub fn int_min(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).min(normalize(other, bits, signed))
}

/// `v.max(other)`.
pub fn int_max(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).max(normalize(other, bits, signed))
}

/// `v.clamp(lo, hi)`.
pub fn int_clamp(value: i128, lo: i128, hi: i128, bits: i32, signed: i32) -> i128 {
    normalize(value, bits, signed).clamp(normalize(lo, bits, signed), normalize(hi, bits, signed))
}

/// `v.bit_count()` — one bits within the width.
pub fn int_bit_count(value: i128, bits: i32) -> i32 {
    let bits = bits.clamp(1, 128) as u32;
    let mask: u128 = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    ((value as u128) & mask).count_ones() as i32
}

/// `v.leading_zeros()` — width-scoped, so `(0: UInt8)` answers `8`.
pub fn int_leading_zeros(value: i128, bits: i32) -> i32 {
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
pub fn int_trailing_zeros(value: i128, bits: i32) -> i32 {
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
pub fn int_rotate_left(value: i128, n: i64, bits: i32) -> i128 {
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
pub fn int_rotate_right(value: i128, n: i64, bits: i32) -> i128 {
    let width = bits.clamp(1, 128) as i64;
    int_rotate_left(value, width - n.rem_euclid(width), bits)
}

/// `v.wrapping_add(other)` — and `sub`/`mul` below: arithmetic that wraps
/// at the width, the deliberate form of what `+` traps on.
pub fn int_wrapping_add(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_add(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.wrapping_sub(other)`.
pub fn int_wrapping_sub(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_sub(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.wrapping_mul(other)`.
pub fn int_wrapping_mul(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    normalize(
        normalize(value, bits, signed).wrapping_mul(normalize(other, bits, signed)),
        bits,
        signed,
    )
}

/// `v.saturating_add(other)` — clamps at the width's bounds.
pub fn int_saturating_add(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    let result = normalize(value, bits, signed).saturating_add(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// `v.saturating_sub(other)`.
pub fn int_saturating_sub(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    let result = normalize(value, bits, signed).saturating_sub(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// `v.saturating_mul(other)`.
pub fn int_saturating_mul(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    let result = normalize(value, bits, signed).saturating_mul(normalize(other, bits, signed));
    result.clamp(width_min(bits, signed), width_max(bits, signed))
}

/// Whether the operation fits the width — the `ok` half of `checked_*`.
pub fn int_checked_ok(result: Option<i128>, bits: i32, signed: i32) -> bool {
    match result {
        Some(value) => value >= width_min(bits, signed) && value <= width_max(bits, signed),
        None => false,
    }
}

/// `v.checked_add(other)` — `ok` flag.
pub fn int_checked_add_ok(value: i128, other: i128, bits: i32, signed: i32) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    int_checked_ok(a.checked_add(b), bits, signed)
}

/// `v.checked_sub(other)` — `ok` flag.
pub fn int_checked_sub_ok(value: i128, other: i128, bits: i32, signed: i32) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    int_checked_ok(a.checked_sub(b), bits, signed)
}

/// `v.checked_mul(other)` — `ok` flag.
pub fn int_checked_mul_ok(value: i128, other: i128, bits: i32, signed: i32) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    int_checked_ok(a.checked_mul(b), bits, signed)
}

/// `v.checked_div(other)` — `ok` flag; `other == 0` and the `MIN / -1`
/// wrap both fail.
pub fn int_checked_div_ok(value: i128, other: i128, bits: i32, signed: i32) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return false;
    }
    int_checked_ok(a.checked_div(b), bits, signed)
}

/// `v / other` — only read when `checked_div_ok` is true.
pub fn int_checked_div_value(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return 0;
    }
    normalize(a.wrapping_div(b), bits, signed)
}

/// `v.checked_rem(other)` — `ok` flag.
pub fn int_checked_rem_ok(value: i128, other: i128, bits: i32, signed: i32) -> bool {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return false;
    }
    int_checked_ok(a.checked_rem(b), bits, signed)
}

/// `v % other` — only read when `checked_rem_ok` is true.
pub fn int_checked_rem_value(value: i128, other: i128, bits: i32, signed: i32) -> i128 {
    let a = normalize(value, bits, signed);
    let b = normalize(other, bits, signed);
    if b == 0 {
        return 0;
    }
    normalize(a.wrapping_rem(b), bits, signed)
}

/// `v.checked_pow(exp)` — `ok` flag.
pub fn int_checked_pow_ok(value: i128, exp: i128, bits: i32, signed: i32) -> bool {
    if exp < 0 || exp > i32::MAX as i128 {
        return false;
    }
    let a = normalize(value, bits, signed);
    let e = exp as u32;
    int_checked_ok(a.checked_pow(e), bits, signed)
}

/// `v.checked_pow(exp)` value — only read when `ok` is true.
pub fn int_checked_pow_value(value: i128, exp: i128, bits: i32, signed: i32) -> i128 {
    if exp < 0 {
        return 0;
    }
    let a = normalize(value, bits, signed);
    let e = exp as u32;
    normalize(a.wrapping_pow(e), bits, signed)
}

/// `IntN.parse(text)` — `ok` flag: the text is a valid decimal literal in
/// the width's range.
pub fn int_parse_ok(text: &str, bits: i32, signed: i32) -> bool {
    let parsed = parse_integer(text.trim(), 10, signed != 0);
    int_checked_ok(parsed, bits, signed)
}

/// `IntN.parse(text)` — the wrapped value; only read when `int_parse_ok`
/// is true.
pub fn int_parse_value(text: &str, bits: i32, signed: i32) -> i128 {
    let parsed = parse_integer(text.trim(), 10, signed != 0).unwrap_or(0);
    normalize(parsed, bits, signed)
}

/// `IntN.parse(text, radix: n)` — `ok` flag.
pub fn int_parse_radix_ok(text: &str, radix: i32, bits: i32, signed: i32) -> bool {
    let radix = radix.clamp(2, 36) as u32;
    let parsed = parse_integer(text.trim(), radix, signed != 0);
    int_checked_ok(parsed, bits, signed)
}

/// `IntN.parse(text, radix: n)` — the wrapped value.
pub fn int_parse_radix_value(text: &str, radix: i32, bits: i32, signed: i32) -> i128 {
    let radix = radix.clamp(2, 36) as u32;
    let parsed = parse_integer(text.trim(), radix, signed != 0).unwrap_or(0);
    normalize(parsed, bits, signed)
}

/// Renders an integer in a radix from 2 through 36.
pub fn format_integer_radix(value: i128, radix: i32) -> String {
    let radix = radix.clamp(2, 36) as u32;
    let unsigned = if value < 0 {
        value.wrapping_neg() as u128
    } else {
        value as u128
    };
    let mut digits = Vec::new();
    let mut remaining = unsigned;
    loop {
        let digit = (remaining % radix as u128) as u8;
        digits.push(if digit < 10 {
            (b'0' + digit) as char
        } else {
            (b'a' + digit - 10) as char
        });
        remaining /= radix as u128;
        if remaining == 0 {
            break;
        }
    }
    if value < 0 {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

/// Formats a float using Zirk's small `[0][width][.precision][e|E]` format
/// subset. Invalid trailing characters are ignored for native compatibility.
pub fn format_float(value: f64, spec: &str) -> String {
    let mut chars = spec.trim().chars().peekable();
    let zero_pad = chars.next_if_eq(&'0').is_some();
    let mut width = 0usize;
    while let Some(digit) = chars.peek().and_then(|character| character.to_digit(10)) {
        width = width.saturating_mul(10).saturating_add(digit as usize);
        chars.next();
    }
    let precision = if chars.next_if_eq(&'.').is_some() {
        let mut precision = 0usize;
        while let Some(digit) = chars.peek().and_then(|character| character.to_digit(10)) {
            precision = precision.saturating_mul(10).saturating_add(digit as usize);
            chars.next();
        }
        Some(precision)
    } else {
        None
    };
    let exponent = chars
        .next()
        .filter(|character| matches!(character, 'e' | 'E'));
    match (zero_pad, precision, exponent) {
        (true, None, None) => format!("{value:0width$}"),
        (true, None, Some('e')) => format!("{value:0width$e}"),
        (true, None, Some('E')) => format!("{value:0width$E}"),
        (true, Some(p), None) => format!("{value:0width$.p$}"),
        (true, Some(p), Some('e')) => format!("{value:0width$.p$e}"),
        (true, Some(p), Some('E')) => format!("{value:0width$.p$E}"),
        (false, None, None) => format!("{value:width$}"),
        (false, None, Some('e')) => format!("{value:width$e}"),
        (false, None, Some('E')) => format!("{value:width$E}"),
        (false, Some(p), None) => format!("{value:width$.p$}"),
        (false, Some(p), Some('e')) => format!("{value:width$.p$e}"),
        (false, Some(p), Some('E')) => format!("{value:width$.p$E}"),
        (_, _, Some(_)) => unreachable!("exponent filter limits this to e or E"),
    }
}

/// Converts IEEE-754 binary128 raw bits to the nearest binary64 value using
/// integer arithmetic, avoiding target-specific quad-float libcalls.
pub fn float128_bits_to_f64(bits: u128) -> f64 {
    let sign = (bits >> 127) & 1;
    let biased_exp = ((bits >> 112) & 0x7fff) as i32;
    let frac = bits & ((1_u128 << 112) - 1);
    let sign_f = if sign == 1 { -1.0 } else { 1.0 };
    if biased_exp == 0x7fff {
        return if frac == 0 {
            sign_f * f64::INFINITY
        } else {
            f64::NAN
        };
    }
    if biased_exp == 0 {
        return sign_f * 0.0;
    }
    let mut new_exp = biased_exp - 16383 + 1023;
    if new_exp >= 0x7ff {
        return sign_f * f64::INFINITY;
    }
    const SHIFT: u32 = 60;
    let mut mantissa = (frac >> SHIFT) as u64;
    let remainder = frac & ((1_u128 << SHIFT) - 1);
    let halfway = 1_u128 << (SHIFT - 1);
    let round_up = remainder > halfway || (remainder == halfway && mantissa & 1 == 1);
    if new_exp <= 0 {
        if new_exp <= -52 {
            return sign_f * 0.0;
        }
        let full = (1_u128 << 112) | frac;
        let total_shift = SHIFT + (1 - new_exp) as u32;
        let mut denorm = (full >> total_shift) as u64;
        let remainder = full & ((1_u128 << total_shift) - 1);
        let halfway = 1_u128 << (total_shift - 1);
        if remainder > halfway || (remainder == halfway && denorm & 1 == 1) {
            denorm += 1;
        }
        return f64::from_bits(((sign as u64) << 63) | denorm);
    }
    if round_up {
        mantissa += 1;
        if mantissa == 1_u64 << 52 {
            mantissa = 0;
            new_exp += 1;
            if new_exp >= 0x7ff {
                return sign_f * f64::INFINITY;
            }
        }
    }
    f64::from_bits(((sign as u64) << 63) | ((new_exp as u64) << 52) | mantissa)
}

/// Widens a binary64 value to its exact IEEE-754 binary128 bit pattern.
pub fn f64_to_float128_bits(value: f64) -> u128 {
    let bits64 = value.to_bits();
    let sign = (bits64 >> 63) & 1;
    let exponent = ((bits64 >> 52) & 0x7ff) as i64;
    let fraction = bits64 & ((1_u64 << 52) - 1);
    if exponent == 0x7ff {
        return ((sign as u128) << 127) | (0x7fff_u128 << 112) | ((fraction as u128) << 60);
    }
    if exponent == 0 {
        if fraction == 0 {
            return (sign as u128) << 127;
        }
        let msb = 63 - fraction.leading_zeros() as i64;
        let shift = 52 - msb;
        let exponent = 1 - 1023 - shift + 16383;
        let fraction = (fraction << shift) & ((1_u64 << 52) - 1);
        return ((sign as u128) << 127) | ((exponent as u128) << 112) | ((fraction as u128) << 60);
    }
    ((sign as u128) << 127)
        | (((exponent - 1023 + 16383) as u128) << 112)
        | ((fraction as u128) << 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_parse_and_format_are_platform_neutral() {
        assert_eq!(parse_integer(" ff ", 16, false), Some(255));
        assert_eq!(parse_integer("-8", 10, true), Some(-8));
        assert_eq!(parse_float("NaN"), None);
        assert_eq!(round_to_float16(0.1 + 0.2), 0.300048828125);
        assert_eq!(round_to_float16(65520.0), f64::INFINITY);
        assert_eq!(float16_to_f64(float16_bits(0.1)), 0.0999755859375);
        assert_eq!(float16_bits(1.00048828125), float16_bits(1.0));
        assert_eq!(format_integer_radix(-255, 16), "-ff");
        assert_eq!(format_float(1.5, "08.2"), "00001.50");
        assert_eq!(float128_bits_to_f64(f64_to_float128_bits(1.5)), 1.5);
    }
}
