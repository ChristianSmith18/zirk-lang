//! Canonical recursive value layouts shared by generic storage and unions.

use crate::{FloatWidth, IntWidth, IrType, Module, UnionMetadataError};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeLayoutKind {
    ZeroSized,
    Integer(IntWidth),
    BinaryFloat(FloatWidth),
    Decimal,
    Boolean,
    ManagedReference,
    RawPointer,
    Aggregate,
    Nullable,
    TaggedUnion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayoutField {
    pub index: u32,
    pub offset_bits: u64,
    pub layout: Box<RuntimeTypeLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTypeLayout {
    pub kind: RuntimeLayoutKind,
    pub size_bits: u64,
    pub alignment_bits: u16,
    /// Possible managed-reference paths. For a tagged union these are
    /// alternative-specific and are represented on each alternative too.
    pub reference_paths: Vec<Vec<u32>>,
    pub fields: Vec<RuntimeLayoutField>,
    pub alternatives: Vec<RuntimeTypeLayout>,
    /// Element, key/value, or payload layouts carried by opaque generic
    /// collection and synchronization handles. These do not occupy bytes in
    /// the handle itself, but are part of its canonical runtime contract.
    pub generic_arguments: Vec<RuntimeTypeLayout>,
    /// Stable over module-local type IDs and source aliases.
    pub fingerprint: [u8; 32],
}

impl Module {
    /// Derives a target-independent storage contract for every storable IR
    /// type. The descriptor preserves field offsets, nested layouts, tagged
    /// alternatives and managed-reference paths.
    pub fn runtime_type_layout(&self, ty: IrType) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        self.derive_runtime_type_layout(ty, &mut HashSet::new())
    }

    fn derive_runtime_type_layout(
        &self,
        ty: IrType,
        active_unions: &mut HashSet<u32>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        let (kind, fields, alternatives) = match ty {
            IrType::Void | IrType::Never => (RuntimeLayoutKind::ZeroSized, Vec::new(), Vec::new()),
            IrType::Int(width) => (RuntimeLayoutKind::Integer(width), Vec::new(), Vec::new()),
            IrType::Float(width) => (
                RuntimeLayoutKind::BinaryFloat(width),
                Vec::new(),
                Vec::new(),
            ),
            IrType::Decimal => (RuntimeLayoutKind::Decimal, Vec::new(), Vec::new()),
            IrType::Boolean => (RuntimeLayoutKind::Boolean, Vec::new(), Vec::new()),
            IrType::Nullable(nullable) => {
                let children = [IrType::Boolean, nullable.inner()];
                (RuntimeLayoutKind::Nullable, children.to_vec(), Vec::new())
            }
            IrType::Value(id) => {
                let value = self
                    .values
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                (
                    RuntimeLayoutKind::Aggregate,
                    value.fields.iter().map(|field| field.ty).collect(),
                    Vec::new(),
                )
            }
            IrType::Enum(id) => {
                let enumeration = self
                    .enums
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                let mut fields = vec![IrType::Int(IntWidth::I32)];
                fields.extend(enumeration.fields.iter().map(|field| field.ty));
                (RuntimeLayoutKind::Aggregate, fields, Vec::new())
            }
            IrType::Union(id) => {
                let (canonical_id, _) = self
                    .union_type_for_representation(id)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                if !active_unions.insert(canonical_id) {
                    return Err(UnionMetadataError::UnsupportedType);
                }
                let layout = self
                    .union_types
                    .get(canonical_id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                let mut alternatives = Vec::with_capacity(layout.alternatives.len());
                for alternative in layout.alternatives.iter().copied() {
                    alternatives.push(self.derive_runtime_type_layout(alternative, active_unions)?);
                }
                active_unions.remove(&canonical_id);
                return self.finish_tagged_union(alternatives);
            }
            IrType::Closure(id) => {
                let closure = self
                    .closures
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                let mut fields = vec![IrType::Pointer(0)];
                fields.extend(closure.captures.iter().copied());
                (RuntimeLayoutKind::Aggregate, fields, Vec::new())
            }
            IrType::Callable(id) => {
                let closure = self
                    .closures
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                let mut fields = vec![IrType::Pointer(0), IrType::Pointer(0)];
                fields.extend(closure.captures.iter().copied());
                (RuntimeLayoutKind::Aggregate, fields, Vec::new())
            }
            IrType::NativeSlice(_) | IrType::NativeSliceMut(_) => {
                let fields = [IrType::Pointer(0), IrType::Int(IntWidth::U64)].to_vec();
                (RuntimeLayoutKind::Aggregate, fields, Vec::new())
            }
            IrType::Map(id) => {
                let (key, value) = *self
                    .map_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![key, value]);
            }
            IrType::Set(id) => {
                let element = *self
                    .set_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![element]);
            }
            IrType::Array(id) => {
                let element = *self
                    .array_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![element]);
            }
            IrType::List(id) => {
                let element = *self
                    .list_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![element]);
            }
            IrType::Channel(id) => {
                let element = *self
                    .channel_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![element]);
            }
            IrType::Atomic(kind) => {
                return self.finish_generic_pointer(
                    RuntimeLayoutKind::ManagedReference,
                    vec![kind.ir_type()],
                );
            }
            IrType::Mutex(id) => {
                let element = *self
                    .mutex_types
                    .get(id as usize)
                    .ok_or(UnionMetadataError::UnsupportedType)?;
                return self
                    .finish_generic_pointer(RuntimeLayoutKind::ManagedReference, vec![element]);
            }
            IrType::String
            | IrType::Char
            | IrType::Object(_)
            | IrType::Contract(_)
            | IrType::ObjectAny
            | IrType::Dynamic
            | IrType::Weak(_)
            | IrType::Dependent(_)
            | IrType::Pin(_)
            | IrType::Range => {
                return self.finish_pointer(RuntimeLayoutKind::ManagedReference, vec![Vec::new()]);
            }
            IrType::Regex
            | IrType::Pointer(_)
            | IrType::Job
            | IrType::JournalHandle
            | IrType::BorrowedView { .. } => {
                return self.finish_pointer(RuntimeLayoutKind::RawPointer, Vec::new());
            }
        };

        let scalar_shape = match ty {
            IrType::Void | IrType::Never => Some((0, 8)),
            IrType::Int(width) => Some((u64::from(width.bits()), natural_alignment(width.bits()))),
            IrType::Float(width) => {
                Some((u64::from(width.bits()), natural_alignment(width.bits())))
            }
            IrType::Decimal => Some((256, 128)),
            IrType::Boolean => Some((8, 8)),
            _ => None,
        };
        if let Some((size, alignment)) = scalar_shape {
            return self.finish(kind, size, alignment, Vec::new(), Vec::new(), Vec::new());
        }

        let mut child_layouts = Vec::with_capacity(fields.len());
        for field in fields {
            child_layouts.push(self.derive_runtime_type_layout(field, active_unions)?);
        }
        self.finish_aggregate(kind, child_layouts, alternatives)
    }

    fn finish_pointer(
        &self,
        kind: RuntimeLayoutKind,
        reference_paths: Vec<Vec<u32>>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        self.finish(kind, 64, 64, reference_paths, Vec::new(), Vec::new())
    }

    fn finish_generic_pointer(
        &self,
        kind: RuntimeLayoutKind,
        arguments: Vec<IrType>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        let mut layout = self.finish_pointer(kind, vec![Vec::new()])?;
        layout.generic_arguments = arguments
            .into_iter()
            .map(|argument| self.derive_runtime_type_layout(argument, &mut HashSet::new()))
            .collect::<Result<_, _>>()?;
        let mut hasher = Sha256::new();
        hasher.update(layout.fingerprint);
        for argument in &layout.generic_arguments {
            hasher.update(argument.fingerprint);
        }
        layout.fingerprint = hasher.finalize().into();
        Ok(layout)
    }

    fn finish_aggregate(
        &self,
        kind: RuntimeLayoutKind,
        children: Vec<RuntimeTypeLayout>,
        alternatives: Vec<RuntimeTypeLayout>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        let mut offset = 0u64;
        let mut alignment = 8u16;
        let mut paths = Vec::new();
        let mut fields = Vec::with_capacity(children.len());
        for (index, child) in children.into_iter().enumerate() {
            alignment = alignment.max(child.alignment_bits);
            offset = align_up(offset, child.alignment_bits)?;
            let field_offset = offset;
            offset = offset
                .checked_add(child.size_bits)
                .ok_or(UnionMetadataError::LayoutOverflow)?;
            for path in &child.reference_paths {
                let mut path = path.clone();
                path.insert(0, index as u32);
                paths.push(path);
            }
            fields.push(RuntimeLayoutField {
                index: index as u32,
                offset_bits: field_offset,
                layout: Box::new(child),
            });
        }
        paths.sort();
        paths.dedup();
        let size = align_up(offset, alignment)?;
        self.finish(kind, size, alignment, paths, fields, alternatives)
    }

    fn finish_tagged_union(
        &self,
        alternatives: Vec<RuntimeTypeLayout>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        let mut paths = Vec::new();
        let mut payload_size = 0u64;
        let mut payload_alignment = 8u16;
        for (tag, alternative) in alternatives.iter().enumerate() {
            payload_size = payload_size.max(alternative.size_bits);
            payload_alignment = payload_alignment.max(alternative.alignment_bits);
            for reference_path in &alternative.reference_paths {
                let mut path = vec![1, tag as u32];
                path.extend(reference_path.iter().copied());
                paths.push(path);
            }
        }
        paths.sort();
        paths.dedup();
        let alignment = 32u16.max(payload_alignment);
        let payload_offset = align_up(32, payload_alignment)?;
        let size = align_up(
            payload_offset
                .checked_add(payload_size)
                .ok_or(UnionMetadataError::LayoutOverflow)?,
            alignment,
        )?;
        self.finish(
            RuntimeLayoutKind::TaggedUnion,
            size,
            alignment,
            paths,
            Vec::new(),
            alternatives,
        )
    }

    fn finish(
        &self,
        kind: RuntimeLayoutKind,
        size_bits: u64,
        alignment_bits: u16,
        mut reference_paths: Vec<Vec<u32>>,
        fields: Vec<RuntimeLayoutField>,
        alternatives: Vec<RuntimeTypeLayout>,
    ) -> Result<RuntimeTypeLayout, UnionMetadataError> {
        reference_paths.sort();
        reference_paths.dedup();
        let mut hasher = Sha256::new();
        match kind {
            RuntimeLayoutKind::ZeroSized => hasher.update([0]),
            RuntimeLayoutKind::Integer(width) => {
                hasher.update([1]);
                hasher.update(width.bits().to_le_bytes());
                hasher.update([width.signed() as u8]);
            }
            RuntimeLayoutKind::BinaryFloat(width) => {
                hasher.update([2]);
                hasher.update(width.bits().to_le_bytes());
            }
            RuntimeLayoutKind::Decimal => hasher.update([3]),
            RuntimeLayoutKind::Boolean => hasher.update([4]),
            RuntimeLayoutKind::ManagedReference => hasher.update([5]),
            RuntimeLayoutKind::RawPointer => hasher.update([6]),
            RuntimeLayoutKind::Aggregate => hasher.update([7]),
            RuntimeLayoutKind::Nullable => hasher.update([8]),
            RuntimeLayoutKind::TaggedUnion => hasher.update([9]),
        }
        hasher.update(size_bits.to_le_bytes());
        hasher.update(alignment_bits.to_le_bytes());
        for field in &fields {
            hasher.update(field.index.to_le_bytes());
            hasher.update(field.offset_bits.to_le_bytes());
            hasher.update(field.layout.fingerprint);
        }
        for alternative in &alternatives {
            hasher.update(alternative.fingerprint);
        }
        for path in &reference_paths {
            hasher.update((path.len() as u32).to_le_bytes());
            for index in path {
                hasher.update(index.to_le_bytes());
            }
        }
        Ok(RuntimeTypeLayout {
            kind,
            size_bits,
            alignment_bits,
            reference_paths,
            fields,
            alternatives,
            generic_arguments: Vec::new(),
            fingerprint: hasher.finalize().into(),
        })
    }
}

fn align_up(value: u64, alignment_bits: u16) -> Result<u64, UnionMetadataError> {
    let alignment = u64::from(alignment_bits);
    value
        .checked_add(alignment - 1)
        .map(|v| v / alignment * alignment)
        .ok_or(UnionMetadataError::LayoutOverflow)
}

fn natural_alignment(size_bits: u32) -> u16 {
    let bytes = u64::from(size_bits).div_ceil(8).max(1);
    (bytes.next_power_of_two().min(16) * 8) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnumLayout, Nullable, ObjectField, ValueLayout};

    #[test]
    fn recursive_layouts_are_stable_and_include_reference_paths() {
        let module = Module {
            values: vec![ValueLayout {
                name: "Pair".into(),
                fields: vec![
                    ObjectField {
                        name: "number".into(),
                        ty: IrType::Int(IntWidth::I64),
                    },
                    ObjectField {
                        name: "text".into(),
                        ty: IrType::String,
                    },
                ],
            }],
            enums: vec![EnumLayout {
                name: "MaybePair".into(),
                fields: vec![ObjectField {
                    name: "payload".into(),
                    ty: IrType::Value(0),
                }],
                variants: vec![vec![], vec![0]],
                union_layout: None,
            }],
            union_types: vec![{
                let mut union =
                    crate::UnionTypeLayout::new(vec![IrType::String, IrType::Int(IntWidth::I32)])
                        .unwrap();
                union.representation_layout = Some(0);
                union
            }],
            ..Module::default()
        };
        let tuple = module.runtime_type_layout(IrType::Value(0)).unwrap();
        assert_eq!(tuple.size_bits, 128);
        assert_eq!(tuple.reference_paths, [vec![1]]);
        assert_eq!(tuple, module.runtime_type_layout(IrType::Value(0)).unwrap());

        let nullable = module
            .runtime_type_layout(IrType::Nullable(Nullable::Value(0)))
            .unwrap();
        assert_eq!(nullable.kind, RuntimeLayoutKind::Nullable);
        assert_eq!(nullable.reference_paths, [vec![1, 1]]);

        let union = module.runtime_type_layout(IrType::Union(0)).unwrap();
        assert_eq!(union.kind, RuntimeLayoutKind::TaggedUnion);
        assert_eq!(union.alternatives.len(), 2);
        assert_eq!(union.reference_paths.len(), 1);
        assert_eq!(union.reference_paths[0][0], 1);
        assert!(union.reference_paths[0][1] < 2);

        let enumeration = module.runtime_type_layout(IrType::Enum(0)).unwrap();
        assert_eq!(enumeration.kind, RuntimeLayoutKind::Aggregate);
        assert_eq!(enumeration.reference_paths, [vec![1, 1]]);
    }

    #[test]
    fn opaque_generic_handles_include_argument_layouts_in_the_fingerprint() {
        let module = Module {
            map_types: vec![
                (IrType::Int(IntWidth::I32), IrType::String),
                (IrType::Int(IntWidth::I64), IrType::String),
            ],
            set_types: vec![IrType::Int(IntWidth::I32)],
            array_types: vec![IrType::String],
            ..Module::default()
        };
        let map = module.runtime_type_layout(IrType::Map(0)).unwrap();
        assert_eq!(map.size_bits, 64);
        assert_eq!(map.reference_paths.len(), 1);
        assert!(map.reference_paths[0].is_empty());
        assert_eq!(map.generic_arguments.len(), 2);
        assert_eq!(
            map.generic_arguments[0].kind,
            RuntimeLayoutKind::Integer(IntWidth::I32)
        );
        assert_eq!(
            map.generic_arguments[1].kind,
            RuntimeLayoutKind::ManagedReference
        );
        assert_ne!(
            map.fingerprint,
            module
                .runtime_type_layout(IrType::Map(1))
                .unwrap()
                .fingerprint
        );
        assert_eq!(
            module
                .runtime_type_layout(IrType::Set(0))
                .unwrap()
                .generic_arguments
                .len(),
            1
        );
        let array = module.runtime_type_layout(IrType::Array(0)).unwrap();
        assert_eq!(array.generic_arguments[0].reference_paths.len(), 1);
        assert!(array.generic_arguments[0].reference_paths[0].is_empty());
    }
}
