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

/// A finite IEEE-754 binary128 value encoded in the platform-independent wire
/// layout used by LLVM's `fp128` type.
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
    pub const fn from_bits(bits: u128) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u128 {
        self.0
    }

    pub const fn zero(negative: bool) -> Self {
        Self((negative as u128) << 127)
    }

    pub fn from_decimal(text: &str) -> Result<Self, Float128Error> {
        let text = text.trim();
        let (negative, text) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
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

    pub fn neg(self) -> Self {
        Self(self.0 ^ (1_u128 << 127))
    }

    pub fn add(self, right: Self) -> Result<Self, Float128Error> {
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
        Self::from_scaled(left + right, exponent)
    }

    pub fn sub(self, right: Self) -> Result<Self, Float128Error> {
        self.add(right.neg())
    }

    pub fn mul(self, right: Self) -> Result<Self, Float128Error> {
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        Self::from_scaled(
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
        let (left_sign, left, left_exp) = self.unpack()?;
        let (right_sign, right, right_exp) = right.unpack()?;
        if right.is_zero() {
            return Err(Float128Error::DivisionByZero);
        }
        let (numerator, denominator) = if left_exp >= right_exp {
            (left << (left_exp - right_exp) as usize, right)
        } else {
            (left, right << (right_exp - left_exp) as usize)
        };
        Self::from_ratio(left_sign != right_sign, numerator, denominator)
    }

    /// Zirk's remainder uses truncated quotient semantics, like LLVM `frem`.
    pub fn rem(self, right: Self) -> Result<Self, Float128Error> {
        let (left_sign, left, left_exp) = self.unpack()?;
        let (_, right, right_exp) = right.unpack()?;
        if right.is_zero() {
            return Err(Float128Error::DivisionByZero);
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

    /// Matches the native runtime's documented display bridge for `Float128`.
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
