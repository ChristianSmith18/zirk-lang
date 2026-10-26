//! Typed, deterministic values used by compile-time type evaluation.
//!
//! The encoding is a compiler-internal canonical format, not a package ABI.
//! `type_identity` is the resolver's stable fully-qualified identity (including
//! package/version identity where applicable), never an arena index or pointer.

use crate::{FloatWidth, IntWidth};

const ENCODING_MAGIC: &[u8; 5] = b"ZKCV\x01";
const PACKAGE_METADATA_MAGIC: &[u8; 4] = b"ZKPM";
pub const PACKAGE_METADATA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageMetadataError {
    InvalidMagic,
    UnsupportedVersion(u16),
    Truncated,
    InvalidConstantEncoding,
    TooManyConstants,
}

impl PackageMetadataError {
    pub fn diagnostic_message(&self) -> String {
        match self {
            Self::UnsupportedVersion(version) => format!(
                "package constant metadata version {version} is incompatible; rebuild this package with the current Zirk compiler"
            ),
            Self::InvalidMagic => {
                "package constant metadata is invalid; rebuild this package".into()
            }
            Self::Truncated | Self::InvalidConstantEncoding | Self::TooManyConstants => {
                "package constant metadata is incomplete or invalid; rebuild this package".into()
            }
        }
    }
}

/// Versioned, portable singleton-value payloads stored in package type metadata.
/// Each entry is the canonical encoding of a [`CanonicalConst`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageConstMetadata {
    pub values: Vec<Vec<u8>>,
}

impl PackageConstMetadata {
    pub fn from_values(values: &[CanonicalConst]) -> Self {
        Self {
            values: values.iter().map(CanonicalConst::encode).collect(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut output = PACKAGE_METADATA_MAGIC.to_vec();
        output.extend_from_slice(&PACKAGE_METADATA_VERSION.to_le_bytes());
        put_len(&mut output, self.values.len());
        for value in &self.values {
            put_len(&mut output, value.len());
            output.extend_from_slice(value);
        }
        output
    }

    /// Decode and validate metadata before a package consumer uses it. A
    /// version mismatch is explicit so callers can request a package rebuild.
    pub fn decode(bytes: &[u8]) -> Result<Self, PackageMetadataError> {
        let mut reader = MetadataReader { bytes, offset: 0 };
        if reader.take(4)? != PACKAGE_METADATA_MAGIC {
            return Err(PackageMetadataError::InvalidMagic);
        }
        let version = u16::from_le_bytes(reader.array()?);
        if version != PACKAGE_METADATA_VERSION {
            return Err(PackageMetadataError::UnsupportedVersion(version));
        }
        let count =
            usize::try_from(reader.u64()?).map_err(|_| PackageMetadataError::TooManyConstants)?;
        if count > 1_000_000 {
            return Err(PackageMetadataError::TooManyConstants);
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let length =
                usize::try_from(reader.u64()?).map_err(|_| PackageMetadataError::Truncated)?;
            let encoded = reader.take(length)?;
            if !encoded.starts_with(ENCODING_MAGIC) {
                return Err(PackageMetadataError::InvalidConstantEncoding);
            }
            values.push(encoded.to_vec());
        }
        if reader.offset != bytes.len() {
            return Err(PackageMetadataError::Truncated);
        }
        Ok(Self { values })
    }
}

struct MetadataReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> MetadataReader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], PackageMetadataError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PackageMetadataError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(PackageMetadataError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PackageMetadataError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PackageMetadataError::Truncated)
    }

    fn u64(&mut self) -> Result<u64, PackageMetadataError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalConst {
    pub type_identity: String,
    pub value: ConstValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Boolean(bool),
    Integer {
        width: IntWidth,
        value: i128,
    },
    /// Unsigned payloads above `i128::MAX` (possible for UInt128) retain all
    /// 128 bits. It shares the canonical integer tag and wire representation.
    BigUnsigned {
        width: IntWidth,
        value: u128,
    },
    Decimal {
        coefficient: i128,
        scale: u8,
    },
    Float {
        width: FloatWidth,
        bits: u128,
    },
    String(String),
    Char(String),
    Null,
    Duration(i64),
    Date(i64),
    Time(i64),
    DateTime(i128),
    /// Absolute Unix timeline value in nanoseconds, with a resolved zone-free identity.
    Instant(i128),
    /// Canonical IANA identifier and the bundled IANA rule-data version.
    TimeZone {
        identifier: String,
        rule_data: String,
    },
    /// Timeline identity plus zone display identity and the exact rules used.
    ZonedDateTime {
        instant_nanos: i128,
        identifier: String,
        rule_data: String,
    },
    /// Calendar displacement components; this is never converted to elapsed time here.
    Period {
        years: i32,
        months: i32,
        weeks: i32,
        days: i32,
    },
    Enum {
        enum_identity: String,
        variant_identity: String,
        fields: Vec<CanonicalConst>,
    },
    Tuple {
        tuple_identity: String,
        elements: Vec<CanonicalConst>,
    },
    Aggregate {
        type_identity: String,
        fields: Vec<(String, CanonicalConst)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalConstError {
    IntegerOutOfRange,
    InvalidFloatWidthBits,
    FloatNaN,
    DecimalScaleOutOfRange,
    DecimalCoefficientOutOfRange,
}

impl CanonicalConst {
    /// Verify that this value is composed only of owned, recursively immutable
    /// compile-time values. Nominal classes are intentionally absent from the
    /// representation: only registered immutable enum constructors and
    /// anonymous object snapshots may produce aggregates.
    pub fn verify_immutable_value(&self) -> Result<(), &'static str> {
        match &self.value {
            ConstValue::Enum {
                enum_identity,
                variant_identity,
                fields,
            } => {
                if &self.type_identity != enum_identity
                    || !variant_identity.starts_with(&format!("{enum_identity}::"))
                {
                    return Err("enum value identity does not match its declared owner");
                }
                for field in fields {
                    field.verify_immutable_value()?;
                }
            }
            ConstValue::Tuple {
                tuple_identity,
                elements,
            } => {
                if &self.type_identity != tuple_identity {
                    return Err("tuple value identity does not match its declared element types");
                }
                for element in elements {
                    element.verify_immutable_value()?;
                }
            }
            ConstValue::Aggregate {
                type_identity,
                fields,
            } => {
                if &self.type_identity != type_identity
                    || !type_identity.starts_with("Object<{")
                    || !type_identity.ends_with("}>")
                {
                    return Err("aggregate is not a verified immutable object value");
                }
                if fields.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                    return Err("aggregate fields are not uniquely canonicalized");
                }
                for (_, field) in fields {
                    field.verify_immutable_value()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut output = ENCODING_MAGIC.to_vec();
        put_text(&mut output, &self.type_identity);
        self.value.encode_into(&mut output);
        output
    }

    /// Computes the serialized size without allocating the serialized buffer.
    /// Returns `None` as soon as the configured ceiling is exceeded.
    pub fn encoded_len_limited(&self, limit: u64) -> Option<u64> {
        let identity = text_len(&self.type_identity, limit)?;
        add_limited(5, identity, limit)?
            .checked_add(value_len_limited(&self.value, limit)?)
            .filter(|length| *length <= limit)
    }

    /// Charges one depth unit for each recursively nested aggregate value.
    /// Scalar fields do not consume aggregate depth.
    pub fn charge_aggregate_depth(
        &self,
        budget: &mut crate::type_budget::TypeComputationBudget,
    ) -> Result<(), crate::type_budget::BudgetExceeded> {
        if !matches!(
            self.value,
            ConstValue::Enum { .. } | ConstValue::Tuple { .. } | ConstValue::Aggregate { .. }
        ) {
            return Ok(());
        }
        match budget.with_depth(crate::type_budget::BudgetKind::AggregateDepth, |budget| {
            match &self.value {
                ConstValue::Enum { fields, .. } => {
                    for child in fields {
                        child.charge_aggregate_depth(budget)?;
                    }
                }
                ConstValue::Tuple { elements, .. } => {
                    for child in elements {
                        child.charge_aggregate_depth(budget)?;
                    }
                }
                ConstValue::Aggregate { fields, .. } => {
                    for (_, child) in fields {
                        child.charge_aggregate_depth(budget)?;
                    }
                }
                _ => unreachable!("aggregate check above covers recursive constants"),
            }
            Ok(())
        }) {
            Ok(()) => Ok(()),
            Err(crate::type_budget::DepthBudgetError::Exceeded(error)) => Err(error),
            Err(crate::type_budget::DepthBudgetError::Operation(error)) => Err(error),
            Err(crate::type_budget::DepthBudgetError::InvalidDimension(_)) => {
                unreachable!("aggregate depth uses a supported TypeComputationBudget dimension")
            }
        }
    }
}

fn add_limited(left: u64, right: u64, limit: u64) -> Option<u64> {
    left.checked_add(right).filter(|sum| *sum <= limit)
}

fn text_len(value: &str, limit: u64) -> Option<u64> {
    add_limited(8, u64::try_from(value.len()).ok()?, limit)
}

fn child_len(value: &CanonicalConst, limit: u64) -> Option<u64> {
    add_limited(8, value.encoded_len_limited(limit)?, limit)
}

fn values_len(values: &[CanonicalConst], limit: u64) -> Option<u64> {
    let mut length = 8_u64;
    for value in values {
        length = add_limited(length, child_len(value, limit)?, limit)?;
    }
    Some(length)
}

fn value_len_limited(value: &ConstValue, limit: u64) -> Option<u64> {
    match value {
        ConstValue::Boolean(_) => Some(2),
        ConstValue::Integer { .. } | ConstValue::BigUnsigned { .. } => Some(18),
        ConstValue::Decimal { .. } => Some(20),
        ConstValue::Float { .. } => Some(21),
        ConstValue::String(text) | ConstValue::Char(text) => {
            add_limited(1, text_len(text, limit)?, limit)
        }
        ConstValue::Null => Some(1),
        ConstValue::Duration(_) | ConstValue::Date(_) | ConstValue::Time(_) => Some(9),
        ConstValue::DateTime(_) | ConstValue::Instant(_) => Some(17),
        ConstValue::TimeZone {
            identifier,
            rule_data,
        } => add_limited(1, text_len(identifier, limit)?, limit)
            .and_then(|length| add_limited(length, text_len(rule_data, limit)?, limit)),
        ConstValue::ZonedDateTime {
            identifier,
            rule_data,
            ..
        } => {
            let length = add_limited(17, text_len(identifier, limit)?, limit)?;
            add_limited(length, text_len(rule_data, limit)?, limit)
        }
        ConstValue::Period { .. } => Some(17),
        ConstValue::Enum {
            enum_identity,
            variant_identity,
            fields,
        } => {
            let mut length = add_limited(1, text_len(enum_identity, limit)?, limit)?;
            length = add_limited(length, text_len(variant_identity, limit)?, limit)?;
            add_limited(length, values_len(fields, limit)?, limit)
        }
        ConstValue::Tuple {
            tuple_identity,
            elements,
        } => add_limited(
            add_limited(1, text_len(tuple_identity, limit)?, limit)?,
            values_len(elements, limit)?,
            limit,
        ),
        ConstValue::Aggregate {
            type_identity,
            fields,
        } => {
            let mut length = add_limited(1, text_len(type_identity, limit)?, limit)?;
            length = add_limited(length, 8, limit)?;
            for (name, value) in fields {
                length = add_limited(length, text_len(name, limit)?, limit)?;
                length = add_limited(length, child_len(value, limit)?, limit)?;
            }
            Some(length)
        }
    }
}

impl ConstValue {
    pub fn integer(width: IntWidth, value: i128) -> Result<Self, CanonicalConstError> {
        let valid = if width.signed() {
            let (min, max) = width.literal_range();
            (min..=max).contains(&value)
        } else {
            value >= 0 && (value as u128) <= unsigned_max(width)
        };
        if !valid {
            return Err(CanonicalConstError::IntegerOutOfRange);
        }
        Ok(Self::Integer { width, value })
    }

    /// Builds an unsigned integer without narrowing the full UInt128 domain.
    pub fn unsigned_integer(width: IntWidth, value: u128) -> Result<Self, CanonicalConstError> {
        if width.signed() || value > unsigned_max(width) {
            return Err(CanonicalConstError::IntegerOutOfRange);
        }
        if value <= i128::MAX as u128 {
            Ok(Self::Integer {
                width,
                value: value as i128,
            })
        } else {
            Ok(Self::BigUnsigned { width, value })
        }
    }

    /// Canonicalizes exact decimal spellings without passing through binary Float.
    pub fn decimal(coefficient: i128, scale: u32) -> Result<Self, CanonicalConstError> {
        if scale > 38 {
            return Err(CanonicalConstError::DecimalScaleOutOfRange);
        }
        if coefficient.unsigned_abs().to_string().len() > 38 {
            return Err(CanonicalConstError::DecimalCoefficientOutOfRange);
        }
        let mut coefficient = coefficient;
        let mut scale = scale;
        if coefficient == 0 {
            scale = 0;
        } else {
            while scale > 0 && coefficient % 10 == 0 {
                coefficient /= 10;
                scale -= 1;
            }
        }
        Ok(Self::Decimal {
            coefficient,
            scale: scale as u8,
        })
    }

    /// Validates raw IEEE bits, collapses signed zero, and rejects NaN because
    /// the language's ordered equality is non-reflexive for NaN.
    pub fn float(width: FloatWidth, bits: u128) -> Result<Self, CanonicalConstError> {
        let (total, exponent, fraction) = match width {
            FloatWidth::F16 => (16, 5, 10),
            FloatWidth::F32 => (32, 8, 23),
            FloatWidth::F64 => (64, 11, 52),
            FloatWidth::F128 => (128, 15, 112),
        };
        if total < 128 && bits >= (1u128 << total) {
            return Err(CanonicalConstError::InvalidFloatWidthBits);
        }
        let exp_mask = (1u128 << exponent) - 1;
        let frac_mask = (1u128 << fraction) - 1;
        let exp = (bits >> fraction) & exp_mask;
        let frac = bits & frac_mask;
        if exp == exp_mask && frac != 0 {
            return Err(CanonicalConstError::FloatNaN);
        }
        let magnitude_mask = (1u128 << (total - 1)) - 1;
        let canonical_bits = if bits & magnitude_mask == 0 { 0 } else { bits };
        Ok(Self::Float {
            width,
            bits: canonical_bits,
        })
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        match self {
            Self::Boolean(value) => {
                output.push(1);
                output.push(u8::from(*value));
            }
            Self::Integer { width, value } => {
                output.push(2);
                output.push(int_tag(*width));
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::BigUnsigned { width, value } => {
                output.push(2);
                output.push(int_tag(*width));
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::Decimal { coefficient, scale } => {
                output.push(3);
                output.extend_from_slice(&coefficient.to_le_bytes());
                output.push(*scale);
            }
            Self::Float { width, bits } => {
                output.push(4);
                output.push(float_tag(*width));
                output.extend_from_slice(&bits.to_le_bytes());
            }
            Self::String(value) => {
                output.push(5);
                put_text(output, value);
            }
            Self::Char(value) => {
                output.push(6);
                put_text(output, value);
            }
            Self::Null => output.push(7),
            Self::Duration(value) => {
                output.push(8);
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::Date(value) => {
                output.push(9);
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::Time(value) => {
                output.push(10);
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::DateTime(value) => {
                output.push(11);
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::Instant(value) => {
                output.push(15);
                output.extend_from_slice(&value.to_le_bytes());
            }
            Self::TimeZone {
                identifier,
                rule_data,
            } => {
                output.push(16);
                put_text(output, identifier);
                put_text(output, rule_data);
            }
            Self::ZonedDateTime {
                instant_nanos,
                identifier,
                rule_data,
            } => {
                output.push(17);
                output.extend_from_slice(&instant_nanos.to_le_bytes());
                put_text(output, identifier);
                put_text(output, rule_data);
            }
            Self::Period {
                years,
                months,
                weeks,
                days,
            } => {
                output.push(18);
                output.extend_from_slice(&years.to_le_bytes());
                output.extend_from_slice(&months.to_le_bytes());
                output.extend_from_slice(&weeks.to_le_bytes());
                output.extend_from_slice(&days.to_le_bytes());
            }
            Self::Enum {
                enum_identity,
                variant_identity,
                fields,
            } => {
                output.push(12);
                put_text(output, enum_identity);
                put_text(output, variant_identity);
                put_values(output, fields);
            }
            Self::Tuple {
                tuple_identity,
                elements,
            } => {
                output.push(13);
                put_text(output, tuple_identity);
                put_values(output, elements);
            }
            Self::Aggregate {
                type_identity,
                fields,
            } => {
                output.push(14);
                put_text(output, type_identity);
                put_len(output, fields.len());
                for (name, value) in fields {
                    put_text(output, name);
                    put_value(output, value);
                }
            }
        }
    }
}

fn unsigned_max(width: IntWidth) -> u128 {
    if width.signed() {
        return 0;
    }
    match width.bits() {
        128 => u128::MAX,
        bits => (1u128 << bits) - 1,
    }
}

fn int_tag(width: IntWidth) -> u8 {
    match width {
        IntWidth::I8 => 1,
        IntWidth::I16 => 2,
        IntWidth::I32 => 3,
        IntWidth::I64 => 4,
        IntWidth::I128 => 5,
        IntWidth::U8 => 6,
        IntWidth::U16 => 7,
        IntWidth::U32 => 8,
        IntWidth::U64 => 9,
        IntWidth::U128 => 10,
    }
}

fn float_tag(width: FloatWidth) -> u8 {
    match width {
        FloatWidth::F16 => 1,
        FloatWidth::F32 => 2,
        FloatWidth::F64 => 3,
        FloatWidth::F128 => 4,
    }
}
fn put_text(output: &mut Vec<u8>, value: &str) {
    put_len(output, value.len());
    output.extend_from_slice(value.as_bytes());
}
fn put_len(output: &mut Vec<u8>, length: usize) {
    output.extend_from_slice(&(length as u64).to_le_bytes());
}
fn put_value(output: &mut Vec<u8>, value: &CanonicalConst) {
    let encoded = value.encode();
    put_len(output, encoded.len());
    output.extend_from_slice(&encoded);
}
fn put_values(output: &mut Vec<u8>, values: &[CanonicalConst]) {
    put_len(output, values.len());
    for value in values {
        put_value(output, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(value: ConstValue) -> CanonicalConst {
        CanonicalConst {
            type_identity: "pkg@1::Decimal".into(),
            value,
        }
    }

    #[test]
    fn decimal_spellings_with_equal_exact_values_have_identical_encodings() {
        let one_decimal = ConstValue::decimal(10, 1).unwrap();
        let one_hundredth_spelling = ConstValue::decimal(100, 2).unwrap();
        assert_eq!(one_decimal, one_hundredth_spelling);
        assert_eq!(
            constant(one_decimal).encode(),
            constant(one_hundredth_spelling).encode()
        );
        assert_eq!(
            ConstValue::decimal(0, 18).unwrap(),
            ConstValue::decimal(0, 0).unwrap()
        );
    }

    #[test]
    fn integer_type_width_is_part_of_canonical_identity() {
        let i32_value = constant(ConstValue::integer(IntWidth::I32, 1).unwrap());
        let i64_value = constant(ConstValue::integer(IntWidth::I64, 1).unwrap());
        assert_ne!(i32_value.encode(), i64_value.encode());
        assert_eq!(
            ConstValue::integer(IntWidth::U8, 256),
            Err(CanonicalConstError::IntegerOutOfRange)
        );
    }

    #[test]
    fn unsigned_128_representation_preserves_the_full_value_domain() {
        let max = CanonicalConst {
            type_identity: "UInt128".into(),
            value: ConstValue::unsigned_integer(IntWidth::U128, u128::MAX).unwrap(),
        };
        let encoded = max.encode();
        assert!(matches!(
            max.value,
            ConstValue::BigUnsigned {
                width: IntWidth::U128,
                value: u128::MAX
            }
        ));
        let value_offset = ENCODING_MAGIC.len() + 8 + "UInt128".len();
        assert_eq!(encoded[value_offset], 2);
        assert_eq!(encoded[value_offset + 1], int_tag(IntWidth::U128));
        assert_eq!(
            &encoded[value_offset + 2..value_offset + 18],
            &u128::MAX.to_le_bytes()
        );
        assert_ne!(
            encoded,
            CanonicalConst {
                type_identity: "UInt128".into(),
                value: ConstValue::unsigned_integer(IntWidth::U128, u128::MAX - 1).unwrap(),
            }
            .encode()
        );
        assert_eq!(
            ConstValue::unsigned_integer(IntWidth::U64, u64::MAX as u128),
            Ok(ConstValue::Integer {
                width: IntWidth::U64,
                value: u64::MAX as i128,
            })
        );
        assert_eq!(
            ConstValue::unsigned_integer(IntWidth::U8, 256),
            Err(CanonicalConstError::IntegerOutOfRange)
        );
    }

    #[test]
    fn float_canonicalization_matches_ordered_equality() {
        let positive_zero = ConstValue::float(FloatWidth::F64, 0).unwrap();
        let negative_zero = ConstValue::float(FloatWidth::F64, 1u128 << 63).unwrap();
        assert_eq!(positive_zero, negative_zero);
        assert_eq!(
            ConstValue::float(FloatWidth::F32, 0x7fc0_0000),
            Err(CanonicalConstError::FloatNaN)
        );
        assert_ne!(
            ConstValue::float(FloatWidth::F64, 0x7ff0_0000_0000_0000).unwrap(),
            ConstValue::float(FloatWidth::F64, 0xfff0_0000_0000_0000).unwrap()
        );
    }

    #[test]
    fn aggregate_encoding_preserves_type_identity_and_field_order() {
        let a = CanonicalConst {
            type_identity: "pkg::Point".into(),
            value: ConstValue::Aggregate {
                type_identity: "pkg::Point".into(),
                fields: vec![
                    (
                        "x".into(),
                        CanonicalConst {
                            type_identity: "Int32".into(),
                            value: ConstValue::Integer {
                                width: IntWidth::I32,
                                value: 1,
                            },
                        },
                    ),
                    (
                        "y".into(),
                        CanonicalConst {
                            type_identity: "Int32".into(),
                            value: ConstValue::Integer {
                                width: IntWidth::I32,
                                value: 2,
                            },
                        },
                    ),
                ],
            },
        };
        let mut b = a.clone();
        if let ConstValue::Aggregate { fields, .. } = &mut b.value {
            fields.swap(0, 1);
        }
        assert_ne!(a.encode(), b.encode());
        assert_ne!(
            a.encode(),
            CanonicalConst {
                type_identity: "other::Point".into(),
                ..a
            }
            .encode()
        );
    }

    #[test]
    fn aggregate_nesting_uses_the_deterministic_depth_budget() {
        let nested = |depth: usize| {
            let mut value = CanonicalConst {
                type_identity: "Boolean".into(),
                value: ConstValue::Boolean(true),
            };
            for _ in 0..depth {
                value = CanonicalConst {
                    type_identity: "Box".into(),
                    value: ConstValue::Aggregate {
                        type_identity: "Box".into(),
                        fields: vec![("value".into(), value)],
                    },
                };
            }
            value
        };

        nested(64)
            .charge_aggregate_depth(&mut crate::type_budget::TypeComputationBudget::default())
            .unwrap();
        let error = nested(65)
            .charge_aggregate_depth(&mut crate::type_budget::TypeComputationBudget::default())
            .unwrap_err();
        assert_eq!(error.kind, crate::type_budget::BudgetKind::AggregateDepth);
        assert_eq!(error.limit, 64);
        assert_eq!(error.attempted, 65);
    }

    #[test]
    fn bounded_encoded_size_matches_encoding_and_stops_before_oversize_allocation() {
        let value = CanonicalConst {
            type_identity: "pkg::Record".into(),
            value: ConstValue::Aggregate {
                type_identity: "pkg::Record".into(),
                fields: vec![(
                    "message".into(),
                    constant(ConstValue::String("hello".into())),
                )],
            },
        };
        assert_eq!(
            value.encoded_len_limited(1024),
            Some(value.encode().len() as u64)
        );
        assert_eq!(
            constant(ConstValue::String("x".repeat(4096))).encoded_len_limited(1024),
            None
        );
    }

    #[test]
    fn immutable_value_verifier_rejects_nominal_mutable_aggregate_identity() {
        let mutable_class = CanonicalConst {
            type_identity: "pkg::MutableBox".into(),
            value: ConstValue::Aggregate {
                type_identity: "pkg::MutableBox".into(),
                fields: vec![(
                    "value".into(),
                    constant(ConstValue::Integer {
                        width: IntWidth::I32,
                        value: 1,
                    }),
                )],
            },
        };
        assert!(
            mutable_class
                .verify_immutable_value()
                .unwrap_err()
                .contains("verified immutable object")
        );

        let immutable_object = CanonicalConst {
            type_identity: "Object<{value: Int32}>".into(),
            value: ConstValue::Aggregate {
                type_identity: "Object<{value: Int32}>".into(),
                fields: vec![(
                    "value".into(),
                    constant(ConstValue::Integer {
                        width: IntWidth::I32,
                        value: 1,
                    }),
                )],
            },
        };
        assert_eq!(immutable_object.verify_immutable_value(), Ok(()));
    }

    #[test]
    fn package_constant_metadata_round_trips_and_rejects_incompatible_versions() {
        let values = [
            constant(ConstValue::Boolean(true)),
            constant(ConstValue::TimeZone {
                identifier: "America/Santiago".into(),
                rule_data: "2025b".into(),
            }),
        ];
        let metadata = PackageConstMetadata::from_values(&values);
        let encoded = metadata.encode();
        assert_eq!(PackageConstMetadata::decode(&encoded).unwrap(), metadata);

        let mut stale = encoded.clone();
        stale[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
        let error = PackageConstMetadata::decode(&stale).unwrap_err();
        assert_eq!(error, PackageMetadataError::UnsupportedVersion(u16::MAX));
        assert!(error.diagnostic_message().contains("rebuild this package"));
        assert_eq!(
            PackageConstMetadata::decode(&encoded[..encoded.len() - 1]),
            Err(PackageMetadataError::Truncated)
        );
    }
}
