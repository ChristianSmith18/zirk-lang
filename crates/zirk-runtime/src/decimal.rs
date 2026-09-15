//! Runtime helpers for the exact base-ten `Float` type.
//!
//! `Float` is a base-ten decimal value: a signed 128-bit integer `coef` and a
//! non-negative decimal `scale`, denoting the exact rational `coef * 10^-scale`.
//! It is not IEEE 754 binary floating point — that is the separate `Float`
//! family, still backed by [`crate::scalar`] and [`crate::string`].
//!
//! Every value that leaves this module is *normalized*: trailing decimal zeros
//! are stripped, so `1.0` and `1.00` share one representation and equality is a
//! plain field compare after aligning scales.
//!
//! `+`, `-`, `*` and integer `**` are exact; a result that cannot fit 38
//! significant digits raises a controlled `ArithmeticOverflowError`. `/`, `%`,
//! `sqrt` and fractional `pow` round half-to-even to [`MAX_SIGNIFICANT_DIGITS`].
//! There is no `NaN` and no infinity — the operations that would need them are
//! controlled runtime errors, consistent with the rest of the language.
//!
//! The C ABI passes a `Float` by pointer, the same way [`crate::string`] passes
//! `Int128`: there is no stable by-value ABI for a 128-bit aggregate.

use std::ffi::c_void;

use crate::failure;
use crate::string::alloc_owned;
use zirk_runtime_core::decimal as core_decimal;
#[allow(unused_imports)]
pub use zirk_runtime_core::decimal::MIN_DIV_SCALE;
pub use zirk_runtime_core::decimal::{Decimal, RoundingMode};

/// Rounding budget for an inexact result, in significant decimal digits.
///
/// 28 is the `.NET decimal` / `rust_decimal` budget, proven for money and other
/// base-ten domains. Exact `+ - *` may use the full 38-digit coefficient; only
/// inexact results (`/`, `sqrt`, fractional `pow`) are rounded into this.
pub const MAX_SIGNIFICANT_DIGITS: u32 = 28;

/// Hard ceiling on the coefficient: `i128` holds at most 38 decimal digits.
const MAX_COEF_DIGITS: u32 = 38;

// ---------------------------------------------------------------------------
// Normalization and digit helpers
// ---------------------------------------------------------------------------

fn pow10_i128(exp: u32) -> Option<i128> {
    let mut acc: i128 = 1;
    for _ in 0..exp {
        acc = acc.checked_mul(10)?;
    }
    Some(acc)
}

fn digit_count(mut magnitude: u128) -> u32 {
    if magnitude == 0 {
        return 1;
    }
    let mut count = 0;
    while magnitude != 0 {
        magnitude /= 10;
        count += 1;
    }
    count
}

fn overflow() -> ! {
    failure::zirk_rt_overflow()
}

fn native_result<T>(result: Result<T, core_decimal::DecimalError>) -> T {
    match result {
        Ok(value) => value,
        Err(core_decimal::DecimalError::Overflow) => failure::zirk_rt_overflow(),
        Err(core_decimal::DecimalError::DivisionByZero) => failure::zirk_rt_division_by_zero(),
        Err(core_decimal::DecimalError::Domain) => failure::zirk_rt_decimal_domain(),
        Err(core_decimal::DecimalError::InvalidCast) => failure::zirk_rt_invalid_cast(),
    }
}

// ---------------------------------------------------------------------------
// 256-bit intermediate — only reached when the 128-bit path overflows
// ---------------------------------------------------------------------------

/// Unsigned 256-bit magnitude as `(high 128 bits, low 128 bits)`.
#[derive(Clone, Copy)]
struct U256 {
    hi: u128,
    lo: u128,
}

impl U256 {
    const ZERO: U256 = U256 { hi: 0, lo: 0 };

    fn to_u128(self) -> Option<u128> {
        (self.hi == 0).then_some(self.lo)
    }
}

/// Full-width `u128 * u128 -> u256`.
fn mul_u128(a: u128, b: u128) -> U256 {
    let (a_lo, a_hi) = (a as u64 as u128, a >> 64);
    let (b_lo, b_hi) = (b as u64 as u128, b >> 64);

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    // Sum the cross terms into the middle 128 bits, tracking carry into `hi`.
    let (mid, carry1) = lh.overflowing_add(hl);
    let (lo, carry2) = ll.overflowing_add(mid << 64);
    let hi = hh + (mid >> 64) + (carry1 as u128) * (1u128 << 64) + carry2 as u128;

    U256 { hi, lo }
}

/// `u256 / u128 -> (quotient u256, remainder u128)` by bitwise long division.
///
/// Only reached from the multiply/divide overflow paths; the divisor is always
/// non-zero (guarded in lowering) so the remainder fits `u128`.
fn divrem_u256(numerator: U256, divisor: u128) -> (U256, u128) {
    let mut quotient = U256::ZERO;
    let mut remainder: u128 = 0;

    for bit in (0..256).rev() {
        let numerator_bit = if bit >= 128 {
            (numerator.hi >> (bit - 128)) & 1
        } else {
            (numerator.lo >> bit) & 1
        };
        // remainder = (remainder << 1) | numerator_bit
        remainder = (remainder << 1) | numerator_bit;
        // The remainder can momentarily reach `divisor` but never `2*divisor`,
        // so a single subtraction restores the invariant.
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

// ---------------------------------------------------------------------------
// Rounding
// ---------------------------------------------------------------------------

/// Applies `mode` to `quotient` given the division `remainder` and `divisor`
/// magnitudes and the result `negative` flag. All magnitudes are unsigned.
fn round_quotient(
    quotient: u128,
    remainder: u128,
    divisor: u128,
    negative: bool,
    mode: RoundingMode,
) -> u128 {
    if remainder == 0 {
        return quotient;
    }
    // Compare `2*remainder` with `divisor` without overflowing: `remainder` is
    // always `< divisor`, so `divisor - remainder` is safe.
    let complement = divisor - remainder;
    let round_up = match mode {
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
    if round_up { quotient + 1 } else { quotient }
}

/// Reduces `(coef, scale)` to at most `max_digits` significant digits by
/// dividing out powers of ten with `mode` rounding. Never increases the scale.
fn reduce_to_digits(coef: i128, scale: u32, max_digits: u32, mode: RoundingMode) -> Decimal {
    let negative = coef < 0;
    let mut magnitude = coef.unsigned_abs();
    let mut scale = scale;

    while digit_count(magnitude) > max_digits && scale > 0 {
        let rem = magnitude % 10;
        magnitude /= 10;
        scale -= 1;
        let bumped = round_quotient(magnitude, rem, 10, negative, mode);
        magnitude = bumped;
    }

    if digit_count(magnitude) > MAX_COEF_DIGITS {
        overflow();
    }

    let signed = if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    };
    Decimal::new(signed, scale)
}

// ---------------------------------------------------------------------------
// Alignment for + / - / comparison
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Core arithmetic
// ---------------------------------------------------------------------------

fn add(a: Decimal, b: Decimal) -> Decimal {
    native_result(core_decimal::add(a, b))
}

fn sub(a: Decimal, b: Decimal) -> Decimal {
    native_result(core_decimal::sub(a, b))
}

fn mul(a: Decimal, b: Decimal) -> Decimal {
    native_result(core_decimal::mul(a, b))
}

/// Divides `a` by `b` to `target_scale` fractional digits, rounding with `mode`.
fn div_to_scale(a: Decimal, b: Decimal, target_scale: u32, mode: RoundingMode) -> Decimal {
    // `b` is non-zero here (guarded in lowering); `a` may be zero -> zero.
    if a.coef == 0 {
        return Decimal::ZERO;
    }
    // Compute at a working scale that keeps the multiplier non-negative
    // (`work >= a.scale` makes `raise = work - a.scale + b.scale >= 0`), then
    // round down to the requested scale.
    let work = target_scale.max(a.scale as u32);
    let raise = work - a.scale as u32 + b.scale as u32;

    let negative = (a.coef < 0) ^ (b.coef < 0);
    let divisor = b.coef.unsigned_abs();
    let dividend = a.coef.unsigned_abs();

    // `dividend * 10^raise`, in a 256-bit intermediate when it will not fit 128.
    let (quotient_mag, remainder) =
        match pow10_u128_checked(raise).and_then(|factor| dividend.checked_mul(factor)) {
            Some(scaled) => (scaled / divisor, scaled % divisor),
            None => {
                let wide = scale_u256(dividend, raise);
                let (q, r) = divrem_u256(wide, divisor);
                match q.to_u128() {
                    Some(mag) => (mag, r),
                    None => overflow(),
                }
            }
        };

    let rounded = round_quotient(quotient_mag, remainder, divisor, negative, mode);
    if digit_count(rounded) > MAX_COEF_DIGITS {
        overflow();
    }
    let signed = if negative {
        -(rounded as i128)
    } else {
        rounded as i128
    };
    let at_work = Decimal::new(signed, work);
    if work > target_scale {
        round_to_places(at_work, target_scale as i32, mode)
    } else {
        at_work
    }
}

fn pow10_u128_checked(exp: u32) -> Option<u128> {
    let mut acc: u128 = 1;
    for _ in 0..exp {
        acc = acc.checked_mul(10)?;
    }
    Some(acc)
}

/// `value * 10` on a 256-bit magnitude; overflows the process past 2^256.
fn u256_mul10(v: U256) -> U256 {
    let low = mul_u128(v.lo, 10);
    let hi10 = match v.hi.checked_mul(10) {
        Some(x) => x,
        None => overflow(),
    };
    let hi = match hi10.checked_add(low.hi) {
        Some(x) => x,
        None => overflow(),
    };
    U256 { hi, lo: low.lo }
}

/// `value * 10^exp` as a 256-bit magnitude; overflows the process past 2^256.
///
/// The genuinely pathological case (both operands of a division carrying
/// 30-plus-digit coefficients) can reach the ceiling here; ordinary decimals
/// stay far inside it.
fn scale_u256(value: u128, exp: u32) -> U256 {
    let mut acc = U256 { hi: 0, lo: value };
    for _ in 0..exp {
        acc = u256_mul10(acc);
    }
    acc
}

fn div_default(a: Decimal, b: Decimal) -> Decimal {
    native_result(core_decimal::div(a, b))
}

fn rem(a: Decimal, b: Decimal) -> Decimal {
    native_result(core_decimal::rem(a, b))
}

fn pow_int(base: Decimal, exponent: i64) -> Decimal {
    native_result(core_decimal::pow_int(base, exponent))
}

fn to_f64(value: Decimal) -> f64 {
    value.coef as f64 / 10f64.powi(value.scale as i32)
}

fn sqrt(value: Decimal) -> Decimal {
    native_result(core_decimal::sqrt(value))
}

fn pow_fractional(base: Decimal, exponent: Decimal) -> Decimal {
    native_result(core_decimal::pow(base, exponent))
}

fn compare(a: Decimal, b: Decimal) -> i32 {
    native_result(core_decimal::compare(a, b))
}

// ---------------------------------------------------------------------------
// Rounding to a fixed number of decimal places (member `round`)
// ---------------------------------------------------------------------------

fn round_to_places(value: Decimal, places: i32, mode: RoundingMode) -> Decimal {
    native_result(core_decimal::round_to_places(value, places, mode))
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Renders a `Decimal` as an exact decimal string.
fn format_decimal(value: Decimal) -> String {
    core_decimal::format(core_decimal::Decimal {
        coef: value.coef,
        scale: value.scale,
    })
}

/// Parses exact decimal / scientific text into a `Decimal`.
///
/// Returns `None` when the text is not a valid literal or its exact form needs
/// more than 38 significant digits.
fn from_str(text: &str) -> Option<Decimal> {
    core_decimal::parse(text).map(|value| Decimal {
        coef: value.coef,
        scale: value.scale,
    })
}

// ---------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------

/// # Safety
/// `a` and `b` must point at valid `Decimal` values; `out` at writable storage.
macro_rules! binary_op {
    ($name:ident, $f:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(out: *mut Decimal, a: *const Decimal, b: *const Decimal) {
            let a = unsafe { *a };
            let b = unsafe { *b };
            unsafe { *out = ($f)(a, b) };
        }
    };
}

binary_op!(zirk_rt_decimal_add, add);
binary_op!(zirk_rt_decimal_sub, sub);
binary_op!(zirk_rt_decimal_mul, mul);
binary_op!(zirk_rt_decimal_div, div_default);
binary_op!(zirk_rt_decimal_rem, rem);
binary_op!(zirk_rt_decimal_pow, pow_fractional);

/// `a ** n` with an integer exponent.
///
/// # Safety
/// `a`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_pow_i(out: *mut Decimal, a: *const Decimal, n: i64) {
    let a = unsafe { *a };
    unsafe { *out = pow_int(a, n) };
}

/// `a.div(b, mode, places)` — explicit rounding.
///
/// # Safety
/// `a`/`b`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_div_ex(
    out: *mut Decimal,
    a: *const Decimal,
    b: *const Decimal,
    mode: i32,
    places: i32,
) {
    let a = unsafe { *a };
    let b = unsafe { *b };
    let places = places.max(0) as u32;
    unsafe { *out = div_to_scale(a, b, places, RoundingMode::from_abi(mode)) };
}

/// `value.round(places, mode)`.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_round(
    out: *mut Decimal,
    value: *const Decimal,
    places: i32,
    mode: i32,
) {
    let value = unsafe { *value };
    unsafe { *out = round_to_places(value, places, RoundingMode::from_abi(mode)) };
}

/// `value.sqrt()`.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_sqrt(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    unsafe { *out = sqrt(value) };
}

/// `-value`.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_neg(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    unsafe {
        *out = Decimal {
            coef: -value.coef,
            scale: value.scale,
        }
    };
}

/// `value.abs()`.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_abs(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    let coef = if value.coef == i128::MIN {
        overflow()
    } else {
        value.coef.abs()
    };
    unsafe {
        *out = Decimal {
            coef,
            scale: value.scale,
        }
    };
}

/// `a <=> b` — `-1`, `0`, `1`.
///
/// # Safety
/// `a`/`b` must point at valid `Decimal` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_cmp(a: *const Decimal, b: *const Decimal) -> i32 {
    let a = unsafe { *a };
    let b = unsafe { *b };
    compare(a, b)
}

/// `value.min(other)` / `value.max(other)`.
///
/// # Safety
/// `a`/`b`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_min(
    out: *mut Decimal,
    a: *const Decimal,
    b: *const Decimal,
) {
    let (a, b) = (unsafe { *a }, unsafe { *b });
    unsafe { *out = if compare(a, b) <= 0 { a } else { b } };
}

/// # Safety
/// `a`/`b`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_max(
    out: *mut Decimal,
    a: *const Decimal,
    b: *const Decimal,
) {
    let (a, b) = (unsafe { *a }, unsafe { *b });
    unsafe { *out = if compare(a, b) >= 0 { a } else { b } };
}

/// `value.clamp(low, high)`.
///
/// # Safety
/// `value`/`low`/`high`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_clamp(
    out: *mut Decimal,
    value: *const Decimal,
    low: *const Decimal,
    high: *const Decimal,
) {
    let (value, low, high) = (unsafe { *value }, unsafe { *low }, unsafe { *high });
    let clamped = if compare(value, low) < 0 {
        low
    } else if compare(value, high) > 0 {
        high
    } else {
        value
    };
    unsafe { *out = clamped };
}

/// `value.is_zero()` / `value.is_negative()`.
///
/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_is_zero(value: *const Decimal) -> bool {
    (unsafe { *value }).coef == 0
}

/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_is_negative(value: *const Decimal) -> bool {
    (unsafe { *value }).coef < 0
}

/// `value.sign()` — `-1`, `0`, `1`.
///
/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_sign(value: *const Decimal) -> i32 {
    (unsafe { *value }).coef.signum() as i32
}

/// `value.scale` — the decimal scale as an `Int32`.
///
/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_scale(value: *const Decimal) -> i32 {
    (unsafe { *value }).scale as i32
}

/// `value.floor()` / `ceil()` / `truncate()` — the ties differ by direction.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_floor(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    unsafe { *out = round_to_places(value, 0, RoundingMode::Floor) };
}

/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_ceil(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    unsafe { *out = round_to_places(value, 0, RoundingMode::Ceil) };
}

/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_truncate(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    unsafe { *out = round_to_places(value, 0, RoundingMode::Down) };
}

/// `value.fraction()` — `value - truncate(value)`.
///
/// # Safety
/// `value`/`out` as in [`zirk_rt_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_fraction(out: *mut Decimal, value: *const Decimal) {
    let value = unsafe { *value };
    let whole = round_to_places(value, 0, RoundingMode::Down);
    unsafe { *out = sub(value, whole) };
}

/// `value.is_integer()`.
///
/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_is_integer(value: *const Decimal) -> bool {
    (unsafe { *value }).scale == 0
}

/// `Float` -> `String` (exact).
///
/// # Safety
/// `value` must point at a valid `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_str_from_decimal(value: *const Decimal) -> *mut c_void {
    let value = unsafe { *value };
    alloc_owned(&format_decimal(value))
}

/// `Float.parse(text)` — `ok` flag.
///
/// # Safety
/// `text` must be a runtime string handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_parse_ok(text: *const c_void) -> bool {
    read_str(text).and_then(|s| from_str(&s)).is_some()
}

/// `Float.parse(text)` — the value; only meaningful when `parse_ok` is true.
///
/// # Safety
/// `text` must be a runtime string handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_parse_value(out: *mut Decimal, text: *const c_void) {
    let parsed = read_str(text)
        .and_then(|s| from_str(&s))
        .unwrap_or(Decimal::ZERO);
    unsafe { *out = parsed };
}

/// The literal-text form the compiler emits for a `ConstDecimal`.
///
/// # Safety
/// `text`/`len` describe a valid UTF-8 byte range; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_from_literal(
    out: *mut Decimal,
    text: *const u8,
    len: usize,
) {
    let bytes = unsafe { std::slice::from_raw_parts(text, len) };
    let parsed = std::str::from_utf8(bytes)
        .ok()
        .and_then(from_str)
        // A literal the lexer accepted always parses; a miss is a compiler bug.
        .unwrap_or_else(|| failure::zirk_rt_overflow());
    unsafe { *out = parsed };
}

// -- conversions -----------------------------------------------------------

/// `Int128 -> Float` (exact, implicit).
///
/// # Safety
/// `value` points at an `i128`; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_from_i128(out: *mut Decimal, value: *const i128) {
    let value = unsafe { value.read_unaligned() };
    unsafe { *out = Decimal::new(value, 0) };
}

/// `Decimal -> IntN` (`IntN(...)` / `as`): truncates toward zero and rejects
/// a result outside the requested width or signedness.
///
/// # Safety
/// `value` points at a `Decimal`; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_to_i128_checked(
    out: *mut i128,
    value: *const Decimal,
    bits: u32,
    signed: bool,
) {
    let value = unsafe { *value };
    let divisor = pow10_i128(value.scale as u32).unwrap_or_else(|| failure::zirk_rt_invalid_cast());
    let truncated = value.coef / divisor;
    let in_range = if signed {
        bits == 128 || {
            let magnitude = 1_i128 << (bits - 1);
            truncated >= -magnitude && truncated < magnitude
        }
    } else {
        truncated >= 0 && (bits == 128 || (truncated as u128) < (1_u128 << bits))
    };
    if !in_range {
        failure::zirk_rt_invalid_cast();
    }
    unsafe { out.write_unaligned(truncated) };
}

/// `Float64 -> Decimal` (`Decimal(x)` / `as`, checked: rejects non-finite).
///
/// # Safety
/// `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_from_f64(out: *mut Decimal, value: f64) {
    if !value.is_finite() {
        failure::zirk_rt_invalid_cast();
    }
    // Shortest round-trip text, then rounded into the significant-digit budget.
    let parsed = from_str(&format!("{value}"))
        .map(|d| {
            reduce_to_digits(
                d.coef,
                d.scale as u32,
                MAX_SIGNIFICANT_DIGITS,
                RoundingMode::HalfEven,
            )
        })
        .unwrap_or(Decimal::ZERO);
    unsafe { *out = parsed };
}

/// `Decimal -> Float64` (`Float(x)` / `as`, may lose precision).
///
/// # Safety
/// `value` points at a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_decimal_to_f64(value: *const Decimal) -> f64 {
    to_f64(unsafe { *value })
}

// -- helpers -------------------------------------------------------------

fn read_str(handle: *const c_void) -> Option<String> {
    unsafe { crate::string::borrow(handle) }.map(|s| unsafe { s.as_str() }.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        from_str(text).expect("valid literal")
    }
    fn s(value: Decimal) -> String {
        format_decimal(value)
    }

    #[test]
    fn the_headline_case_holds() {
        assert_eq!(s(add(d("0.1"), d("0.2"))), "0.3");
        assert_eq!(compare(add(d("0.1"), d("0.2")), d("0.3")), 0);
    }

    #[test]
    fn trailing_zeros_do_not_affect_equality() {
        assert_eq!(compare(d("1.0"), d("1.00")), 0);
        assert_eq!(compare(d("2.50"), d("2.5")), 0);
        assert_eq!(d("1.50").scale, 1);
    }

    #[test]
    fn addition_aligns_scales() {
        assert_eq!(s(add(d("0.5"), d("0.025"))), "0.525");
        assert_eq!(s(sub(d("1"), d("0.1"))), "0.9");
        assert_eq!(s(add(d("-1.5"), d("2.25"))), "0.75");
    }

    #[test]
    fn exact_multiplication() {
        assert_eq!(s(mul(d("1.2"), d("0.5"))), "0.6");
        assert_eq!(s(mul(d("0.1"), d("0.1"))), "0.01");
        assert_eq!(s(pow_int(d("1.1"), 3)), "1.331");
    }

    #[test]
    fn wide_multiplication_rounds_back() {
        // Coefficients near 10^20 with a fractional scale: the exact product
        // overflows `i128`, takes the 256-bit path, and rounds back to fit.
        let big = d("99999999999.9999999999"); // coef ~1e21, scale 10
        let product = mul(big, big);
        assert!(digit_count(product.coef.unsigned_abs()) <= MAX_COEF_DIGITS);
        assert!(s(product).starts_with("9999999999999999999"));
    }

    #[test]
    fn division_exact_and_rounded() {
        assert_eq!(s(div_default(d("1"), d("4"))), "0.25");
        assert_eq!(s(div_default(d("1"), d("8"))), "0.125");
        assert_eq!(s(div_default(d("6"), d("2"))), "3");
        let third = div_default(d("1"), d("3"));
        assert!(third.scale as u32 >= MIN_DIV_SCALE);
        assert!(s(third).starts_with("0.33333333333333"));
        // significant digits stay within the budget
        assert!(digit_count(third.coef.unsigned_abs()) <= MAX_SIGNIFICANT_DIGITS);
    }

    #[test]
    fn division_half_even_control() {
        // 2.345 rounded to 2 places, half-even -> 2.34
        assert_eq!(
            s(round_to_places(d("2.345"), 2, RoundingMode::HalfEven)),
            "2.34"
        );
        // 2.355 -> 2.36
        assert_eq!(
            s(round_to_places(d("2.355"), 2, RoundingMode::HalfEven)),
            "2.36"
        );
        assert_eq!(
            s(round_to_places(d("2.345"), 2, RoundingMode::HalfUp)),
            "2.35"
        );
    }

    #[test]
    fn remainder_takes_the_dividend_sign() {
        assert_eq!(s(rem(d("-1.0"), d("0.3"))), "-0.1");
        assert_eq!(s(rem(d("7"), d("3"))), "1");
    }

    #[test]
    fn formatting_is_exact() {
        assert_eq!(s(d("123.400")), "123.4");
        assert_eq!(s(d("0.1")), "0.1");
        assert_eq!(s(d("-0.05")), "-0.05");
        assert_eq!(s(d("0")), "0");
        assert_eq!(s(d("1e-3")), "0.001");
        assert_eq!(s(d("6.25e-2")), "0.0625");
    }

    #[test]
    fn parse_rejects_oversized_and_garbage() {
        assert!(from_str("999999999999999999999999999999999999999").is_none()); // 39 digits
        assert!(from_str("abc").is_none());
        assert!(from_str("").is_none());
        assert!(from_str("1.2.3").is_none());
    }

    #[test]
    fn conversions_round_trip_where_exact() {
        let one_tenth = d("0.1");
        assert_eq!(to_f64(one_tenth), 0.1);
        assert_eq!(s(Decimal::new(42, 0)), "42");
    }

    #[test]
    fn decimal_to_integer_truncates_the_completed_value_toward_zero() {
        for (text, expected) in [("0.75", 0), ("3.9", 3), ("-3.9", -3)] {
            let value = d(text);
            let mut result = i128::MIN;
            unsafe { zirk_rt_decimal_to_i128_checked(&mut result, &value, 128, true) };
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn sqrt_is_close() {
        assert_eq!(s(sqrt(d("4"))), "2");
        assert_eq!(s(sqrt(d("0"))), "0");
        assert_eq!(s(sqrt(d("2.25"))), "1.5");
        let two = sqrt(d("2"));
        assert!(s(two).starts_with("1.41421356"));
    }

    #[test]
    fn a_and_b_negative_and_a_true_overflow() {
        assert_eq!(s(mul(d("-2"), d("3"))), "-6");
        assert_eq!(s(mul(d("-2"), d("-3"))), "6");
    }

    #[test]
    fn native_adapters_match_runtime_core_decimal_semantics() {
        let pairs = [
            ("0.1", "0.2"),
            ("99999999999.9999999999", "0.5"),
            ("-7.5", "2"),
        ];
        for (left, right) in pairs {
            let (left, right) = (d(left), d(right));
            assert_eq!(
                add(left, right),
                native_result(core_decimal::add(left, right))
            );
            assert_eq!(
                sub(left, right),
                native_result(core_decimal::sub(left, right))
            );
            assert_eq!(
                mul(left, right),
                native_result(core_decimal::mul(left, right))
            );
            assert_eq!(
                div_default(left, right),
                native_result(core_decimal::div(left, right))
            );
            assert_eq!(
                compare(left, right),
                native_result(core_decimal::compare(left, right))
            );
        }
        let value = d("2.355");
        assert_eq!(
            round_to_places(value, 2, RoundingMode::HalfEven),
            native_result(core_decimal::round_to_places(
                value,
                2,
                RoundingMode::HalfEven
            ))
        );
        assert!(core_decimal::parse("999999999999999999999999999999999999999").is_none());
        assert_eq!(
            core_decimal::div(d("1"), Decimal::ZERO),
            Err(core_decimal::DecimalError::DivisionByZero)
        );
    }
}
