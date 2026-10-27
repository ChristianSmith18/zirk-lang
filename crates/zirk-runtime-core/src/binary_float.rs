//! Portable IEEE-754 binary128 value operations.
//!
//! Rust stable has no `f128`.  This implementation keeps the binary128 bit
//! layout and performs the finite arithmetic required by Zirk with integer
//! significands, so browser execution does not silently collapse values to
//! binary64 or depend on target-specific quad-float libcalls.

use std::cmp::Ordering;

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, ToPrimitive, Zero};

const FRACTION_BITS: u32 = 112;
const PRECISION: u32 = FRACTION_BITS + 1;
const EXPONENT_BIAS: i32 = 16_383;
const MIN_NORMAL_EXPONENT: i32 = -16_382;
const MAX_NORMAL_EXPONENT: i32 = 16_383;
const SUBNORMAL_EXPONENT: i32 = -16_494;
const EXPONENT_MASK: u128 = 0x7fff;
const FRACTION_MASK: u128 = (1_u128 << FRACTION_BITS) - 1;
const DISPLAY_PRECISION: u32 = 36;

/// An IEEE-754 binary128 value encoded in the platform-independent wire layout
/// used by LLVM's `fp128` type. Decimal constructors accept finite values;
/// arithmetic may produce signed infinities or a quiet NaN for the caller's
/// language-level guard to handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Float128(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Float128Error {
    InvalidLiteral,
    NonFinite,
    DivisionByZero,
    Overflow,
}

#[allow(clippy::should_implement_trait)] // Names mirror Zirk IR BinaryOp variants.
impl Float128 {
    /// IEEE total ordering for all binary128 bit patterns, including signed
    /// zeros, infinities and NaN payloads.
    pub fn total_cmp(self, other: Self) -> std::cmp::Ordering {
        fn key(bits: u128) -> u128 {
            let sign = 1_u128 << 127;
            if bits & sign == 0 { bits | sign } else { !bits }
        }
        key(self.0).cmp(&key(other.0))
    }

    pub const fn from_bits(bits: u128) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u128 {
        self.0
    }

    pub const fn zero(negative: bool) -> Self {
        Self((negative as u128) << 127)
    }

    pub const fn infinity(negative: bool) -> Self {
        Self(((negative as u128) << 127) | (EXPONENT_MASK as u128) << FRACTION_BITS)
    }

    pub const fn quiet_nan() -> Self {
        Self(((EXPONENT_MASK as u128) << FRACTION_BITS) | (1_u128 << (FRACTION_BITS - 1)))
    }

    pub const fn is_nan(self) -> bool {
        ((self.0 >> FRACTION_BITS) & EXPONENT_MASK) == EXPONENT_MASK as u128
            && (self.0 & FRACTION_MASK) != 0
    }

    pub const fn is_infinite(self) -> bool {
        ((self.0 >> FRACTION_BITS) & EXPONENT_MASK) == EXPONENT_MASK as u128
            && (self.0 & FRACTION_MASK) == 0
    }

    pub fn from_decimal(text: &str) -> Result<Self, Float128Error> {
        let text = text.trim();
        let (negative, text) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        if text.eq_ignore_ascii_case("inf") || text.eq_ignore_ascii_case("infinity") {
            return Ok(Self::infinity(negative));
        }
        let (coefficient, explicit_exponent) = match text.find(['e', 'E']) {
            Some(position) => (
                &text[..position],
                text[position + 1..]
                    .parse::<i32>()
                    .map_err(|_| Float128Error::InvalidLiteral)?,
            ),
            None => (text, 0),
        };
        let mut digits = String::new();
        let mut decimals = 0_i32;
        let mut dot_seen = false;
        for character in coefficient.chars() {
            match character {
                '0'..='9' => {
                    digits.push(character);
                    if dot_seen {
                        decimals = decimals.saturating_add(1);
                    }
                }
                '.' if !dot_seen => dot_seen = true,
                _ => return Err(Float128Error::InvalidLiteral),
            }
        }
        if digits.is_empty() {
            return Err(Float128Error::InvalidLiteral);
        }
        let mut numerator =
            BigUint::parse_bytes(digits.as_bytes(), 10).ok_or(Float128Error::InvalidLiteral)?;
        if numerator.is_zero() {
            return Ok(Self::zero(negative));
        }
        let scale = decimals.saturating_sub(explicit_exponent);
        let denominator = if scale >= 0 {
            BigUint::from(10_u8).pow(scale as u32)
        } else {
            numerator *= BigUint::from(10_u8).pow(scale.unsigned_abs());
            BigUint::one()
        };
        Self::from_ratio(negative, numerator, denominator)
    }

    /// Encodes an internal value without passing through a lossy decimal
    /// formatter.  The interpreter keeps this representation private.
    pub fn wire_text(self) -> String {
        format!("@f128:{:032x}", self.0)
    }

    /// Formats a binary128 value with enough decimal digits to recover finite
    /// values when parsed back as binary128. The calculation stays
    /// in integer arithmetic so formatting does not depend on host `f64` or
    /// target-specific quad-precision libcalls.
    pub fn to_decimal_string(self) -> Result<String, Float128Error> {
        let sign_bit = self.0 >> 127 != 0;
        let exponent = (self.0 >> FRACTION_BITS) & EXPONENT_MASK;
        let fraction = self.0 & FRACTION_MASK;
        if exponent == EXPONENT_MASK {
            return if fraction == 0 {
                Ok(if sign_bit { "-inf" } else { "inf" }.to_owned())
            } else {
                Err(Float128Error::NonFinite)
            };
        }
        let (negative, significand, binary_exponent) = self.unpack()?;
        if significand.is_zero() {
            return Ok(if negative { "-0" } else { "0" }.to_owned());
        }

        let binary_magnitude = binary_exponent + significand.bits() as i32 - 1;
        let mut decimal_exponent =
            (binary_magnitude as f64 * std::f64::consts::LOG10_2).floor() as i32;
        while compare_to_power_of_ten(&significand, binary_exponent, decimal_exponent)
            == Ordering::Less
        {
            decimal_exponent -= 1;
        }
        while compare_to_power_of_ten(&significand, binary_exponent, decimal_exponent + 1)
            != Ordering::Less
        {
            decimal_exponent += 1;
        }

        let mut numerator = significand;
        let mut denominator = BigUint::one();
        if binary_exponent >= 0 {
            numerator <<= binary_exponent as usize;
        } else {
            denominator <<= binary_exponent.unsigned_abs() as usize;
        }

        let decimal_shift = DISPLAY_PRECISION as i32 - 1 - decimal_exponent;
        if decimal_shift >= 0 {
            numerator *= BigUint::from(10_u8).pow(decimal_shift as u32);
        } else {
            denominator *= BigUint::from(10_u8).pow(decimal_shift.unsigned_abs());
        }
        let mut coefficient = round_div(numerator, denominator);
        let mut digits = coefficient.to_str_radix(10);
        if digits.len() > DISPLAY_PRECISION as usize {
            coefficient /= BigUint::from(10_u8);
            decimal_exponent += 1;
            digits = coefficient.to_str_radix(10);
        }
        while digits.len() < DISPLAY_PRECISION as usize {
            digits.insert(0, '0');
        }
        while digits.ends_with('0') {
            digits.pop();
        }

        let mut rendered = if (-6..21).contains(&decimal_exponent) {
            let point = decimal_exponent + 1;
            if point <= 0 {
                format!("0.{}{}", "0".repeat((-point) as usize), digits)
            } else if point as usize >= digits.len() {
                format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
            } else {
                format!(
                    "{}.{}",
                    &digits[..point as usize],
                    &digits[point as usize..]
                )
            }
        } else if digits.len() == 1 {
            format!("{}e{}", digits, decimal_exponent)
        } else {
            format!("{}.{}e{}", &digits[..1], &digits[1..], decimal_exponent)
        };
        if negative {
            rendered.insert(0, '-');
        }
        Ok(rendered)
    }

    /// Formats this value with the subset of Rust's float width/precision
    /// specifiers exposed by Zirk's `FloatN.format` method, without narrowing
    /// binary128 through binary64.
    pub fn format(self, spec: &str) -> Result<String, Float128Error> {
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
        let exponent_style = chars
            .next()
            .filter(|character| matches!(character, 'e' | 'E'));
        if chars.next().is_some() {
            return Err(Float128Error::InvalidLiteral);
        }

        let bits_exponent = (self.0 >> FRACTION_BITS) & EXPONENT_MASK;
        let fraction = self.0 & FRACTION_MASK;
        if bits_exponent == EXPONENT_MASK {
            if fraction != 0 {
                return Err(Float128Error::NonFinite);
            }
            let mut value = if self.0 >> 127 != 0 {
                "-inf".to_owned()
            } else {
                "inf".to_owned()
            };
            pad_float(&mut value, width, zero_pad);
            return Ok(value);
        }

        let (negative, significand, binary_exponent) = self.unpack()?;
        if precision.is_none() && exponent_style.is_none() {
            let mut value = self.to_decimal_string()?;
            pad_float(&mut value, width, zero_pad);
            return Ok(value);
        }

        let requested_precision = precision.unwrap_or(6).min(10_000);
        let decimal_exponent = if significand.is_zero() {
            0
        } else {
            let magnitude = binary_exponent + significand.bits() as i32 - 1;
            let mut exponent = (magnitude as f64 * std::f64::consts::LOG10_2).floor() as i32;
            while compare_to_power_of_ten(&significand, binary_exponent, exponent) == Ordering::Less
            {
                exponent -= 1;
            }
            while compare_to_power_of_ten(&significand, binary_exponent, exponent + 1)
                != Ordering::Less
            {
                exponent += 1;
            }
            exponent
        };

        let mut numerator = significand;
        let mut denominator = BigUint::one();
        if binary_exponent >= 0 {
            numerator <<= binary_exponent as usize;
        } else {
            denominator <<= binary_exponent.unsigned_abs() as usize;
        }
        let decimal_shift = if exponent_style.is_some() {
            requested_precision as i32 - decimal_exponent
        } else {
            requested_precision as i32
        };
        if decimal_shift >= 0 {
            numerator *= BigUint::from(10_u8).pow(decimal_shift as u32);
        } else {
            denominator *= BigUint::from(10_u8).pow(decimal_shift.unsigned_abs());
        }
        let mut coefficient = round_div(numerator, denominator);
        let mut digits = coefficient.to_str_radix(10);

        let mut value = if let Some(style) = exponent_style {
            let significant_digits = requested_precision.saturating_add(1);
            if digits.len() > significant_digits {
                coefficient /= BigUint::from(10_u8);
                digits = coefficient.to_str_radix(10);
                let exponent = decimal_exponent + 1;
                if digits.len() < significant_digits {
                    digits.push_str(&"0".repeat(significant_digits - digits.len()));
                }
                format_exponential(&digits, exponent, style)
            } else {
                if digits.len() < significant_digits {
                    digits.insert_str(0, &"0".repeat(significant_digits - digits.len()));
                }
                format_exponential(&digits, decimal_exponent, style)
            }
        } else {
            if digits.len() <= requested_precision {
                digits.insert_str(0, &"0".repeat(requested_precision + 1 - digits.len()));
            }
            let point = digits.len() - requested_precision;
            let mut fixed = digits[..point].to_owned();
            if requested_precision > 0 {
                fixed.push('.');
                fixed.push_str(&digits[point..]);
            }
            fixed
        };
        if negative {
            value.insert(0, '-');
        }
        pad_float(&mut value, width, zero_pad);
        Ok(value)
    }

    pub fn from_wire_or_decimal(text: &str) -> Result<Self, Float128Error> {
        if let Some(bits) = text.strip_prefix("@f128:") {
            return u128::from_str_radix(bits, 16)
                .map(Self)
                .map_err(|_| Float128Error::InvalidLiteral);
        }
        Self::from_decimal(text)
    }

    pub fn from_i128(value: i128) -> Self {
        let negative = value.is_negative();
        let magnitude = value.unsigned_abs();
        Self::from_ratio(negative, BigUint::from(magnitude), BigUint::one())
            .expect("an i128 always fits binary128")
    }

    pub fn from_u128(value: u128) -> Self {
        Self::from_ratio(false, BigUint::from(value), BigUint::one())
            .expect("a u128 always fits binary128")
    }

    /// Converts a finite binary64 value exactly, preserving its binary value
    /// rather than parsing a decimal display approximation.
    pub fn from_f64(value: f64) -> Result<Self, Float128Error> {
        if value.is_nan() {
            return Err(Float128Error::NonFinite);
        }
        if value.is_infinite() {
            return Ok(Self::infinity(value.is_sign_negative()));
        }
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        if exponent == 0 && fraction == 0 {
            return Ok(Self::zero(negative));
        }
        let (significand, power) = if exponent == 0 {
            (fraction, -1074)
        } else {
            ((1_u64 << 52) | fraction, exponent - 1023 - 52)
        };
        if power >= 0 {
            Self::from_ratio(
                negative,
                BigUint::from(significand) << power as usize,
                BigUint::one(),
            )
        } else {
            Self::from_ratio(
                negative,
                BigUint::from(significand),
                BigUint::one() << power.unsigned_abs() as usize,
            )
        }
    }

    pub fn neg(self) -> Self {
        Self(self.0 ^ (1_u128 << 127))
    }

    fn is_zero_bits(self) -> bool {
        self.0 & !(1_u128 << 127) == 0
    }

    pub fn add(self, right: Self) -> Result<Self, Float128Error> {
        if self.is_nan() || right.is_nan() {
            return Ok(Self::quiet_nan());
        }
        if self.is_infinite() || right.is_infinite() {
            if self.is_infinite() && right.is_infinite() && (self.0 >> 127) != (right.0 >> 127) {
                return Ok(Self::quiet_nan());
            }
            return Ok(if self.is_infinite() { self } else { right });
        }
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        let exponent = left_exp.min(right_exp);
        let left = BigInt::from_biguint(
            if left_sign { Sign::Minus } else { Sign::Plus },
            left << (left_exp - exponent) as usize,
        );
        let right = BigInt::from_biguint(
            if right_sign { Sign::Minus } else { Sign::Plus },
            right << (right_exp - exponent) as usize,
        );
        Self::from_arithmetic_scaled(left + right, exponent)
    }

    pub fn sub(self, right: Self) -> Result<Self, Float128Error> {
        self.add(right.neg())
    }

    pub fn mul(self, right: Self) -> Result<Self, Float128Error> {
        if self.is_nan() || right.is_nan() {
            return Ok(Self::quiet_nan());
        }
        if self.is_infinite() || right.is_infinite() {
            if self.is_zero_bits() || right.is_zero_bits() {
                return Ok(Self::quiet_nan());
            }
            return Ok(Self::infinity((self.0 ^ right.0) >> 127 != 0));
        }
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        Self::from_arithmetic_scaled(
            BigInt::from_biguint(
                if left_sign == right_sign {
                    Sign::Plus
                } else {
                    Sign::Minus
                },
                left * right,
            ),
            left_exp + right_exp,
        )
    }

    pub fn div(self, right: Self) -> Result<Self, Float128Error> {
        if self.is_nan() || right.is_nan() || (self.is_infinite() && right.is_infinite()) {
            return Ok(Self::quiet_nan());
        }
        if self.is_infinite() {
            return Ok(Self::infinity((self.0 ^ right.0) >> 127 != 0));
        }
        if right.is_infinite() {
            return Ok(Self::zero((self.0 ^ right.0) >> 127 != 0));
        }
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        if right.is_zero() {
            if left.is_zero() {
                return Ok(Self::quiet_nan());
            }
            return Ok(Self::infinity(left_sign != right_sign));
        }
        let (numerator, denominator) = if left_exp >= right_exp {
            (left << (left_exp - right_exp) as usize, right)
        } else {
            (left, right << (right_exp - left_exp) as usize)
        };
        Self::from_arithmetic_ratio(left_sign != right_sign, numerator, denominator)
    }

    /// Zirk's remainder uses truncated quotient semantics, like LLVM `frem`.
    pub fn rem(self, right: Self) -> Result<Self, Float128Error> {
        if self.is_nan() || right.is_nan() || self.is_infinite() {
            return Ok(Self::quiet_nan());
        }
        if right.is_infinite() {
            return Ok(self);
        }
        let (left_sign, left, left_exp) = self.unpack()?;
        let (_, right, right_exp) = right.unpack()?;
        if right.is_zero() {
            return Ok(Self::quiet_nan());
        }
        let (numerator, denominator, quotient_shift) = if left_exp >= right_exp {
            (left << (left_exp - right_exp) as usize, right, right_exp)
        } else {
            (left, right << (right_exp - left_exp) as usize, left_exp)
        };
        let quotient = &numerator / &denominator;
        let product = quotient * denominator;
        let remainder = numerator - product;
        Self::from_scaled(
            BigInt::from_biguint(if left_sign { Sign::Minus } else { Sign::Plus }, remainder),
            quotient_shift,
        )
    }

    pub fn compare(self, right: Self) -> Result<Ordering, Float128Error> {
        if self.is_nan() || right.is_nan() {
            return Err(Float128Error::NonFinite);
        }
        if self.is_infinite() || right.is_infinite() {
            return Ok(match (self.is_infinite(), right.is_infinite()) {
                (true, true) => {
                    let left_negative = self.0 >> 127 != 0;
                    let right_negative = right.0 >> 127 != 0;
                    match (left_negative, right_negative) {
                        (true, true) | (false, false) => Ordering::Equal,
                        (true, false) => Ordering::Less,
                        (false, true) => Ordering::Greater,
                    }
                }
                (true, false) => {
                    if self.0 >> 127 != 0 {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
                (false, true) => {
                    if right.0 >> 127 != 0 {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                }
                (false, false) => unreachable!(),
            });
        }
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        if left.is_zero() && right.is_zero() {
            return Ok(Ordering::Equal);
        }
        if left_sign != right_sign {
            return Ok(if left_sign {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let exponent = left_exp.min(right_exp);
        let comparison = (left << (left_exp - exponent) as usize)
            .cmp(&(right << (right_exp - exponent) as usize));
        Ok(if left_sign {
            comparison.reverse()
        } else {
            comparison
        })
    }

    pub fn trunc_to_bigint(self) -> Result<BigInt, Float128Error> {
        let (negative, significand, exponent) = self.unpack()?;
        let magnitude = if exponent >= 0 {
            significand << exponent as usize
        } else {
            significand >> (-exponent) as usize
        };
        Ok(BigInt::from_biguint(
            if negative { Sign::Minus } else { Sign::Plus },
            magnitude,
        ))
    }

    pub fn trunc_to_integer(self, bits: u32, signed: bool) -> Result<i128, Float128Error> {
        let value = self.trunc_to_bigint()?;
        let (minimum, maximum) = if signed {
            (
                -(BigInt::one() << (bits - 1)),
                (BigInt::one() << (bits - 1)) - 1,
            )
        } else {
            (BigInt::zero(), (BigInt::one() << bits) - 1)
        };
        if value < minimum || value > maximum {
            return Err(Float128Error::Overflow);
        }
        if signed {
            value.to_i128().ok_or(Float128Error::Overflow)
        } else {
            value
                .to_u128()
                .map(|value| value as i128)
                .ok_or(Float128Error::Overflow)
        }
    }

    /// Truncates to an integer and returns its declared-width bit pattern.
    /// This preserves the upper half of `UInt128`, which cannot be represented
    /// by the interpreter's signed `i128` transport value.
    pub fn trunc_to_integer_bits(self, bits: u32, signed: bool) -> Result<u128, Float128Error> {
        if !(1..=128).contains(&bits) {
            return Err(Float128Error::Overflow);
        }
        let value = self.trunc_to_bigint()?;
        let (minimum, maximum) = if signed {
            (
                -(BigInt::one() << (bits - 1)),
                (BigInt::one() << (bits - 1)) - 1,
            )
        } else {
            (BigInt::zero(), (BigInt::one() << bits) - 1)
        };
        if value < minimum || value > maximum {
            return Err(Float128Error::Overflow);
        }
        if signed {
            value
                .to_i128()
                .map(|value| value as u128)
                .ok_or(Float128Error::Overflow)
        } else {
            value.to_u128().ok_or(Float128Error::Overflow)
        }
    }

    /// Lossy compatibility bridge for consumers that explicitly need `f64`.
    pub fn display_f64(self) -> f64 {
        super::scalar::float128_bits_to_f64(self.0)
    }

    fn unpack(self) -> Result<(bool, BigUint, i32), Float128Error> {
        let negative = self.0 >> 127 != 0;
        let exponent = ((self.0 >> FRACTION_BITS) & EXPONENT_MASK) as i32;
        let fraction = self.0 & FRACTION_MASK;
        if exponent == EXPONENT_MASK as i32 {
            return Err(Float128Error::NonFinite);
        }
        if exponent == 0 {
            Ok((negative, BigUint::from(fraction), SUBNORMAL_EXPONENT))
        } else {
            Ok((
                negative,
                BigUint::from((1_u128 << FRACTION_BITS) | fraction),
                exponent - EXPONENT_BIAS - FRACTION_BITS as i32,
            ))
        }
    }

    fn from_scaled(value: BigInt, exponent: i32) -> Result<Self, Float128Error> {
        let negative = value.sign() == Sign::Minus;
        Self::from_power_of_two(
            negative,
            value.abs().to_biguint().unwrap_or_default(),
            exponent,
        )
    }

    fn from_arithmetic_scaled(value: BigInt, exponent: i32) -> Result<Self, Float128Error> {
        let negative = value.sign() == Sign::Minus;
        Self::from_scaled(value, exponent).or_else(|error| {
            if error == Float128Error::Overflow {
                Ok(Self::infinity(negative))
            } else {
                Err(error)
            }
        })
    }

    fn from_arithmetic_ratio(
        negative: bool,
        numerator: BigUint,
        denominator: BigUint,
    ) -> Result<Self, Float128Error> {
        Self::from_ratio(negative, numerator, denominator).or_else(|error| {
            if error == Float128Error::Overflow {
                Ok(Self::infinity(negative))
            } else {
                Err(error)
            }
        })
    }

    fn from_power_of_two(
        negative: bool,
        numerator: BigUint,
        exponent: i32,
    ) -> Result<Self, Float128Error> {
        if numerator.is_zero() {
            return Ok(Self::zero(negative));
        }
        if exponent >= 0 {
            Self::from_ratio(negative, numerator << exponent as usize, BigUint::one())
        } else {
            Self::from_ratio(negative, numerator, BigUint::one() << (-exponent) as usize)
        }
    }

    fn from_ratio(
        negative: bool,
        numerator: BigUint,
        denominator: BigUint,
    ) -> Result<Self, Float128Error> {
        if numerator.is_zero() {
            return Ok(Self::zero(negative));
        }
        let mut exponent = numerator.bits() as i32 - denominator.bits() as i32;
        let comparison = if exponent >= 0 {
            numerator.cmp(&(denominator.clone() << exponent as usize))
        } else {
            (numerator.clone() << (-exponent) as usize).cmp(&denominator)
        };
        if comparison == Ordering::Less {
            exponent -= 1;
        }
        if exponent > MAX_NORMAL_EXPONENT {
            return Err(Float128Error::Overflow);
        }

        if exponent >= MIN_NORMAL_EXPONENT {
            let shift = FRACTION_BITS as i32 - exponent;
            let (scaled_numerator, scaled_denominator) = if shift >= 0 {
                (numerator << shift as usize, denominator)
            } else {
                (numerator, denominator << (-shift) as usize)
            };
            let mut significand = round_div(scaled_numerator, scaled_denominator);
            if significand.bits() > PRECISION as u64 {
                significand >>= 1usize;
                exponent += 1;
            }
            if exponent > MAX_NORMAL_EXPONENT {
                return Err(Float128Error::Overflow);
            }
            let fraction = significand.to_u128().unwrap_or(0) & FRACTION_MASK;
            return Ok(Self(
                ((negative as u128) << 127)
                    | ((exponent + EXPONENT_BIAS) as u128) << FRACTION_BITS
                    | fraction,
            ));
        }

        let significand = round_div(numerator << (-SUBNORMAL_EXPONENT) as usize, denominator);
        if significand.is_zero() {
            return Ok(Self::zero(negative));
        }
        if significand.bits() > FRACTION_BITS as u64 {
            return Ok(Self(
                ((negative as u128) << 127) | (1_u128 << FRACTION_BITS),
            ));
        }
        Ok(Self(
            ((negative as u128) << 127) | significand.to_u128().unwrap_or(0),
        ))
    }
}

fn compare_to_power_of_ten(
    significand: &BigUint,
    binary_exponent: i32,
    decimal_exponent: i32,
) -> Ordering {
    let mut numerator = significand.clone();
    let mut denominator = BigUint::one();
    if binary_exponent >= 0 {
        numerator <<= binary_exponent as usize;
    } else {
        denominator <<= binary_exponent.unsigned_abs() as usize;
    }
    if decimal_exponent >= 0 {
        denominator *= BigUint::from(10_u8).pow(decimal_exponent as u32);
    } else {
        numerator *= BigUint::from(10_u8).pow(decimal_exponent.unsigned_abs());
    }
    numerator.cmp(&denominator)
}

fn format_exponential(digits: &str, exponent: i32, style: char) -> String {
    let marker = if style == 'E' { 'E' } else { 'e' };
    let mantissa = if digits.len() <= 1 {
        format!("{}{}", digits, "0".repeat(digits.len().saturating_sub(1)))
    } else {
        format!("{}.{}", &digits[..1], &digits[1..])
    };
    format!("{mantissa}{marker}{exponent}")
}

fn pad_float(value: &mut String, width: usize, zero_pad: bool) {
    let padding = width.saturating_sub(value.len());
    if padding == 0 {
        return;
    }
    if zero_pad {
        let sign = value.starts_with('-');
        if sign {
            value.insert_str(1, &"0".repeat(padding));
        } else {
            value.insert_str(0, &"0".repeat(padding));
        }
    } else {
        value.insert_str(0, &" ".repeat(padding));
    }
}

fn round_div(numerator: BigUint, denominator: BigUint) -> BigUint {
    let quotient = &numerator / &denominator;
    let remainder = numerator % denominator.clone();
    let twice = remainder << 1usize;
    quotient.clone()
        + BigUint::from((twice > denominator) || (twice == denominator && quotient.bit(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_precision_beyond_binary64() {
        let one = Float128::from_decimal("1").unwrap();
        let unit = Float128::from_decimal("1.92592994438723585305597794258492732e-34").unwrap();
        assert_eq!(one.add(unit).unwrap().sub(one).unwrap(), unit);
    }

    #[test]
    fn arithmetic_and_conversion_are_finite_and_deterministic() {
        let left = Float128::from_decimal("0.1").unwrap();
        let right = Float128::from_decimal("0.2").unwrap();
        assert_eq!(
            left.add(right)
                .unwrap()
                .compare(Float128::from_decimal("0.3").unwrap()),
            Ok(Ordering::Greater)
        );
        assert_eq!(
            Float128::from_i128(-42).trunc_to_bigint().unwrap(),
            BigInt::from(-42)
        );
        assert_eq!(
            Float128::from_decimal("65504").unwrap().display_f64(),
            65504.0
        );
    }

    #[test]
    fn zero_division_distinguishes_infinity_from_nan() {
        let one = Float128::from_decimal("1").unwrap();
        let zero = Float128::zero(false);
        let infinity = one.div(zero).unwrap();
        let nan = zero.div(zero).unwrap();

        assert!(infinity.is_infinite());
        assert_eq!(infinity.to_decimal_string().unwrap(), "inf");
        assert_eq!(infinity.compare(one), Ok(Ordering::Greater));
        assert_eq!(infinity.compare(infinity), Ok(Ordering::Equal));
        assert_eq!(
            Float128::infinity(true).compare(infinity),
            Ok(Ordering::Less)
        );
        assert!(nan.is_nan());
        assert_eq!(nan.to_decimal_string(), Err(Float128Error::NonFinite));
        assert!(infinity.add(one).unwrap().is_infinite());
        assert!(infinity.add(Float128::infinity(true)).unwrap().is_nan());
        assert!(infinity.mul(zero).unwrap().is_nan());
        assert!(one.div(infinity).unwrap().is_zero_bits());
        assert!(infinity.div(infinity).unwrap().is_nan());
        assert_eq!(one.rem(infinity), Ok(one));
    }

    #[test]
    fn binary64_conversion_preserves_the_exact_source_value() {
        let converted = Float128::from_f64(0.1).unwrap();
        let exact_decimal = Float128::from_decimal("0.1").unwrap();
        assert_eq!(converted.compare(exact_decimal), Ok(Ordering::Greater));
        assert_eq!(
            Float128::from_f64(1.5).unwrap(),
            Float128::from_decimal("1.5").unwrap()
        );
        assert_eq!(
            Float128::from_f64(f64::INFINITY),
            Ok(Float128::infinity(false))
        );
    }

    #[test]
    fn decimal_display_round_trips_binary128_values() {
        let values = [
            Float128::from_decimal("0").unwrap(),
            Float128::from_decimal("-0").unwrap(),
            Float128::from_decimal("1.5").unwrap(),
            Float128::from_decimal("0.1").unwrap(),
            Float128::from_decimal("-3.1415926535897932384626433832795028841971").unwrap(),
            Float128::from_bits(1),
            Float128::from_bits(1_u128 << FRACTION_BITS),
            Float128::from_bits((0x7ffe_u128 << FRACTION_BITS) | FRACTION_MASK),
        ];
        for value in values {
            let rendered = value.to_decimal_string().unwrap();
            let reparsed = Float128::from_decimal(&rendered).unwrap();
            assert_eq!(reparsed, value, "{value:?} rendered as {rendered}");
        }
        assert_eq!(
            Float128::from_decimal("1.5").unwrap().to_decimal_string(),
            Ok("1.5".into())
        );
        assert_eq!(Float128::zero(true).to_decimal_string(), Ok("-0".into()));
        assert_eq!(
            Float128::from_bits(0x7fff_u128 << 112).to_decimal_string(),
            Ok("inf".into())
        );
        assert_eq!(
            Float128::from_bits((1_u128 << 127) | (0x7fff_u128 << 112)).to_decimal_string(),
            Ok("-inf".into())
        );
        assert_eq!(
            Float128::from_bits((0x7fff_u128 << 112) | 1).to_decimal_string(),
            Err(Float128Error::NonFinite)
        );
    }

    #[test]
    fn format_spec_preserves_binary128_precision_without_f64() {
        let value = Float128::from_decimal("1.23456").unwrap();
        assert_eq!(value.format(".3").unwrap(), "1.235");
        assert_eq!(value.format(".3e").unwrap(), "1.235e0");
        assert_eq!(value.format("010.2").unwrap(), "0000001.23");
        assert_eq!(Float128::zero(false).format(".2").unwrap(), "0.00");
        assert_eq!(
            Float128::from_bits(0x7fff_u128 << 112)
                .format(".2E")
                .unwrap(),
            "inf"
        );
    }

    #[test]
    fn integer_conversion_preserves_uint128_bits_and_checks_exclusive_bounds() {
        let high_bit = Float128::from_decimal("170141183460469231731687303715884105728").unwrap();
        assert_eq!(
            high_bit.trunc_to_integer_bits(128, false),
            Ok(1_u128 << 127)
        );
        assert_eq!(
            high_bit.trunc_to_integer_bits(128, true),
            Err(Float128Error::Overflow)
        );
        let upper = Float128::from_decimal("340282366920938463463374607431768211456").unwrap();
        assert_eq!(
            upper.trunc_to_integer_bits(128, false),
            Err(Float128Error::Overflow)
        );
        assert_eq!(
            Float128::from_decimal("-170141183460469231731687303715884105728")
                .unwrap()
                .trunc_to_integer_bits(128, true),
            Ok(1_u128 << 127)
        );
    }
}
