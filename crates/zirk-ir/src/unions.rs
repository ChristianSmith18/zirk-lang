//! Canonical, versioned union type metadata for portable IR packages.

use crate::{AtomicKind, FloatWidth, IntWidth, IrType, Nullable};
use sha2::{Digest, Sha256};

pub const UNION_METADATA_SCHEMA_VERSION: u16 = 2;
/// Major schema version for the portable IR envelope used in union identity.
pub const PORTABLE_IR_SCHEMA_MAJOR: u16 = 1;
const MAGIC: &[u8; 4] = b"ZKUN";
const PACKAGE_MAGIC: &[u8; 4] = b"ZKPM";
const LAYOUT_MAGIC: &[u8; 4] = b"ZKUL";
pub const UNION_PACKAGE_METADATA_SCHEMA_VERSION: u16 = 1;
pub const UNION_LAYOUT_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;
/// The first native release keeps union payloads inline and rejects any
/// aligned payload larger than this limit; there is no implicit boxed path.
pub const MAX_INLINE_UNION_PAYLOAD_BYTES: u64 = 4096;
const MAX_ALTERNATIVES: usize = 256;
const MAX_SOURCE_ALIASES: usize = 256;
const MAX_PACKAGE_UNIONS: usize = 65_536;
const MAX_CANONICAL_KEY_BYTES: usize = 1_048_576;
const MAX_ALIAS_NAME_BYTES: usize = 4096;
const MAX_REFERENCE_PATHS: usize = 4096;
const MAX_REFERENCE_PATH_DEPTH: usize = 64;

/// Canonical tagged-union layout. Alternatives are ordered by canonical,
/// ID-free type encodings; this order defines the tag assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionTypeLayout {
    pub alternatives: Vec<IrType>,
    /// Canonical, ID-free type encodings in the same order as `alternatives`.
    pub canonical_alternatives: Vec<Vec<u8>>,
    /// Optional module-local flattened enum layout used by the current
    /// interpreter/native bridge. It is serialized but excluded from identity.
    pub representation_layout: Option<u32>,
    /// Sorted source aliases that name this canonical union. Diagnostic-only;
    /// aliases do not alter type identity or the ABI fingerprint.
    pub source_aliases: Vec<String>,
    pub fingerprint: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionMetadataError {
    InvalidMagic,
    UnsupportedSchema(u16),
    Truncated,
    TrailingBytes,
    TooManyAlternatives,
    NonCanonicalAlternatives,
    FingerprintMismatch,
    UnsupportedType,
    InvalidCanonicalIdentity,
    CanonicalIdentityCollision,
    TooManySourceAliases,
    InvalidSourceAlias,
    UnsupportedPackageSchema(u16),
    UnsupportedIrSchema(u16),
    MissingPackageUnion(String),
    ConflictingPackageUnion(String),
    PackageUnionFingerprintMismatch(String),
    LayoutOverflow,
    UnsupportedLayoutSchema(u16),
    InvalidReferenceMap,
    TooManyReferencePaths,
    InlinePayloadTooLarge(u64),
}

impl std::fmt::Display for UnionMetadataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid portable union metadata header"),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "portable union metadata schema {version} is unsupported; rebuild the package with this Zirk compiler"
            ),
            Self::Truncated => formatter.write_str("portable union metadata is truncated"),
            Self::TrailingBytes => {
                formatter.write_str("portable union metadata has trailing bytes")
            }
            Self::TooManyAlternatives => {
                formatter.write_str("portable union has an invalid number of alternatives")
            }
            Self::NonCanonicalAlternatives => {
                formatter.write_str("portable union alternatives are not canonical")
            }
            Self::FingerprintMismatch => formatter.write_str(
                "portable union fingerprint does not match its alternatives; rebuild the package",
            ),
            Self::UnsupportedType => {
                formatter.write_str("portable union contains an unsupported IR type")
            }
            Self::InvalidCanonicalIdentity => {
                formatter.write_str("portable union contains an invalid canonical type identity")
            }
            Self::CanonicalIdentityCollision => formatter.write_str(
                "package union identity conflicts with the linked type layout; rebuild the package",
            ),
            Self::TooManySourceAliases => {
                formatter.write_str("portable union has too many source aliases")
            }
            Self::InvalidSourceAlias => {
                formatter.write_str("portable union contains invalid source alias metadata")
            }
            Self::UnsupportedPackageSchema(version) => write!(
                formatter,
                "union package metadata schema {version} is unsupported; rebuild the package"
            ),
            Self::UnsupportedIrSchema(version) => write!(
                formatter,
                "package uses portable IR schema {version}; rebuild it with this Zirk compiler"
            ),
            Self::MissingPackageUnion(alias) => write!(
                formatter,
                "package does not provide required union `{alias}`; rebuild the dependent package"
            ),
            Self::ConflictingPackageUnion(alias) => write!(
                formatter,
                "package exports conflicting ABI fingerprints for union `{alias}`; rebuild the package"
            ),
            Self::PackageUnionFingerprintMismatch(alias) => write!(
                formatter,
                "package union `{alias}` has an incompatible fingerprint; rebuild the package"
            ),
            Self::LayoutOverflow => {
                formatter.write_str("union layout size or alignment overflowed")
            }
            Self::UnsupportedLayoutSchema(version) => write!(
                formatter,
                "union layout descriptor schema {version} is unsupported; rebuild the package"
            ),
            Self::InvalidReferenceMap => {
                formatter.write_str("union layout descriptor has an invalid reference map")
            }
            Self::TooManyReferencePaths => {
                formatter.write_str("union layout descriptor has too many reference paths")
            }
            Self::InlinePayloadTooLarge(size_bytes) => write!(
                formatter,
                "union payload is {size_bytes} bytes, exceeding the {MAX_INLINE_UNION_PAYLOAD_BYTES}-byte inline limit"
            ),
        }
    }
}

impl std::error::Error for UnionMetadataError {}

/// Union ABI metadata exported by a compiled package. Package loaders validate
/// this envelope before linking code that exposes union-typed callables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionPackageMetadata {
    pub ir_schema_major: u16,
    /// Exported names are paired with canonical ABI fingerprints. No local
    /// IR type IDs or enum representation IDs cross this package boundary.
    pub unions: Vec<UnionAbiExport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionAbiExport {
    pub alias: String,
    pub fingerprint: [u8; 32],
}

/// Target-independent physical contract for an inline tagged union. Sizes
/// and alignments are expressed in bits; pointer-bearing layouts use Zirk's
/// supported 64-bit target ABI. Reference paths are field-index paths inside
/// the payload for the tagged alternative, never byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionLayoutDescriptor {
    pub tag_bits: u16,
    pub size_bits: u64,
    pub alignment_bits: u16,
    pub fingerprint: [u8; 32],
    pub alternatives: Vec<UnionAlternativeDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionAlternativeDescriptor {
    pub tag: u32,
    pub payload_size_bits: u64,
    pub payload_alignment_bits: u16,
    /// Paths are specific to this active alternative. Empty path means the
    /// payload itself is a managed reference; nested records add field IDs.
    pub reference_paths: Vec<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeShape {
    size_bits: u64,
    alignment_bits: u16,
}

impl UnionPackageMetadata {
    pub fn current(unions: Vec<UnionTypeLayout>) -> Result<Self, UnionMetadataError> {
        let mut metadata = Self {
            ir_schema_major: PORTABLE_IR_SCHEMA_MAJOR,
            unions: unions
                .iter()
                .flat_map(|union| {
                    union.source_aliases.iter().map(|alias| UnionAbiExport {
                        alias: alias.clone(),
                        fingerprint: union.fingerprint,
                    })
                })
                .collect(),
        };
        metadata.unions.sort_by(|left, right| {
            (&left.alias, left.fingerprint).cmp(&(&right.alias, right.fingerprint))
        });
        metadata.unions.dedup();
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn encode(&self) -> Result<Vec<u8>, UnionMetadataError> {
        self.validate()?;
        let mut bytes = PACKAGE_MAGIC.to_vec();
        bytes.extend_from_slice(&UNION_PACKAGE_METADATA_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.ir_schema_major.to_le_bytes());
        let count = u32::try_from(self.unions.len())
            .map_err(|_| UnionMetadataError::TooManyAlternatives)?;
        bytes.extend_from_slice(&count.to_le_bytes());
        for union in &self.unions {
            bytes.extend_from_slice(&(union.alias.len() as u16).to_le_bytes());
            bytes.extend_from_slice(union.alias.as_bytes());
            bytes.extend_from_slice(&union.fingerprint);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, UnionMetadataError> {
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(4)? != PACKAGE_MAGIC {
            return Err(UnionMetadataError::InvalidMagic);
        }
        let schema = u16::from_le_bytes(reader.array()?);
        if schema != UNION_PACKAGE_METADATA_SCHEMA_VERSION {
            return Err(UnionMetadataError::UnsupportedPackageSchema(schema));
        }
        let ir_schema_major = u16::from_le_bytes(reader.array()?);
        if ir_schema_major != PORTABLE_IR_SCHEMA_MAJOR {
            return Err(UnionMetadataError::UnsupportedIrSchema(ir_schema_major));
        }
        let count = usize::try_from(reader.u32()?).map_err(|_| UnionMetadataError::Truncated)?;
        if count > MAX_PACKAGE_UNIONS {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        let mut unions = Vec::with_capacity(count);
        for _ in 0..count {
            let alias_length = u16::from_le_bytes(reader.array()?) as usize;
            if alias_length == 0 || alias_length > MAX_ALIAS_NAME_BYTES {
                return Err(UnionMetadataError::InvalidSourceAlias);
            }
            let alias = std::str::from_utf8(reader.take(alias_length)?)
                .map_err(|_| UnionMetadataError::InvalidSourceAlias)?
                .to_string();
            let fingerprint = reader.array()?;
            unions.push(UnionAbiExport { alias, fingerprint });
        }
        if reader.offset != bytes.len() {
            return Err(UnionMetadataError::TrailingBytes);
        }
        let metadata = Self {
            ir_schema_major,
            unions,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    /// Checks one imported ABI alias before the linker accepts a package.
    pub fn require_union(
        &self,
        alias: &str,
        expected_fingerprint: &[u8; 32],
    ) -> Result<(), UnionMetadataError> {
        let provided = self
            .unions
            .iter()
            .filter(|union| union.alias == alias)
            .collect::<Vec<_>>();
        if provided.is_empty() {
            return Err(UnionMetadataError::MissingPackageUnion(alias.into()));
        }
        if provided
            .iter()
            .any(|union| union.fingerprint != provided[0].fingerprint)
        {
            return Err(UnionMetadataError::ConflictingPackageUnion(alias.into()));
        }
        if &provided[0].fingerprint != expected_fingerprint {
            return Err(UnionMetadataError::PackageUnionFingerprintMismatch(
                alias.into(),
            ));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), UnionMetadataError> {
        if self.ir_schema_major != PORTABLE_IR_SCHEMA_MAJOR {
            return Err(UnionMetadataError::UnsupportedIrSchema(
                self.ir_schema_major,
            ));
        }
        if self.unions.len() > MAX_PACKAGE_UNIONS {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        if let Some(pair) = self
            .unions
            .windows(2)
            .find(|pair| pair[0].alias == pair[1].alias)
        {
            return Err(UnionMetadataError::ConflictingPackageUnion(
                pair[0].alias.clone(),
            ));
        }
        if self
            .unions
            .iter()
            .any(|union| union.alias.is_empty() || union.alias.len() > MAX_ALIAS_NAME_BYTES)
            || self.unions.windows(2).any(|pair| {
                (pair[0].alias.as_str(), pair[0].fingerprint)
                    >= (pair[1].alias.as_str(), pair[1].fingerprint)
            })
        {
            return Err(UnionMetadataError::InvalidSourceAlias);
        }
        Ok(())
    }
}

/// Validates a package's exported union ABI against imports before its code is
/// linked. The package envelope is decoded first, so stale package/IR schemas
/// and corrupted fingerprints fail closed with rebuild guidance.
pub fn validate_package_union_imports(
    package_metadata: &[u8],
    required_unions: &[UnionTypeLayout],
) -> Result<(), UnionMetadataError> {
    let package = UnionPackageMetadata::decode(package_metadata)?;
    for required in required_unions {
        required.validate()?;
        if required.source_aliases.is_empty() {
            if !package
                .unions
                .iter()
                .any(|export| export.fingerprint == required.fingerprint)
            {
                return Err(UnionMetadataError::PackageUnionFingerprintMismatch(
                    "<anonymous union>".into(),
                ));
            }
        } else {
            for alias in &required.source_aliases {
                package.require_union(alias, &required.fingerprint)?;
            }
        }
    }
    Ok(())
}

impl UnionLayoutDescriptor {
    pub fn derive(module: &crate::Module, union_id: u32) -> Result<Self, UnionMetadataError> {
        let layout = module
            .union_types
            .get(union_id as usize)
            .ok_or(UnionMetadataError::UnsupportedType)?;
        layout.validate()?;
        let mut alternatives = Vec::with_capacity(layout.alternatives.len());
        let mut max_payload_size = 0u64;
        let mut max_payload_alignment = 8u16;
        for (tag, payload) in layout.alternatives.iter().copied().enumerate() {
            let mut seen_unions = std::collections::HashSet::new();
            let (shape, references) = type_shape(module, payload, &mut seen_unions)?;
            max_payload_size = max_payload_size.max(shape.size_bits);
            max_payload_alignment = max_payload_alignment.max(shape.alignment_bits);
            let mut reference_paths = references
                .into_iter()
                .map(|path| {
                    let mut active_payload_path = Vec::with_capacity(path.len() + 1);
                    active_payload_path.push(1);
                    active_payload_path.extend(path);
                    active_payload_path
                })
                .collect::<Vec<_>>();
            reference_paths.sort();
            reference_paths.dedup();
            alternatives.push(UnionAlternativeDescriptor {
                tag: u32::try_from(tag).map_err(|_| UnionMetadataError::TooManyAlternatives)?,
                payload_size_bits: shape.size_bits,
                payload_alignment_bits: shape.alignment_bits,
                reference_paths,
            });
        }
        let payload_bytes = max_payload_size.div_ceil(8);
        if payload_bytes > MAX_INLINE_UNION_PAYLOAD_BYTES {
            return Err(UnionMetadataError::InlinePayloadTooLarge(payload_bytes));
        }
        let alignment_bits = 32u16.max(max_payload_alignment);
        let payload_offset = align_up(32, max_payload_alignment)?;
        let size_bits = align_up(
            payload_offset
                .checked_add(max_payload_size)
                .ok_or(UnionMetadataError::LayoutOverflow)?,
            alignment_bits,
        )?;
        let descriptor = Self {
            tag_bits: 32,
            size_bits,
            alignment_bits,
            fingerprint: layout.fingerprint,
            alternatives,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), UnionMetadataError> {
        let mut max_payload_size = 0u64;
        let mut max_payload_alignment = 8u16;
        if self.tag_bits != 32
            || self.alternatives.len() < 2
            || self.alternatives.len() > MAX_ALTERNATIVES
            || self.alignment_bits == 0
            || !self.alignment_bits.is_power_of_two()
            || self.size_bits % u64::from(self.alignment_bits) != 0
            || self
                .alternatives
                .iter()
                .enumerate()
                .any(|(index, alternative)| {
                    alternative.tag != index as u32
                        || alternative.payload_alignment_bits == 0
                        || !alternative.payload_alignment_bits.is_power_of_two()
                        || alternative.payload_size_bits
                            % u64::from(alternative.payload_alignment_bits)
                            != 0
                        || alternative.reference_paths.len() > MAX_REFERENCE_PATHS
                        || alternative
                            .reference_paths
                            .iter()
                            .any(|path| path.is_empty() || path.len() > MAX_REFERENCE_PATH_DEPTH)
                        || alternative
                            .reference_paths
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                })
        {
            return Err(UnionMetadataError::InvalidCanonicalIdentity);
        }
        for alternative in &self.alternatives {
            max_payload_size = max_payload_size.max(alternative.payload_size_bits);
            max_payload_alignment = max_payload_alignment.max(alternative.payload_alignment_bits);
        }
        let payload_bytes = max_payload_size.div_ceil(8);
        if payload_bytes > MAX_INLINE_UNION_PAYLOAD_BYTES {
            return Err(UnionMetadataError::InlinePayloadTooLarge(payload_bytes));
        }
        let expected_alignment = 32u16.max(max_payload_alignment);
        let expected_payload_offset = align_up(32, max_payload_alignment)?;
        let expected_size = align_up(
            expected_payload_offset
                .checked_add(max_payload_size)
                .ok_or(UnionMetadataError::LayoutOverflow)?,
            expected_alignment,
        )?;
        if self.alignment_bits != expected_alignment || self.size_bits != expected_size {
            return Err(UnionMetadataError::InvalidCanonicalIdentity);
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, UnionMetadataError> {
        self.validate()?;
        let mut bytes = LAYOUT_MAGIC.to_vec();
        bytes.extend_from_slice(&UNION_LAYOUT_DESCRIPTOR_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.tag_bits.to_le_bytes());
        bytes.extend_from_slice(&self.alignment_bits.to_le_bytes());
        bytes.extend_from_slice(&self.size_bits.to_le_bytes());
        bytes.extend_from_slice(&self.fingerprint);
        bytes.extend_from_slice(&(self.alternatives.len() as u16).to_le_bytes());
        for alternative in &self.alternatives {
            bytes.extend_from_slice(&alternative.tag.to_le_bytes());
            bytes.extend_from_slice(&alternative.payload_size_bits.to_le_bytes());
            bytes.extend_from_slice(&alternative.payload_alignment_bits.to_le_bytes());
            bytes.extend_from_slice(&(alternative.reference_paths.len() as u16).to_le_bytes());
            for path in &alternative.reference_paths {
                bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
                for field in path {
                    bytes.extend_from_slice(&field.to_le_bytes());
                }
            }
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, UnionMetadataError> {
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(4)? != LAYOUT_MAGIC {
            return Err(UnionMetadataError::InvalidMagic);
        }
        let schema = u16::from_le_bytes(reader.array()?);
        if schema != UNION_LAYOUT_DESCRIPTOR_SCHEMA_VERSION {
            return Err(UnionMetadataError::UnsupportedLayoutSchema(schema));
        }
        let tag_bits = u16::from_le_bytes(reader.array()?);
        let alignment_bits = u16::from_le_bytes(reader.array()?);
        let size_bits = reader.u64()?;
        let fingerprint = reader.array()?;
        let count = u16::from_le_bytes(reader.array()?) as usize;
        if !(2..=MAX_ALTERNATIVES).contains(&count) {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        let mut alternatives = Vec::with_capacity(count);
        for _ in 0..count {
            let tag = reader.u32()?;
            let payload_size_bits = reader.u64()?;
            let payload_alignment_bits = u16::from_le_bytes(reader.array()?);
            let path_count = u16::from_le_bytes(reader.array()?) as usize;
            if path_count > MAX_REFERENCE_PATHS {
                return Err(UnionMetadataError::TooManyReferencePaths);
            }
            let mut reference_paths = Vec::with_capacity(path_count);
            for _ in 0..path_count {
                let depth = u16::from_le_bytes(reader.array()?) as usize;
                if depth == 0 || depth > MAX_REFERENCE_PATH_DEPTH {
                    return Err(UnionMetadataError::InvalidReferenceMap);
                }
                let mut path = Vec::with_capacity(depth);
                for _ in 0..depth {
                    path.push(reader.u32()?);
                }
                reference_paths.push(path);
            }
            alternatives.push(UnionAlternativeDescriptor {
                tag,
                payload_size_bits,
                payload_alignment_bits,
                reference_paths,
            });
        }
        if reader.offset != bytes.len() {
            return Err(UnionMetadataError::TrailingBytes);
        }
        let descriptor = Self {
            tag_bits,
            size_bits,
            alignment_bits,
            fingerprint,
            alternatives,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }
}

fn type_shape(
    module: &crate::Module,
    ty: IrType,
    seen_unions: &mut std::collections::HashSet<u32>,
) -> Result<(TypeShape, Vec<Vec<u32>>), UnionMetadataError> {
    let scalar = |size_bits: u64, alignment_bits: u16| {
        Ok((
            TypeShape {
                size_bits,
                alignment_bits,
            },
            Vec::new(),
        ))
    };
    let pointer = |managed: bool| {
        Ok((
            TypeShape {
                size_bits: 64,
                alignment_bits: 64,
            },
            if managed {
                vec![Vec::new()]
            } else {
                Vec::new()
            },
        ))
    };
    let u32_shape = |size_bits: u64| -> Result<(TypeShape, Vec<Vec<u32>>), UnionMetadataError> {
        scalar(size_bits, natural_alignment(size_bits))
    };
    match ty {
        IrType::Void | IrType::Never => scalar(0, 8),
        IrType::Int(width) => u32_shape(u64::from(width.bits())),
        IrType::Float(width) => u32_shape(u64::from(width.bits())),
        IrType::Boolean => scalar(8, 8),
        IrType::Decimal => scalar(256, 128),
        IrType::String | IrType::Char => pointer(true),
        IrType::Regex => pointer(false),
        IrType::Object(_)
        | IrType::Contract(_)
        | IrType::ObjectAny
        | IrType::Dynamic
        | IrType::Weak(_)
        | IrType::Dependent(_)
        | IrType::Pin(_)
        | IrType::Array(_)
        | IrType::List(_)
        | IrType::Channel(_)
        | IrType::Atomic(_)
        | IrType::Mutex(_)
        | IrType::Range
        | IrType::Map(_)
        | IrType::Set(_) => pointer(true),
        IrType::Pointer(_) | IrType::Job | IrType::JournalHandle | IrType::BorrowedView { .. } => {
            pointer(false)
        }
        IrType::NativeSlice(_) | IrType::NativeSliceMut(_) => scalar(128, 64),
        IrType::Callable(_) => Ok((
            TypeShape {
                size_bits: 192,
                alignment_bits: 64,
            },
            vec![vec![1]],
        )),
        IrType::Closure(id) => {
            let closure = module
                .closures
                .get(id as usize)
                .ok_or(UnionMetadataError::UnsupportedType)?;
            let mut fields = vec![IrType::Pointer(0)];
            fields.extend(closure.captures.iter().copied());
            aggregate_shape(module, &fields, seen_unions)
        }
        IrType::Value(id) => {
            let value = module
                .values
                .get(id as usize)
                .ok_or(UnionMetadataError::UnsupportedType)?;
            aggregate_shape(
                module,
                &value
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>(),
                seen_unions,
            )
        }
        IrType::Enum(id) => {
            let enumeration = module
                .enums
                .get(id as usize)
                .ok_or(UnionMetadataError::UnsupportedType)?;
            let mut fields = vec![IrType::Int(IntWidth::I32)];
            fields.extend(enumeration.fields.iter().map(|field| field.ty));
            aggregate_shape(module, &fields, seen_unions)
        }
        IrType::Union(id) => {
            let (canonical_union_id, _) = module
                .union_type_for_representation(id)
                .ok_or(UnionMetadataError::UnsupportedType)?;
            if !seen_unions.insert(canonical_union_id) {
                return Err(UnionMetadataError::UnsupportedType);
            }
            let descriptor = UnionLayoutDescriptor::derive(module, canonical_union_id)?;
            seen_unions.remove(&canonical_union_id);
            let references = descriptor
                .alternatives
                .iter()
                .flat_map(|alternative| alternative.reference_paths.iter().cloned())
                .collect();
            Ok((
                TypeShape {
                    size_bits: descriptor.size_bits,
                    alignment_bits: descriptor.alignment_bits,
                },
                references,
            ))
        }
        IrType::Nullable(nullable) => {
            let inner = nullable.inner();
            aggregate_shape(module, &[IrType::Boolean, inner], seen_unions)
        }
    }
}

fn aggregate_shape(
    module: &crate::Module,
    fields: &[IrType],
    seen_unions: &mut std::collections::HashSet<u32>,
) -> Result<(TypeShape, Vec<Vec<u32>>), UnionMetadataError> {
    let mut offset = 0u64;
    let mut alignment = 8u16;
    let mut references = Vec::new();
    for (index, field) in fields.iter().copied().enumerate() {
        let (shape, field_references) = type_shape(module, field, seen_unions)?;
        alignment = alignment.max(shape.alignment_bits);
        offset = align_up(offset, shape.alignment_bits)?;
        offset = offset
            .checked_add(shape.size_bits)
            .ok_or(UnionMetadataError::LayoutOverflow)?;
        for mut path in field_references {
            path.insert(0, index as u32);
            references.push(path);
        }
    }
    Ok((
        TypeShape {
            size_bits: align_up(offset, alignment)?,
            alignment_bits: alignment,
        },
        references,
    ))
}

fn natural_alignment(size_bits: u64) -> u16 {
    let bytes = size_bits.div_ceil(8).max(1);
    bytes.next_power_of_two().min(128) as u16 * 8
}

fn align_up(value: u64, alignment: u16) -> Result<u64, UnionMetadataError> {
    let alignment = u64::from(alignment);
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(UnionMetadataError::LayoutOverflow)
    }
}

impl UnionTypeLayout {
    pub fn new(alternatives: Vec<IrType>) -> Result<Self, UnionMetadataError> {
        let canonical = alternatives
            .iter()
            .copied()
            .map(stable_builtin_key)
            .collect::<Result<Vec<_>, _>>()?;
        Self::with_canonical_alternatives(alternatives, canonical)
    }

    /// Creates a portable layout from IR references and canonical type keys.
    /// Nominal and composite types must use this constructor so local layout
    /// IDs never affect ordering or package fingerprints.
    pub fn with_canonical_alternatives(
        alternatives: Vec<IrType>,
        canonical_alternatives: Vec<Vec<u8>>,
    ) -> Result<Self, UnionMetadataError> {
        if alternatives.len() < 2
            || alternatives.len() > MAX_ALTERNATIVES
            || alternatives.len() != canonical_alternatives.len()
        {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        if canonical_alternatives
            .iter()
            .any(|key| key.is_empty() || key.len() > MAX_CANONICAL_KEY_BYTES)
        {
            return Err(UnionMetadataError::InvalidCanonicalIdentity);
        }
        let mut pairs = alternatives
            .into_iter()
            .zip(canonical_alternatives)
            .map(|(ty, key)| Ok((key, type_key(ty)?, ty)))
            .collect::<Result<Vec<_>, UnionMetadataError>>()?;
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        if pairs.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(UnionMetadataError::NonCanonicalAlternatives);
        }
        let canonical_alternatives = pairs
            .iter()
            .map(|(key, _, _)| key.clone())
            .collect::<Vec<_>>();
        let alternatives = pairs.into_iter().map(|(_, _, ty)| ty).collect::<Vec<_>>();
        let fingerprint = fingerprint(&canonical_alternatives);
        Ok(Self {
            alternatives,
            canonical_alternatives,
            representation_layout: None,
            source_aliases: Vec::new(),
            fingerprint,
        })
    }

    pub fn with_representation_layout(mut self, layout_id: u32) -> Self {
        self.representation_layout = Some(layout_id);
        self
    }

    pub fn with_source_aliases(mut self, aliases: Vec<String>) -> Self {
        self.source_aliases = aliases;
        self.source_aliases.sort();
        self.source_aliases.dedup();
        self
    }

    pub fn encode(&self) -> Result<Vec<u8>, UnionMetadataError> {
        self.validate()?;
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&UNION_METADATA_SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.alternatives.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.representation_layout.unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&(self.source_aliases.len() as u16).to_le_bytes());
        for alias in &self.source_aliases {
            out.extend_from_slice(&(alias.len() as u16).to_le_bytes());
            out.extend_from_slice(alias.as_bytes());
        }
        for (ty, canonical) in self.alternatives.iter().zip(&self.canonical_alternatives) {
            let bytes = type_key(*ty)?;
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&bytes);
            out.extend_from_slice(&(canonical.len() as u32).to_le_bytes());
            out.extend_from_slice(canonical);
        }
        out.extend_from_slice(&self.fingerprint);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, UnionMetadataError> {
        let mut r = Reader { bytes, offset: 0 };
        if r.take(4)? != MAGIC {
            return Err(UnionMetadataError::InvalidMagic);
        }
        let version = u16::from_le_bytes(r.array()?);
        if version != UNION_METADATA_SCHEMA_VERSION {
            return Err(UnionMetadataError::UnsupportedSchema(version));
        }
        let count = u16::from_le_bytes(r.array()?) as usize;
        if !(2..=MAX_ALTERNATIVES).contains(&count) {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        let representation = r.u32()?;
        let alias_count = u16::from_le_bytes(r.array()?) as usize;
        if alias_count > MAX_SOURCE_ALIASES {
            return Err(UnionMetadataError::TooManySourceAliases);
        }
        let mut source_aliases = Vec::with_capacity(alias_count);
        for _ in 0..alias_count {
            let len = u16::from_le_bytes(r.array()?) as usize;
            if len == 0 || len > MAX_ALIAS_NAME_BYTES {
                return Err(UnionMetadataError::InvalidSourceAlias);
            }
            let alias = std::str::from_utf8(r.take(len)?)
                .map_err(|_| UnionMetadataError::InvalidSourceAlias)?
                .to_string();
            if source_aliases
                .last()
                .is_some_and(|previous| previous >= &alias)
            {
                return Err(UnionMetadataError::InvalidSourceAlias);
            }
            source_aliases.push(alias);
        }
        let mut alternatives = Vec::with_capacity(count);
        let mut canonical_alternatives = Vec::with_capacity(count);
        let mut prior: Option<Vec<u8>> = None;
        for _ in 0..count {
            let len = u16::from_le_bytes(r.array()?) as usize;
            let key = r.take(len)?.to_vec();
            let ty = decode_type(&key)?;
            let canonical_len =
                usize::try_from(r.u32()?).map_err(|_| UnionMetadataError::Truncated)?;
            if canonical_len == 0 || canonical_len > MAX_CANONICAL_KEY_BYTES {
                return Err(UnionMetadataError::InvalidCanonicalIdentity);
            }
            let canonical = r.take(canonical_len)?.to_vec();
            if prior.as_ref().is_some_and(|p| p >= &canonical) {
                return Err(UnionMetadataError::NonCanonicalAlternatives);
            }
            prior = Some(canonical.clone());
            alternatives.push(ty);
            canonical_alternatives.push(canonical);
        }
        let encoded_fingerprint: [u8; 32] = r.array()?;
        if r.offset != bytes.len() {
            return Err(UnionMetadataError::TrailingBytes);
        }
        let value = Self {
            alternatives,
            canonical_alternatives,
            representation_layout: (representation != u32::MAX).then_some(representation),
            source_aliases,
            fingerprint: encoded_fingerprint,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), UnionMetadataError> {
        if !(2..=MAX_ALTERNATIVES).contains(&self.alternatives.len()) {
            return Err(UnionMetadataError::TooManyAlternatives);
        }
        if self.alternatives.len() != self.canonical_alternatives.len() {
            return Err(UnionMetadataError::NonCanonicalAlternatives);
        }
        if self.source_aliases.len() > MAX_SOURCE_ALIASES
            || self
                .source_aliases
                .iter()
                .any(|alias| alias.is_empty() || alias.len() > MAX_ALIAS_NAME_BYTES)
            || self
                .source_aliases
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(UnionMetadataError::InvalidSourceAlias);
        }
        if self
            .canonical_alternatives
            .iter()
            .any(|key| key.is_empty() || key.len() > MAX_CANONICAL_KEY_BYTES)
        {
            return Err(UnionMetadataError::InvalidCanonicalIdentity);
        }
        for ty in &self.alternatives {
            let _ = type_key(*ty)?;
        }
        if self
            .canonical_alternatives
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(UnionMetadataError::NonCanonicalAlternatives);
        }
        if fingerprint(&self.canonical_alternatives) != self.fingerprint {
            return Err(UnionMetadataError::FingerprintMismatch);
        }
        Ok(())
    }
}

fn fingerprint(canonical_types: &[Vec<u8>]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"zirk.union.layout.v1\0");
    h.update(PORTABLE_IR_SCHEMA_MAJOR.to_le_bytes());
    for key in canonical_types {
        h.update((key.len() as u32).to_le_bytes());
        h.update(key);
    }
    h.finalize().into()
}

fn stable_builtin_key(ty: IrType) -> Result<Vec<u8>, UnionMetadataError> {
    if matches!(
        ty,
        IrType::Object(_)
            | IrType::Contract(_)
            | IrType::Value(_)
            | IrType::Enum(_)
            | IrType::Closure(_)
            | IrType::Callable(_)
            | IrType::Channel(_)
            | IrType::Mutex(_)
            | IrType::Pointer(_)
            | IrType::Weak(_)
            | IrType::NativeSlice(_)
            | IrType::NativeSliceMut(_)
            | IrType::Dependent(_)
            | IrType::Pin(_)
            | IrType::Array(_)
            | IrType::List(_)
            | IrType::Map(_)
            | IrType::Set(_)
            | IrType::Union(_)
            | IrType::Nullable(
                Nullable::Object(_)
                    | Nullable::Contract(_)
                    | Nullable::Value(_)
                    | Nullable::Enum(_)
                    | Nullable::Array(_)
                    | Nullable::List(_)
                    | Nullable::Map(_)
                    | Nullable::Set(_)
            )
    ) {
        return Err(UnionMetadataError::UnsupportedType);
    }
    type_key(ty)
}

fn type_key(ty: IrType) -> Result<Vec<u8>, UnionMetadataError> {
    let mut out = Vec::new();
    let (tag, id) = match ty {
        IrType::Void => (0, None),
        IrType::Never => (1, None),
        IrType::Int(w) => (2 + int_code(w), None),
        IrType::Float(w) => (16 + float_code(w), None),
        IrType::Decimal => (24, None),
        IrType::Boolean => (25, None),
        IrType::String => (26, None),
        IrType::Regex => (27, None),
        IrType::Char => (28, None),
        IrType::Job => (29, None),
        IrType::ObjectAny => (30, None),
        IrType::Dynamic => (31, None),
        IrType::Range => (32, None),
        IrType::JournalHandle => (33, None),
        IrType::Closure(i) => (40, Some(i)),
        IrType::Callable(i) => (41, Some(i)),
        IrType::Channel(i) => (42, Some(i)),
        IrType::Mutex(i) => (43, Some(i)),
        IrType::Object(i) => (44, Some(i)),
        IrType::Contract(i) => (45, Some(i)),
        IrType::Value(i) => (46, Some(i)),
        IrType::Enum(i) => (47, Some(i)),
        IrType::Pointer(i) => (48, Some(i)),
        IrType::Weak(i) => (49, Some(i)),
        IrType::NativeSlice(i) => (50, Some(i)),
        IrType::NativeSliceMut(i) => (51, Some(i)),
        IrType::Dependent(i) => (52, Some(i)),
        IrType::Pin(i) => (53, Some(i)),
        IrType::Array(i) => (54, Some(i)),
        IrType::List(i) => (55, Some(i)),
        IrType::Map(i) => (56, Some(i)),
        IrType::Set(i) => (57, Some(i)),
        IrType::Union(i) => (58, Some(i)),
        IrType::Atomic(k) => (60 + k as u8, None),
        IrType::Nullable(n) => {
            out.push(70);
            encode_nullable(n, &mut out);
            return Ok(out);
        }
        IrType::BorrowedView { region, mutex_type } => {
            out.push(71);
            out.extend(region.to_le_bytes());
            out.extend(mutex_type.to_le_bytes());
            return Ok(out);
        }
    };
    out.push(tag);
    if let Some(id) = id {
        out.extend(id.to_le_bytes());
    }
    Ok(out)
}

fn encode_nullable(n: Nullable, out: &mut Vec<u8>) {
    match n {
        Nullable::Int(w) => {
            out.push(2);
            out.push(int_code(w));
        }
        Nullable::Float(w) => {
            out.push(16);
            out.push(float_code(w));
        }
        Nullable::Boolean => out.push(25),
        Nullable::String => out.push(26),
        Nullable::Char => out.push(28),
        Nullable::Object(i) => {
            out.push(44);
            out.extend(i.to_le_bytes());
        }
        Nullable::Contract(i) => {
            out.push(45);
            out.extend(i.to_le_bytes());
        }
        Nullable::ObjectAny => out.push(30),
        Nullable::Dynamic => out.push(31),
        Nullable::Value(i) => {
            out.push(46);
            out.extend(i.to_le_bytes());
        }
        Nullable::Enum(i) => {
            out.push(47);
            out.extend(i.to_le_bytes());
        }
        Nullable::Array(i) => {
            out.push(54);
            out.extend(i.to_le_bytes());
        }
        Nullable::List(i) => {
            out.push(55);
            out.extend(i.to_le_bytes());
        }
        Nullable::Map(i) => {
            out.push(56);
            out.extend(i.to_le_bytes());
        }
        Nullable::Set(i) => {
            out.push(57);
            out.extend(i.to_le_bytes());
        }
        Nullable::Atomic(k) => out.push(60 + k as u8),
        Nullable::Union(i) => {
            out.push(58);
            out.extend(i.to_le_bytes());
        }
    }
}

fn decode_type(bytes: &[u8]) -> Result<IrType, UnionMetadataError> {
    let mut r = Reader { bytes, offset: 0 };
    let tag = r.byte()?;
    let ty = match tag {
        0 => IrType::Void,
        1 => IrType::Never,
        2..=11 => IrType::Int(int_from(tag - 2)?),
        16..=19 => IrType::Float(float_from(tag - 16)?),
        24 => IrType::Decimal,
        25 => IrType::Boolean,
        26 => IrType::String,
        27 => IrType::Regex,
        28 => IrType::Char,
        29 => IrType::Job,
        30 => IrType::ObjectAny,
        31 => IrType::Dynamic,
        32 => IrType::Range,
        33 => IrType::JournalHandle,
        40 => IrType::Closure(r.u32()?),
        41 => IrType::Callable(r.u32()?),
        42 => IrType::Channel(r.u32()?),
        43 => IrType::Mutex(r.u32()?),
        44 => IrType::Object(r.u32()?),
        45 => IrType::Contract(r.u32()?),
        46 => IrType::Value(r.u32()?),
        47 => IrType::Enum(r.u32()?),
        48 => IrType::Pointer(r.u32()?),
        49 => IrType::Weak(r.u32()?),
        50 => IrType::NativeSlice(r.u32()?),
        51 => IrType::NativeSliceMut(r.u32()?),
        52 => IrType::Dependent(r.u32()?),
        53 => IrType::Pin(r.u32()?),
        54 => IrType::Array(r.u32()?),
        55 => IrType::List(r.u32()?),
        56 => IrType::Map(r.u32()?),
        57 => IrType::Set(r.u32()?),
        58 => IrType::Union(r.u32()?),
        60..=68 => IrType::Atomic(atomic_from(tag - 60)?),
        70 => {
            let inner = decode_nullable(&mut r)?;
            IrType::Nullable(inner)
        }
        71 => IrType::BorrowedView {
            region: r.u32()?,
            mutex_type: r.u32()?,
        },
        _ => return Err(UnionMetadataError::UnsupportedType),
    };
    if r.offset != bytes.len() {
        return Err(UnionMetadataError::TrailingBytes);
    }
    if type_key(ty)? != bytes {
        return Err(UnionMetadataError::UnsupportedType);
    }
    Ok(ty)
}

fn decode_nullable(r: &mut Reader<'_>) -> Result<Nullable, UnionMetadataError> {
    Ok(match r.byte()? {
        2 => Nullable::Int(int_from(r.byte()?)?),
        16 => Nullable::Float(float_from(r.byte()?)?),
        25 => Nullable::Boolean,
        26 => Nullable::String,
        28 => Nullable::Char,
        30 => Nullable::ObjectAny,
        31 => Nullable::Dynamic,
        44 => Nullable::Object(r.u32()?),
        45 => Nullable::Contract(r.u32()?),
        46 => Nullable::Value(r.u32()?),
        47 => Nullable::Enum(r.u32()?),
        54 => Nullable::Array(r.u32()?),
        55 => Nullable::List(r.u32()?),
        56 => Nullable::Map(r.u32()?),
        57 => Nullable::Set(r.u32()?),
        58 => Nullable::Union(r.u32()?),
        60..=68 => Nullable::Atomic(atomic_from(r.bytes[r.offset - 1] - 60)?),
        _ => return Err(UnionMetadataError::UnsupportedType),
    })
}

const fn int_code(w: IntWidth) -> u8 {
    match w {
        IntWidth::I8 => 0,
        IntWidth::I16 => 1,
        IntWidth::I32 => 2,
        IntWidth::I64 => 3,
        IntWidth::I128 => 4,
        IntWidth::U8 => 5,
        IntWidth::U16 => 6,
        IntWidth::U32 => 7,
        IntWidth::U64 => 8,
        IntWidth::U128 => 9,
    }
}
const fn float_code(w: FloatWidth) -> u8 {
    match w {
        FloatWidth::F16 => 0,
        FloatWidth::F32 => 1,
        FloatWidth::F64 => 2,
        FloatWidth::F128 => 3,
    }
}
fn int_from(v: u8) -> Result<IntWidth, UnionMetadataError> {
    Ok(match v {
        0 => IntWidth::I8,
        1 => IntWidth::I16,
        2 => IntWidth::I32,
        3 => IntWidth::I64,
        4 => IntWidth::I128,
        5 => IntWidth::U8,
        6 => IntWidth::U16,
        7 => IntWidth::U32,
        8 => IntWidth::U64,
        9 => IntWidth::U128,
        _ => return Err(UnionMetadataError::UnsupportedType),
    })
}
fn float_from(v: u8) -> Result<FloatWidth, UnionMetadataError> {
    Ok(match v {
        0 => FloatWidth::F16,
        1 => FloatWidth::F32,
        2 => FloatWidth::F64,
        3 => FloatWidth::F128,
        _ => return Err(UnionMetadataError::UnsupportedType),
    })
}
fn atomic_from(v: u8) -> Result<AtomicKind, UnionMetadataError> {
    Ok(match v {
        0 => AtomicKind::Boolean,
        1 => AtomicKind::I8,
        2 => AtomicKind::U8,
        3 => AtomicKind::I16,
        4 => AtomicKind::U16,
        5 => AtomicKind::I32,
        6 => AtomicKind::U32,
        7 => AtomicKind::I64,
        8 => AtomicKind::U64,
        _ => return Err(UnionMetadataError::UnsupportedType),
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], UnionMetadataError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(UnionMetadataError::Truncated)?;
        let v = self
            .bytes
            .get(self.offset..end)
            .ok_or(UnionMetadataError::Truncated)?;
        self.offset = end;
        Ok(v)
    }
    fn byte(&mut self) -> Result<u8, UnionMetadataError> {
        Ok(self.take(1)?[0])
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], UnionMetadataError> {
        self.take(N)?
            .try_into()
            .map_err(|_| UnionMetadataError::Truncated)
    }
    fn u32(&mut self) -> Result<u32, UnionMetadataError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, UnionMetadataError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_metadata_round_trips_schema_and_canonical_fingerprint() {
        let layout = UnionTypeLayout::with_canonical_alternatives(
            vec![
                IrType::String,
                IrType::Int(IntWidth::I32),
                IrType::Nullable(Nullable::Object(7)),
            ],
            vec![
                b"builtin::String".to_vec(),
                b"builtin::Int32".to_vec(),
                b"pkg::User?".to_vec(),
            ],
        )
        .unwrap();
        let encoded = layout.encode().unwrap();
        assert_eq!(
            u16::from_le_bytes([encoded[4], encoded[5]]),
            UNION_METADATA_SCHEMA_VERSION
        );
        let decoded = UnionTypeLayout::decode(&encoded).unwrap();
        assert_eq!(decoded, layout);
        assert_eq!(decoded.fingerprint, layout.fingerprint);
    }

    #[test]
    fn source_alias_provenance_round_trips_without_changing_union_identity() {
        let plain = UnionTypeLayout::new(vec![IrType::String, IrType::Int(IntWidth::I32)]).unwrap();
        let annotated = plain
            .clone()
            .with_source_aliases(vec!["TextOrNumber".into(), "PublicValue".into()]);
        assert_eq!(plain.fingerprint, annotated.fingerprint);
        assert_eq!(
            UnionTypeLayout::decode(&annotated.encode().unwrap()).unwrap(),
            annotated
        );
    }

    #[test]
    fn package_union_metadata_validates_schema_and_required_fingerprints() {
        let layout = UnionTypeLayout::new(vec![IrType::String, IrType::Int(IntWidth::I32)])
            .unwrap()
            .with_source_aliases(vec!["PublicValue".into()]);
        let fingerprint = layout.fingerprint;
        let package_module = crate::Module {
            union_types: vec![layout],
            ..crate::Module::default()
        };
        let package = package_module.union_package_metadata().unwrap();
        let encoded = package.encode().unwrap();
        let decoded = UnionPackageMetadata::decode(&encoded).unwrap();
        assert_eq!(decoded, package);
        assert_eq!(decoded.require_union("PublicValue", &fingerprint), Ok(()));
        let imported_layout =
            UnionTypeLayout::new(vec![IrType::Int(IntWidth::I32), IrType::String])
                .unwrap()
                .with_source_aliases(vec!["PublicValue".into()]);
        assert_eq!(
            validate_package_union_imports(&encoded, &[imported_layout]),
            Ok(())
        );
        assert_eq!(
            decoded.require_union("PublicValue", &[0; 32]),
            Err(UnionMetadataError::PackageUnionFingerprintMismatch(
                "PublicValue".into()
            ))
        );
        let mismatch =
            UnionMetadataError::PackageUnionFingerprintMismatch("PublicValue".into()).to_string();
        assert!(mismatch.contains("rebuild the package"), "{mismatch}");
        let incompatible_layout =
            UnionTypeLayout::new(vec![IrType::Int(IntWidth::I64), IrType::String])
                .unwrap()
                .with_source_aliases(vec!["PublicValue".into()]);
        assert_eq!(
            validate_package_union_imports(&encoded, &[incompatible_layout]),
            Err(UnionMetadataError::PackageUnionFingerprintMismatch(
                "PublicValue".into()
            ))
        );

        let mut wrong_ir_schema = encoded;
        wrong_ir_schema[6..8].copy_from_slice(&99u16.to_le_bytes());
        let error = UnionPackageMetadata::decode(&wrong_ir_schema).unwrap_err();
        assert_eq!(error, UnionMetadataError::UnsupportedIrSchema(99));
        assert!(error.to_string().contains("rebuild it"));
    }

    #[test]
    fn package_metadata_rejects_ambiguous_alias_fingerprints() {
        let package = UnionPackageMetadata {
            ir_schema_major: PORTABLE_IR_SCHEMA_MAJOR,
            unions: vec![
                UnionAbiExport {
                    alias: "PublicValue".into(),
                    fingerprint: [1; 32],
                },
                UnionAbiExport {
                    alias: "PublicValue".into(),
                    fingerprint: [2; 32],
                },
            ],
        };
        let error = package.encode().unwrap_err();
        assert_eq!(
            error,
            UnionMetadataError::ConflictingPackageUnion("PublicValue".into())
        );
        assert!(error.to_string().contains("rebuild the package"));
    }

    #[test]
    fn target_independent_layout_derivation_is_deterministic_and_tag_specific() {
        let layout =
            UnionTypeLayout::new(vec![IrType::String, IrType::Int(IntWidth::I32)]).unwrap();
        let module = crate::Module {
            union_types: vec![layout.clone()],
            ..crate::Module::default()
        };
        let first = module.union_layout_descriptor(0).unwrap();
        let second = module.union_layout_descriptor(0).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            UnionLayoutDescriptor::decode(&first.encode().unwrap()).unwrap(),
            first
        );
        assert_eq!(first.tag_bits, 32);
        assert_eq!(first.alignment_bits, 64);
        assert_eq!(first.size_bits, 128);
        assert_eq!(first.fingerprint, layout.fingerprint);
        for (index, payload) in layout.alternatives.iter().enumerate() {
            let alternative = &first.alternatives[index];
            assert_eq!(alternative.tag, index as u32);
            if *payload == IrType::String {
                assert_eq!(alternative.reference_paths, [vec![1]]);
            } else {
                assert!(alternative.reference_paths.is_empty());
            }
        }
    }

    #[test]
    fn reference_maps_follow_only_the_active_union_alternative() {
        let layout = UnionTypeLayout::with_canonical_alternatives(
            vec![
                IrType::Object(0),
                IrType::ObjectAny,
                IrType::String,
                IrType::Int(IntWidth::I32),
            ],
            vec![
                b"class::Widget".to_vec(),
                b"shape::{ value: String }".to_vec(),
                b"builtin::String".to_vec(),
                b"builtin::Int32".to_vec(),
            ],
        )
        .unwrap();
        let module = crate::Module {
            union_types: vec![layout.clone()],
            ..crate::Module::default()
        };
        let descriptor = module.union_layout_descriptor(0).unwrap();
        for (index, ty) in layout.alternatives.iter().enumerate() {
            assert_eq!(descriptor.alternatives[index].tag, index as u32);
            if matches!(ty, IrType::Object(_) | IrType::ObjectAny) || *ty == IrType::String {
                assert_eq!(descriptor.alternatives[index].reference_paths, [vec![1]]);
            } else {
                assert!(descriptor.alternatives[index].reference_paths.is_empty());
            }
        }
    }

    #[test]
    fn oversized_inline_union_payload_is_rejected_at_the_layout_boundary() {
        let large_value = crate::ValueLayout {
            name: "Large".into(),
            fields: (0..=MAX_INLINE_UNION_PAYLOAD_BYTES / 4)
                .map(|index| crate::ObjectField {
                    name: format!("field_{index}"),
                    ty: IrType::Int(IntWidth::I32),
                })
                .collect(),
        };
        let layout = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Value(0), IrType::Int(IntWidth::I32)],
            vec![
                b"record::Large".to_vec(),
                stable_builtin_key(IrType::Int(IntWidth::I32)).unwrap(),
            ],
        )
        .unwrap();
        let span = zirk_diagnostics::Span::new(4, 8);
        let mut layout = layout;
        layout.representation_layout = Some(0);
        let module = crate::Module {
            values: vec![large_value],
            union_types: vec![layout],
            functions: vec![crate::Function {
                name: "consume".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![crate::Slot {
                    name: "value".into(),
                    ty: IrType::Union(0),
                    span,
                }],
                blocks: vec![],
                entry: crate::BlockId(0),
                span,
                gc_roots: vec![],
            }],
            ..crate::Module::default()
        };

        let error = module.union_layout_descriptor(0).unwrap_err();
        assert!(
            matches!(error, UnionMetadataError::InlinePayloadTooLarge(size)
                if size > MAX_INLINE_UNION_PAYLOAD_BYTES),
            "unexpected oversized union error: {error}"
        );
        assert!(error.to_string().contains("4096-byte inline limit"));
        assert_eq!(module.oversized_union_payloads(), [(0, 4100, span)]);
    }

    #[test]
    fn nullable_union_member_round_trips_with_its_null_capability() {
        let layout = UnionTypeLayout::new(vec![
            IrType::Nullable(Nullable::String),
            IrType::Int(IntWidth::I32),
        ])
        .unwrap();
        let decoded = UnionTypeLayout::decode(&layout.encode().unwrap()).unwrap();
        assert_eq!(decoded.alternatives, layout.alternatives);
        assert_eq!(decoded.fingerprint, layout.fingerprint);
    }

    #[test]
    fn nullable_map_and_set_types_round_trip_in_ir_metadata() {
        for ty in [
            IrType::Nullable(Nullable::Map(3)),
            IrType::Nullable(Nullable::Set(5)),
        ] {
            let encoded = type_key(ty).unwrap();
            assert_eq!(decode_type(&encoded).unwrap(), ty);
        }
        assert_eq!(Nullable::of(IrType::Map(3)), Some(Nullable::Map(3)));
        assert_eq!(Nullable::of(IrType::Set(5)), Some(Nullable::Set(5)));
    }

    #[test]
    fn nullable_union_layout_reference_round_trips_without_entering_identity() {
        let layout = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Nullable(Nullable::Union(7)), IrType::String],
            vec![b"pkg::NestedUnion?".to_vec(), b"builtin::String".to_vec()],
        )
        .unwrap();
        let decoded = UnionTypeLayout::decode(&layout.encode().unwrap()).unwrap();
        assert_eq!(decoded.alternatives, layout.alternatives);
        assert_eq!(decoded.fingerprint, layout.fingerprint);
        assert_eq!(Nullable::of(IrType::Union(7)), Some(Nullable::Union(7)));
    }

    #[test]
    fn union_metadata_rejects_stale_fingerprint_and_schema() {
        let layout = UnionTypeLayout::new(vec![IrType::String, IrType::Boolean]).unwrap();
        let mut encoded = layout.encode().unwrap();
        let last = encoded.len() - 1;
        encoded[last] ^= 1;
        assert_eq!(
            UnionTypeLayout::decode(&encoded),
            Err(UnionMetadataError::FingerprintMismatch)
        );
        let mut encoded = layout.encode().unwrap();
        encoded[4..6].copy_from_slice(&99u16.to_le_bytes());
        assert_eq!(
            UnionTypeLayout::decode(&encoded),
            Err(UnionMetadataError::UnsupportedSchema(99))
        );
        let diagnostic = UnionMetadataError::UnsupportedSchema(99).to_string();
        assert!(diagnostic.contains("rebuild the package"), "{diagnostic}");
    }

    #[test]
    fn nominal_layout_ids_do_not_change_the_canonical_fingerprint() {
        let left = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Object(1), IrType::String],
            vec![b"pkg::User".to_vec(), b"builtin::String".to_vec()],
        )
        .unwrap()
        .with_representation_layout(4);
        let right = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Object(27), IrType::String],
            vec![b"pkg::User".to_vec(), b"builtin::String".to_vec()],
        )
        .unwrap()
        .with_representation_layout(19);
        assert_eq!(left.fingerprint, right.fingerprint);
    }

    #[test]
    fn module_interns_canonical_union_layouts_and_rejects_identity_collisions() {
        let mut module = crate::Module::default();
        let first = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Object(1), IrType::String],
            vec![b"pkg::User".to_vec(), b"builtin::String".to_vec()],
        )
        .unwrap();
        let same = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::String, IrType::Object(1)],
            vec![b"builtin::String".to_vec(), b"pkg::User".to_vec()],
        )
        .unwrap();
        let first = first.with_source_aliases(vec!["PublicValue".into()]);
        let same = same.with_source_aliases(vec!["InternalValue".into()]);
        assert_eq!(module.intern_union_type(first).unwrap(), 0);
        assert_eq!(module.intern_union_type(same).unwrap(), 0);
        assert_eq!(
            module.union_types[0].source_aliases,
            ["InternalValue", "PublicValue"]
        );

        let collision = UnionTypeLayout::with_canonical_alternatives(
            vec![IrType::Object(2), IrType::String],
            vec![b"pkg::User".to_vec(), b"builtin::String".to_vec()],
        )
        .unwrap();
        assert_eq!(
            module.intern_union_type(collision),
            Err(UnionMetadataError::CanonicalIdentityCollision)
        );
    }

    #[test]
    fn one_bridge_representation_cannot_name_two_canonical_unions() {
        let mut module = crate::Module::default();
        let first = UnionTypeLayout::new(vec![IrType::Boolean, IrType::String])
            .unwrap()
            .with_representation_layout(0);
        let second = UnionTypeLayout::new(vec![IrType::Boolean, IrType::Int(IntWidth::I32)])
            .unwrap()
            .with_representation_layout(0);
        module.intern_union_type(first).unwrap();
        assert_eq!(
            module.intern_union_type(second),
            Err(UnionMetadataError::CanonicalIdentityCollision)
        );
    }
}
