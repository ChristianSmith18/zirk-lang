//! Exact base-ten decimal values and platform-neutral arithmetic.
//!
//! This module contains no native ABI or host failure behavior.  Operations
//! return [`DecimalError`] so each consumer can map a language failure to its
//! own boundary (the native runtime's C ABI or the interpreter result model).

/// Significant-digit budget used for inexact operations.
pub const MAX_SIGNIFICANT_DIGITS: u32 = 28;
/// Minimum fractional precision selected by default decimal division.
pub const MIN_DIV_SCALE: u32 = 10;
const MAX_COEF_DIGITS: u32 = 38;

/// A normalized exact base-ten value: `coef * 10^-scale`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    pub coef: i128,
    pub scale: u8,
}

/// Controlled failures from decimal operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalError {
    Overflow,
    DivisionByZero,
    Domain,
    InvalidCast,
}

/// Rounding modes shared by native and interpreted execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingMode {
    HalfEven,
    HalfUp,
    HalfDown,
    Up,
    Down,
    Ceil,
    Floor,
}

impl RoundingMode {
    pub fn from_abi(raw: i32) -> Self {
        match raw {
            1 => Self::HalfUp,
            2 => Self::HalfDown,
            3 => Self::Up,
            4 => Self::Down,
            5 => Self::Ceil,
            6 => Self::Floor,
            _ => Self::HalfEven,
        }
    }
}

impl Decimal {
    pub const ZERO: Self = Self { coef: 0, scale: 0 };

    /// Builds a canonical value from an already representable coefficient.
    pub fn new(coef: i128, scale: u32) -> Self {
        normalize(coef, scale)
    }

    pub const fn is_zero(self) -> bool {
        self.coef == 0
    }
    pub const fn is_negative(self) -> bool {
        self.coef < 0
    }
    pub const fn sign(self) -> i32 {
        self.coef.signum() as i32
    }
    pub const fn is_integer(self) -> bool {
        self.scale == 0
    }
}

fn pow10(exp: u32) -> Option<i128> {
    let mut result = 1i128;
    for _ in 0..exp {
        result = result.checked_mul(10)?;
    }
    Some(result)
}

fn pow10_u128(exp: u32) -> Option<u128> {
    let mut result = 1u128;
    for _ in 0..exp {
        result = result.checked_mul(10)?;
    }
    Some(result)
}

const fn decimal_powers() -> [u128; 39] {
    let mut powers = [1; 39];
    let mut index = 1;
    while index < powers.len() {
        powers[index] = powers[index - 1] * 10;
        index += 1;
    }
    powers
}

const POWERS_OF_TEN: [u128; 39] = decimal_powers();

fn digit_count(magnitude: u128) -> u32 {
    let mut lower = 0;
    let mut upper = POWERS_OF_TEN.len();
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if magnitude >= POWERS_OF_TEN[middle] {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    lower as u32 + 1
}

/// Unsigned 256-bit magnitude, used only to retain native decimal semantics
/// when a representable rounded result has an unrepresentable intermediate.
#[derive(Clone, Copy)]
struct U256 {
    hi: u128,
    lo: u128,
}

impl U256 {
    const ZERO: Self = Self { hi: 0, lo: 0 };

    fn to_u128(self) -> Option<u128> {
        (self.hi == 0).then_some(self.lo)
    }
}

fn mul_u128(a: u128, b: u128) -> U256 {
    let (a_lo, a_hi) = (a as u64 as u128, a >> 64);
    let (b_lo, b_hi) = (b as u64 as u128, b >> 64);
    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;
    let (mid, carry1) = lh.overflowing_add(hl);
    let (lo, carry2) = ll.overflowing_add(mid << 64);
    let hi = hh + (mid >> 64) + (carry1 as u128) * (1u128 << 64) + carry2 as u128;
    U256 { hi, lo }
}

fn divrem_u256(numerator: U256, divisor: u128) -> (U256, u128) {
    let mut quotient = U256::ZERO;
    let mut remainder = 0u128;
    for bit in (0..256).rev() {
        let numerator_bit = if bit >= 128 {
            (numerator.hi >> (bit - 128)) & 1
        } else {
            (numerator.lo >> bit) & 1
        };
        remainder = (remainder << 1) | numerator_bit;
        if remainder >= divisor {
            remainder -= divisor;
            let word = if bit >= 128 {
                &mut quotient.hi
            } else {
                &mut quotient.lo
            };
            *word |= 1u128 << (bit % 128);
        }
    }
    (quotient, remainder)
}

fn add_one_u256(value: U256) -> Result<U256, DecimalError> {
    let (lo, carry) = value.lo.overflowing_add(1);
    Ok(U256 {
        hi: value
            .hi
            .checked_add(carry as u128)
            .ok_or(DecimalError::Overflow)?,
        lo,
    })
}

fn mul10_u256(value: U256) -> Result<U256, DecimalError> {
    let low = mul_u128(value.lo, 10);
    let hi = value
        .hi
        .checked_mul(10)
        .and_then(|high| high.checked_add(low.hi))
        .ok_or(DecimalError::Overflow)?;
    Ok(U256 { hi, lo: low.lo })
}

fn scale_u256(value: u128, exponent: u32) -> Result<U256, DecimalError> {
    let mut result = U256 { hi: 0, lo: value };
    for _ in 0..exponent {
        result = mul10_u256(result)?;
    }
    Ok(result)
}

fn normalize(mut coef: i128, mut scale: u32) -> Decimal {
    if coef == 0 {
        return Decimal::ZERO;
    }
    while scale > 0 && coef % 10 == 0 {
        coef /= 10;
        scale -= 1;
    }
    Decimal {
        coef,
        scale: scale.min(MAX_COEF_DIGITS) as u8,
    }
}

fn align(a: Decimal, b: Decimal) -> Result<(i128, i128, u32), DecimalError> {
    let scale = a.scale.max(b.scale) as u32;
    let left = a
        .coef
        .checked_mul(pow10(scale - a.scale as u32).ok_or(DecimalError::Overflow)?)
        .ok_or(DecimalError::Overflow)?;
    let right = b
        .coef
        .checked_mul(pow10(scale - b.scale as u32).ok_or(DecimalError::Overflow)?)
        .ok_or(DecimalError::Overflow)?;
    Ok((left, right, scale))
}

pub fn add(a: Decimal, b: Decimal) -> Result<Decimal, DecimalError> {
    let (left, right, scale) = align(a, b)?;
    Ok(Decimal::new(
        left.checked_add(right).ok_or(DecimalError::Overflow)?,
        scale,
    ))
}

pub fn sub(a: Decimal, b: Decimal) -> Result<Decimal, DecimalError> {
    let coefficient = b.coef.checked_neg().ok_or(DecimalError::Overflow)?;
    add(
        a,
        Decimal {
            coef: coefficient,
            scale: b.scale,
        },
    )
}

pub fn mul(a: Decimal, b: Decimal) -> Result<Decimal, DecimalError> {
    let scale = a.scale as u32 + b.scale as u32;
    if let Some(product) = a.coef.checked_mul(b.coef) {
        return Ok(Decimal::new(product, scale));
    }
    let negative = (a.coef < 0) ^ (b.coef < 0);
    let mut wide = mul_u128(a.coef.unsigned_abs(), b.coef.unsigned_abs());
    let mut scale = scale;
    loop {
        if let Some(magnitude) = wide.to_u128()
            && digit_count(magnitude) <= MAX_COEF_DIGITS
        {
            return Ok(Decimal::new(signed(magnitude, negative)?, scale));
        }
        if scale == 0 {
            return Err(DecimalError::Overflow);
        }
        let (quotient, remainder) = divrem_u256(wide, 10);
        wide = quotient;
        if remainder > 5 || (remainder == 5 && quotient.lo % 2 == 1) {
            wide = add_one_u256(wide)?;
        }
        scale -= 1;
    }
}

pub fn rem(a: Decimal, b: Decimal) -> Result<Decimal, DecimalError> {
    let (left, right, scale) = align(a, b)?;
    if right == 0 {
        return Err(DecimalError::DivisionByZero);
    }
    Ok(Decimal::new(left % right, scale))
}

pub fn compare(a: Decimal, b: Decimal) -> Result<i32, DecimalError> {
    let (left, right, _) = align(a, b)?;
    Ok(match left.cmp(&right) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    })
}

fn round_magnitude(
    quotient: u128,
    remainder: u128,
    divisor: u128,
    negative: bool,
    mode: RoundingMode,
) -> Result<u128, DecimalError> {
    if remainder == 0 {
        return Ok(quotient);
    }
    let complement = divisor - remainder;
    let increment = match mode {
        RoundingMode::Down => false,
        RoundingMode::Up => true,
        RoundingMode::Floor => negative,
        RoundingMode::Ceil => !negative,
        RoundingMode::HalfUp => remainder >= complement,
        RoundingMode::HalfDown => remainder > complement,
        RoundingMode::HalfEven => {
            remainder > complement || (remainder == complement && quotient % 2 == 1)
        }
    };
    if increment {
        quotient.checked_add(1).ok_or(DecimalError::Overflow)
    } else {
        Ok(quotient)
    }
}

fn signed(magnitude: u128, negative: bool) -> Result<i128, DecimalError> {
    if negative {
        if magnitude == (i128::MAX as u128) + 1 {
            Ok(i128::MIN)
        } else {
            i128::try_from(magnitude)
                .ok()
                .and_then(|v| v.checked_neg())
                .ok_or(DecimalError::Overflow)
        }
    } else {
        i128::try_from(magnitude).map_err(|_| DecimalError::Overflow)
    }
}

pub fn round_to_places(
    value: Decimal,
    places: i32,
    mode: RoundingMode,
) -> Result<Decimal, DecimalError> {
    let places = places.max(0) as u32;
    if value.scale as u32 <= places {
        return Ok(value);
    }
    let divisor = pow10(value.scale as u32 - places).ok_or(DecimalError::Overflow)? as u128;
    let magnitude = value.coef.unsigned_abs();
    let rounded = round_magnitude(
        magnitude / divisor,
        magnitude % divisor,
        divisor,
        value.coef < 0,
        mode,
    )?;
    Ok(Decimal::new(signed(rounded, value.coef < 0)?, places))
}

pub fn negate(value: Decimal) -> Result<Decimal, DecimalError> {
    Ok(Decimal {
        coef: value.coef.checked_neg().ok_or(DecimalError::Overflow)?,
        scale: value.scale,
    })
}

pub fn abs(value: Decimal) -> Result<Decimal, DecimalError> {
    Ok(Decimal {
        coef: value.coef.checked_abs().ok_or(DecimalError::Overflow)?,
        scale: value.scale,
    })
}

/// Divides to a requested fractional scale using the supplied rounding mode.
/// This is public so both the native ABI and the interpreter retain exactly the
/// same quotient, overflow, and rounding behaviour.
pub fn div_to_scale(
    a: Decimal,
    b: Decimal,
    target_scale: u32,
    mode: RoundingMode,
) -> Result<Decimal, DecimalError> {
    if b.coef == 0 {
        return Err(DecimalError::DivisionByZero);
    }
    if a.coef == 0 {
        return Ok(Decimal::ZERO);
    }
    let work = target_scale.max(a.scale as u32);
    let raise = work - a.scale as u32 + b.scale as u32;
    let dividend = a.coef.unsigned_abs();
    let divisor = b.coef.unsigned_abs();
    let negative = (a.coef < 0) ^ (b.coef < 0);
    let (quotient, remainder) =
        match pow10_u128(raise).and_then(|factor| dividend.checked_mul(factor)) {
            Some(scaled) => (scaled / divisor, scaled % divisor),
            None => {
                let (quotient, remainder) = divrem_u256(scale_u256(dividend, raise)?, divisor);
                (quotient.to_u128().ok_or(DecimalError::Overflow)?, remainder)
            }
        };
    let rounded = round_magnitude(quotient, remainder, divisor, negative, mode)?;
    if digit_count(rounded) > MAX_COEF_DIGITS {
        return Err(DecimalError::Overflow);
    }
    let at_work = Decimal::new(signed(rounded, negative)?, work);
    if work > target_scale {
        round_to_places(at_work, target_scale as i32, mode)
    } else {
        Ok(at_work)
    }
}

/// The standard decimal division contract: approximately 28 significant
/// digits, half-even rounding, and a useful minimum fractional precision.
pub fn div(a: Decimal, b: Decimal) -> Result<Decimal, DecimalError> {
    if b.coef == 0 {
        return Err(DecimalError::DivisionByZero);
    }
    if a.coef == 0 {
        return Ok(Decimal::ZERO);
    }
    // The quotient's decimal order is either the difference between the
    // operands' orders or one less. Resolve that final digit by comparing the
    // exact decimal magnitudes; converting through f64 loses precisely the
    // information needed around powers of ten.
    let magnitude_a = a.coef.unsigned_abs();
    let magnitude_b = b.coef.unsigned_abs();
    let digits_a = digit_count(magnitude_a);
    let digits_b = digit_count(magnitude_b);
    let order_a = digits_a as i32 - 1 - a.scale as i32;
    let order_b = digits_b as i32 - 1 - b.scale as i32;
    let candidate = order_a - order_b;
    let quotient_order = if compare_scaled_magnitude(
        magnitude_a,
        digits_a,
        -(a.scale as i32),
        magnitude_b,
        digits_b,
        candidate - b.scale as i32,
    ) != core::cmp::Ordering::Less
    {
        candidate
    } else {
        candidate - 1
    };
    let integer_digits = (quotient_order + 1).max(1) as u32;
    let target = MAX_SIGNIFICANT_DIGITS
        .saturating_sub(integer_digits.saturating_sub(1))
        .max(
            if integer_digits <= MAX_SIGNIFICANT_DIGITS - MIN_DIV_SCALE {
                MIN_DIV_SCALE
            } else {
                0
            },
        );
    let full = div_to_scale(a, b, target, RoundingMode::HalfEven)?;
    reduce_to_digits(full, MAX_SIGNIFICANT_DIGITS, RoundingMode::HalfEven)
}

/// Compares `a * 10^a_exp` and `b * 10^b_exp` without floating point or
/// constructing potentially overflowing powers of ten.
fn compare_scaled_magnitude(
    a: u128,
    a_digits: u32,
    a_exp: i32,
    b: u128,
    b_digits: u32,
    b_exp: i32,
) -> core::cmp::Ordering {
    use core::cmp::Ordering;

    let order =
        (i64::from(a_digits) + i64::from(a_exp)).cmp(&(i64::from(b_digits) + i64::from(b_exp)));
    if order != Ordering::Equal {
        return order;
    }

    match (a_digits, b_digits) {
        (a_digits, b_digits) if a_digits == b_digits => a.cmp(&b),
        (a_digits, b_digits) if a_digits < b_digits => {
            let factor = pow10_u128(b_digits - a_digits).expect("u128 decimal width is bounded");
            let (b_prefix, b_remainder) = (b / factor, b % factor);
            match a.cmp(&b_prefix) {
                Ordering::Equal if b_remainder != 0 => Ordering::Less,
                order => order,
            }
        }
        (a_digits, b_digits) => {
            let factor = pow10_u128(a_digits - b_digits).expect("u128 decimal width is bounded");
            let (a_prefix, a_remainder) = (a / factor, a % factor);
            match a_prefix.cmp(&b) {
                Ordering::Equal if a_remainder != 0 => Ordering::Greater,
                order => order,
            }
        }
    }
}

fn reduce_to_digits(
    value: Decimal,
    max_digits: u32,
    mode: RoundingMode,
) -> Result<Decimal, DecimalError> {
    let negative = value.coef < 0;
    let mut magnitude = value.coef.unsigned_abs();
    let mut scale = value.scale as u32;
    while digit_count(magnitude) > max_digits && scale > 0 {
        magnitude = round_magnitude(magnitude / 10, magnitude % 10, 10, negative, mode)?;
        scale -= 1;
    }
    if digit_count(magnitude) > MAX_COEF_DIGITS {
        return Err(DecimalError::Overflow);
    }
    Ok(Decimal::new(signed(magnitude, negative)?, scale))
}

/// Raises `base` to an integer exponent, including reciprocal results.
pub fn pow_int(base: Decimal, exponent: i128) -> Result<Decimal, DecimalError> {
    if exponent == 0 {
        return Ok(Decimal::new(1, 0));
    }
    let negative = exponent < 0;
    let mut exponent = exponent.unsigned_abs();
    let mut result = Decimal::new(1, 0);
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul(result, factor)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = mul(factor, factor)?;
        }
    }
    if negative {
        div(Decimal::new(1, 0), result)
    } else {
        Ok(result)
    }
}

pub fn to_f64(value: Decimal) -> f64 {
    value.coef as f64 / 10f64.powi(value.scale as i32)
}

/// Converts a finite binary float into its exact decimal spelling. Consumers
/// use this only at an explicit binary-float to decimal conversion boundary.
/// Matches the native `zirk_rt_decimal_from_f64`: shortest round-trip text,
/// then rounded into the significant-digit budget.
pub fn from_f64_checked(value: f64) -> Result<Decimal, DecimalError> {
    if !value.is_finite() {
        return Err(DecimalError::Domain);
    }
    let parsed = parse(&format!("{value}"))
        .map(|d| reduce_to_digits(d, MAX_SIGNIFICANT_DIGITS, RoundingMode::HalfEven));
    parsed.unwrap_or(Ok(Decimal::ZERO))
}

pub fn sqrt(value: Decimal) -> Result<Decimal, DecimalError> {
    if value.coef < 0 {
        return Err(DecimalError::Domain);
    }
    if value.coef == 0 {
        return Ok(Decimal::ZERO);
    }
    from_f64_checked(to_f64(value).sqrt())
}

pub fn pow(base: Decimal, exponent: Decimal) -> Result<Decimal, DecimalError> {
    if exponent.scale == 0 {
        return pow_int(base, exponent.coef);
    }
    from_f64_checked(to_f64(base).powf(to_f64(exponent)))
}

/// Truncates an exact decimal toward zero and checks the requested Zirk
/// integer width, matching the native `zirk_rt_decimal_to_i128_checked` ABI
/// helper without exposing pointer-based ABI details.
pub fn to_i128_checked(value: Decimal, bits: u32, signed: bool) -> Result<i128, DecimalError> {
    if !(1..=128).contains(&bits) {
        return Err(DecimalError::InvalidCast);
    }
    let divisor = pow10(value.scale as u32).ok_or(DecimalError::Overflow)?;
    let truncated = value.coef / divisor;
    let fits = if signed {
        if bits == 128 {
            true
        } else {
            let magnitude = 1_i128 << (bits - 1);
            truncated >= -magnitude && truncated < magnitude
        }
    } else {
        truncated >= 0 && (bits == 128 || (truncated as u128) < (1_u128 << bits))
    };
    fits.then_some(truncated).ok_or(DecimalError::InvalidCast)
}

pub fn format(value: Decimal) -> String {
    let negative = value.coef < 0;
    let digits = value.coef.unsigned_abs().to_string();
    let scale = value.scale as usize;
    let body = if scale == 0 {
        digits
    } else if digits.len() > scale {
        let point = digits.len() - scale;
        format!("{}.{}", &digits[..point], &digits[point..])
    } else {
        format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
    };
    if negative { format!("-{body}") } else { body }
}

pub fn parse(text: &str) -> Option<Decimal> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (negative, rest) = match mantissa.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, mantissa.strip_prefix('+').unwrap_or(mantissa)),
    };
    let (integer, fraction) = match rest.split_once('.') {
        Some((i, f)) => (i, f),
        None => (rest, ""),
    };
    if rest.is_empty()
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
        || (integer.is_empty() && fraction.is_empty())
    {
        return None;
    }
    let digits = format!("{integer}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(Decimal::ZERO);
    }
    if digits.len() as u32 > MAX_COEF_DIGITS {
        return None;
    }
    let magnitude = digits.parse::<u128>().ok()?;
    let scale = fraction.len() as i64 - exponent;
    let coefficient = if scale >= 0 {
        signed(magnitude, negative).ok()?
    } else {
        let factor = pow10((-scale) as u32)?;
        signed(magnitude, negative).ok()?.checked_mul(factor)?
    };
    Some(Decimal::new(coefficient, scale.max(0) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decimal(text: &str) -> Decimal {
        parse(text).unwrap()
    }

    #[test]
    fn preserves_exact_baseline_operations() {
        assert_eq!(format(add(decimal("0.1"), decimal("0.2")).unwrap()), "0.3");
        assert_eq!(format(mul(decimal("1.2"), decimal("0.5")).unwrap()), "0.6");
        assert_eq!(
            format(round_to_places(decimal("2.345"), 2, RoundingMode::HalfEven).unwrap()),
            "2.34"
        );
        assert_eq!(compare(decimal("1.0"), decimal("1.00")).unwrap(), 0);
        assert_eq!(format(decimal("6.25e-2")), "0.0625");
    }

    #[test]
    fn reports_controlled_failures() {
        assert_eq!(
            rem(decimal("1"), Decimal::ZERO),
            Err(DecimalError::DivisionByZero)
        );
        assert_eq!(
            add(
                Decimal {
                    coef: i128::MAX,
                    scale: 0
                },
                decimal("1")
            ),
            Err(DecimalError::Overflow)
        );
    }

    #[test]
    fn retains_native_precision_for_wide_multiplication_and_division() {
        let wide = decimal("99999999999.9999999999");
        let product = mul(wide, wide).unwrap();
        assert!(digit_count(product.coef.unsigned_abs()) <= MAX_COEF_DIGITS);
        assert!(format(product).starts_with("9999999999999999999"));

        assert_eq!(format(div(decimal("1"), decimal("4")).unwrap()), "0.25");
        let third = div(decimal("1"), decimal("3")).unwrap();
        assert!(format(third).starts_with("0.33333333333333"));
        assert!(digit_count(third.coef.unsigned_abs()) <= MAX_SIGNIFICANT_DIGITS);
        assert_eq!(
            div(decimal("1"), Decimal::ZERO),
            Err(DecimalError::DivisionByZero)
        );
    }

    #[test]
    fn parsing_rounding_and_domain_failures_are_explicit() {
        assert_eq!(format(parse("6.25e-2").unwrap()), "0.0625");
        assert!(parse("999999999999999999999999999999999999999").is_none());
        assert_eq!(
            format(round_to_places(decimal("2.355"), 2, RoundingMode::HalfEven).unwrap()),
            "2.36"
        );
        assert_eq!(sqrt(decimal("-1")), Err(DecimalError::Domain));
    }

    #[test]
    fn integer_decimal_powers_preserve_i128_exponents() {
        let exponent = Decimal {
            coef: i64::MAX as i128 + 1,
            scale: 0,
        };
        assert_eq!(pow(Decimal::ZERO, exponent), Ok(Decimal::ZERO));
        assert_eq!(
            pow_int(Decimal::new(-1, 0), i128::MIN),
            Ok(Decimal::new(1, 0))
        );
    }

    #[test]
    fn division_precision_uses_exact_decimal_order_at_powers_of_ten() {
        let below_ten = div(
            decimal("9999999999999999999999999999"),
            decimal("1000000000000000000000000000"),
        )
        .unwrap();
        assert!(format(below_ten).starts_with("9."));
        assert_eq!(digit_count(below_ten.coef.unsigned_abs()), 28);

        let above_ten = div(
            decimal("10000000000000000000000000006"),
            decimal("1000000000000000000000000000"),
        )
        .unwrap();
        assert!(format(above_ten).starts_with("10."));
        assert_eq!(digit_count(above_ten.coef.unsigned_abs()), 28);
    }
}
