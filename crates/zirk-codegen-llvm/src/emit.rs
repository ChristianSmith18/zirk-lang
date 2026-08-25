//! Translation from the Zirk IR into LLVM IR.
//!
//! This is the only place in the project that knows both representations. The
//! IR arrives already verified, so nothing is re-checked here: what is done is
//! a mechanical translation.
//!
//! # Runtime boundary
//!
//! Everything that is not pure computation goes through `extern "C"` symbols of
//! `zirk-runtime` (`docs/decisions/ADR-002-runtime-staticlib.md`). Codegen
//! declares them and calls them; it never assumes their implementation.

use crate::runtime;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FloatType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FloatValue, FunctionValue, PointerValue,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};
use std::collections::HashMap;
use zirk_ir as ir;

/// Prefix applied to every Zirk function in the generated code.
///
/// It avoids collisions with C symbols — a Zirk function named `printf` must
/// not become one — and frees the name `main` for the C entrypoint the runtime
/// needs. The scheme is internal and will be revisited when packages arrive in
/// Phase 8.
const FUNCTION_PREFIX: &str = "zk_";

/// Name of the Zirk entrypoint, per `ZIRK_RUNTIME_SPEC.md` section 2.
const ENTRYPOINT: &str = "main";

/// The LLVM signature of a closure's lifted body.
///
/// The captures come first, then the parameters: that is the order
/// [`ir::InstKind::CallClosure`] passes them in and the order the lifted
/// function declares them.
fn closure_signature<'ctx>(
    context: &'ctx Context,
    layout: &ir::ClosureLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::FunctionType<'ctx> {
    let params: Vec<BasicMetadataTypeEnum> = layout
        .captures
        .iter()
        .chain(&layout.params)
        .map(|ty| {
            llvm_type_in(context, *ty, closures, values, enums)
                .expect("a capture or parameter cannot be Void")
                .into()
        })
        .collect();

    match llvm_type_in(context, layout.returns, closures, values, enums) {
        Some(ty) => ty.fn_type(&params, false),
        None => context.void_type().fn_type(&params, false),
    }
}

/// Translates an IR module into an LLVM module.
pub fn emit<'ctx>(context: &'ctx Context, module: &ir::Module, name: &str) -> LlvmModule<'ctx> {
    let llvm = context.create_module(name);
    let builder = context.create_builder();

    let runtime = runtime::declare(context, &llvm);

    // Every function is declared before any body is emitted, so a call can
    // reference a function defined further down the file.
    let mut functions = HashMap::new();
    for function in &module.functions {
        let declared = declare_function(
            context,
            &llvm,
            function,
            &module.closures,
            &module.values,
            &module.enums,
        );
        functions.insert(function.name.clone(), declared);
    }

    // `extern "C" fn` declarations (roadmap Phase 4e, design D7) — declared
    // under their own real symbol name, sharing `functions` with ordinary
    // Zirk functions so `InstKind::Call` finds either uniformly (design D7:
    // an extern call lowers through the same `Call` instruction).
    for extern_fn in &module.externs {
        let declared = declare_extern_fn(
            context,
            &llvm,
            extern_fn,
            &module.closures,
            &module.values,
            &module.enums,
        );
        functions.insert(extern_fn.name.clone(), declared);
    }

    // String literals become private global constants. The runtime turns them
    // into `String` values; their layout stays opaque here (ADR-005).
    //
    // The globals are built directly rather than with `build_global_string_ptr`
    // because that helper needs the builder positioned inside a block, and at
    // this point no function body exists yet.
    //
    // No trailing null byte: the length travels as an explicit argument, and a
    // Zirk string may legitimately contain a null.
    let strings: Vec<PointerValue> = module
        .strings
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let bytes = context.const_string(value.as_bytes(), false);
            let global = llvm.add_global(bytes.get_type(), None, &format!("zk.str.{index}"));
            global.set_initializer(&bytes);
            global.set_constant(true);
            global.set_linkage(Linkage::Private);
            global.as_pointer_value()
        })
        .collect();

    // One descriptor per class: its method table, in index order. A subclass's
    // starts with its base's entries, so a method's slot is the same whoever
    // is looking — which is what makes an indirect call one load and one jump.
    let descriptors: Vec<PointerValue> = module
        .objects
        .iter()
        .map(|layout| {
            let ptr = context.ptr_type(AddressSpace::default());

            let table_of = |symbols: &[String], name: &str| {
                let entries: Vec<PointerValue> = symbols
                    .iter()
                    .map(|symbol| functions[symbol].as_global_value().as_pointer_value())
                    .collect();
                let array = ptr.const_array(&entries);
                let global = llvm.add_global(array.get_type(), None, name);
                global.set_initializer(&array);
                global.set_constant(true);
                global.set_linkage(Linkage::Private);
                global.as_pointer_value()
            };

            let methods = table_of(&layout.methods, &format!("zk.vtable.{}", layout.name));

            // The descriptor is the method table, how many ancestor ids
            // follow (itself included) and each one — what a checked cast
            // searches (roadmap task 11.6) — then how many contracts follow
            // and one (id, table) pair each. The runtime searches both
            // lists linearly; the shape is fixed between the two and
            // nothing else reads it.
            let word = context.i64_type();
            let mut fields: Vec<BasicValueEnum> = vec![
                methods.into(),
                word.const_int(layout.ancestors.len() as u64, false).into(),
            ];
            for &ancestor in &layout.ancestors {
                fields.push(word.const_int(u64::from(ancestor), false).into());
            }
            fields.push(word.const_int(layout.contracts.len() as u64, false).into());
            for table in &layout.contracts {
                let symbol = format!("zk.itable.{}.{}", layout.name, table.contract);
                fields.push(word.const_int(u64::from(table.contract), false).into());
                fields.push(table_of(&table.methods, &symbol).into());
            }

            // Appended after the contract tables (`fase-4e-colector-mark-sweep`,
            // design D6): how many collector-managed reference fields this
            // class's objects carry, and each one's byte offset. Nothing but
            // the collector's own mark phase reads past the contract
            // section, so this is purely additive — every existing reader
            // above (`zirk_rt_contract_table`, `zirk_rt_check_cast`,
            // `zirk_rt_is_instance`) still stops exactly where it always did.
            let struct_type = object_struct(
                context,
                layout,
                &module.closures,
                &module.values,
                &module.enums,
            );
            let mut gc_paths: Vec<Vec<u32>> = Vec::new();
            for (index, field) in layout.fields.iter().enumerate() {
                if field.ty.is_managed_reference(module) {
                    let mut prefix = vec![index as u32 + OBJECT_HEADER_FIELDS];
                    gc_reference_paths(module, field.ty, &mut prefix, &mut gc_paths);
                }
            }
            fields.push(word.const_int(gc_paths.len() as u64, false).into());
            for path in &gc_paths {
                fields.push(const_field_offset(context, struct_type, path).into());
            }

            let descriptor = context.const_struct(&fields, false);
            let global = llvm.add_global(
                descriptor.get_type(),
                None,
                &format!("zk.type.{}", layout.name),
            );
            global.set_initializer(&descriptor);
            global.set_constant(true);
            global.set_linkage(Linkage::Private);
            global.as_pointer_value()
        })
        .collect();

    for function in &module.functions {
        FunctionEmitter {
            context,
            builder: &builder,
            llvm: &llvm,
            module,
            runtime: &runtime,
            functions: &functions,
            strings: &strings,
            descriptors: &descriptors,
            values: HashMap::new(),
            value_types: HashMap::new(),
            slots: HashMap::new(),
            blocks: HashMap::new(),
        }
        .emit(function, functions[&function.name]);
    }

    if let Some(entry) = functions.get(ENTRYPOINT) {
        emit_c_entrypoint(context, &builder, &llvm, &runtime, *entry);
    }

    llvm
}

/// The LLVM type of an IR type, resolving closure and value layouts against
/// the module.
fn llvm_type_in<'ctx>(
    context: &'ctx Context,
    ty: ir::IrType,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> Option<BasicTypeEnum<'ctx>> {
    Some(match ty {
        ir::IrType::Void => return None,
        // No value of `Never` exists (roadmap Phase 4a): nothing needs an
        // LLVM representation for it, same as `Void`.
        ir::IrType::Never => return None,
        // LLVM already has a native type for any integer width (roadmap
        // Phase 3b) — signedness is not part of an `IntType` at all in
        // LLVM, only of the operation performed on it (`sdiv` vs `udiv`,
        // `sext` vs `zext`), so nothing here needs to know it.
        ir::IrType::Int(width) => context
            .custom_width_int_type(
                std::num::NonZeroU32::new(width.bits()).expect("every IntWidth is nonzero"),
            )
            .expect("every IntWidth is a valid LLVM integer width")
            .into(),
        // Same idea, one family over: LLVM has a native `FloatType` for each
        // width the language has (roadmap Phase 3b).
        ir::IrType::Float(width) => match width {
            ir::FloatWidth::F16 => context.f16_type().into(),
            ir::FloatWidth::F32 => context.f32_type().into(),
            ir::FloatWidth::F64 => context.f64_type().into(),
            ir::FloatWidth::F128 => context.f128_type().into(),
        },
        // `Boolean` is `i1`: LLVM's natural type for a condition, and what a
        // conditional branch expects.
        ir::IrType::Boolean => context.bool_type().into(),
        // `String` is an opaque pointer. Its layout belongs to the runtime.
        ir::IrType::String => context.ptr_type(AddressSpace::default()).into(),
        // `Char` shares `String`'s opaque runtime representation (ADR-014).
        ir::IrType::Char => context.ptr_type(AddressSpace::default()).into(),
        // An object is reached through its address: identity *is* the address,
        // so the value carried around is a pointer. The struct behind it is
        // only needed where a field is addressed.
        ir::IrType::Object(_) => context.ptr_type(AddressSpace::default()).into(),
        // Reached through the contract, but still just the object's address:
        // the descriptor it already carries answers which body to run.
        ir::IrType::Contract(_) => context.ptr_type(AddressSpace::default()).into(),
        // A record or value class carries no identity, so it is the struct
        // itself, not a pointer to one — passed, returned and stored inline,
        // with no allocation (roadmap task 11.5).
        ir::IrType::Value(id) => {
            let layout = values
                .get(id as usize)
                .expect("a verified module declares every value layout");
            value_struct(context, layout, closures, values, enums).into()
        }
        // An algebraic enum is discriminant plus payload, inline the same
        // way a record is — no allocation, no identity (roadmap task 11.3).
        ir::IrType::Enum(id) => {
            let layout = enums
                .get(id as usize)
                .expect("a verified module declares every enum layout");
            enum_struct(context, layout, closures, values, enums).into()
        }
        // A present flag next to the value. The flag comes first so the struct
        // has the same shape whatever the payload is.
        ir::IrType::Nullable(base) => {
            nullable_struct(context, base.inner(), closures, values, enums).into()
        }
        // A closure is its function pointer followed by its captures, inline.
        // Nothing is allocated: it escapes as a value, never through a
        // heap indirection of its own (design D10 of `fase-4d-callables`).
        ir::IrType::Closure(id) => {
            let layout = closures
                .get(id as usize)
                .expect("a verified module declares every closure layout");
            closure_struct(context, layout, closures, values, enums).into()
        }
        // `Pointer<T>` (roadmap Phase 4e, design D8): an ordinary LLVM
        // pointer — opaque at this level, since LLVM's own `ptr` type
        // carries no pointee type; a load/store through it supplies `T`'s
        // own LLVM type separately, at the instruction that needs it.
        ir::IrType::Pointer(_) => context.ptr_type(AddressSpace::default()).into(),
    })
}

/// The struct a record or value class occupies — its fields, in declaration
/// order, with no header and no indirection (roadmap task 11.5).
fn value_struct<'ctx>(
    context: &'ctx Context,
    layout: &ir::ValueLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let fields: Vec<BasicTypeEnum> = layout
        .fields
        .iter()
        .map(|field| {
            llvm_type_in(context, field.ty, closures, values, enums)
                .expect("a value field is not Void")
        })
        .collect();
    context.struct_type(&fields, false)
}

/// The struct an algebraic enum occupies: its discriminant, then every
/// variant's associated fields flattened and concatenated — see
/// [`ir::EnumLayout`] for why this is not a byte-level union (roadmap task
/// 11.3).
fn enum_struct<'ctx>(
    context: &'ctx Context,
    layout: &ir::EnumLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let mut fields: Vec<BasicTypeEnum> = vec![context.i32_type().into()];
    for field in &layout.fields {
        // A `Void` field carries no value — `Result<Void,E>` is
        // `Resource<E>::close()`'s own return type (roadmap Phase 4c) — but
        // still occupies a genuine (zero-sized) struct slot rather than
        // being skipped, so every other field's index into this struct
        // keeps matching `EnumLayout.variants`'s unchanged numbering.
        // `InstKind::BuildEnum`/`LoadField` never read or write into it.
        fields.push(
            llvm_type_in(context, field.ty, closures, values, enums)
                .unwrap_or_else(|| context.struct_type(&[], false).into()),
        );
    }
    context.struct_type(&fields, false)
}

/// Where a field sits inside an enum's struct, discriminant included.
const ENUM_HEADER_FIELDS: u32 = 1;

/// The struct a nullable value occupies: a present flag, then the payload —
/// the flag comes first so the struct has the same shape whatever the
/// payload is. Field 1 is always the payload, whatever `inner` is (used by
/// the GC root/field walk, `gc_reference_paths`, to step through a
/// `Nullable(Object|Value|Enum)` uniformly).
fn nullable_struct<'ctx>(
    context: &'ctx Context,
    inner: ir::IrType,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let inner_ty = llvm_type_in(context, inner, closures, values, enums)
        .expect("a nullable payload is not Void");
    context.struct_type(&[context.bool_type().into(), inner_ty], false)
}

/// Where a nullable's payload sits, its present flag included.
const NULLABLE_HEADER_FIELDS: u32 = 1;

/// The struct a closure value occupies: its function pointer, then its
/// captures inline, in declaration order. Capture `i` sits at field
/// `i + CLOSURE_HEADER_FIELDS`.
fn closure_struct<'ctx>(
    context: &'ctx Context,
    layout: &ir::ClosureLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let mut fields: Vec<BasicTypeEnum> = vec![context.ptr_type(AddressSpace::default()).into()];
    for capture in &layout.captures {
        fields.push(
            llvm_type_in(context, *capture, closures, values, enums)
                .expect("a capture is not Void"),
        );
    }
    context.struct_type(&fields, false)
}

/// Where a capture sits inside a closure's struct, the function pointer
/// included.
const CLOSURE_HEADER_FIELDS: u32 = 1;

/// The leaf field-index path (design D2/D6 of `fase-4e-colector-mark-sweep`)
/// to every collector-managed reference `ty` carries, directly or nested
/// inside a `Value`/`Enum`/`Closure`/`Nullable`.
///
/// `prefix` is the path already walked to reach `ty` itself (empty for a
/// bare slot's own type); each element of `out` is a *complete* path from
/// whatever the caller's own base struct/slot is. The same path shape
/// serves two different appliers: [`FunctionEmitter::apply_gc_path`] walks
/// it with live `build_struct_gep` instructions against a real address (the
/// function-frame shadow-stack roots, design D2); [`const_field_offset`]
/// walks the identical path as an LLVM constant expression to bake a byte
/// offset into a class's static descriptor (the heap-object field walk,
/// design D6) — one path-collector, two appliers, so they cannot drift
/// apart.
///
/// A `Value`/`Enum`/`Closure` layout can never nest itself (infinite size,
/// already rejected upstream), so this recursion always terminates.
fn gc_reference_paths(
    module: &ir::Module,
    ty: ir::IrType,
    prefix: &mut Vec<u32>,
    out: &mut Vec<Vec<u32>>,
) {
    match ty {
        ir::IrType::Object(_) | ir::IrType::Contract(_) => out.push(prefix.clone()),
        ir::IrType::Nullable(n) => {
            let inner = n.inner();
            if inner.is_managed_reference(module) {
                prefix.push(NULLABLE_HEADER_FIELDS);
                gc_reference_paths(module, inner, prefix, out);
                prefix.pop();
            }
        }
        ir::IrType::Value(id) => {
            for (index, field) in module.values[id as usize].fields.iter().enumerate() {
                if field.ty.is_managed_reference(module) {
                    prefix.push(index as u32);
                    gc_reference_paths(module, field.ty, prefix, out);
                    prefix.pop();
                }
            }
        }
        ir::IrType::Enum(id) => {
            for (index, field) in module.enums[id as usize].fields.iter().enumerate() {
                if field.ty.is_managed_reference(module) {
                    prefix.push(index as u32 + ENUM_HEADER_FIELDS);
                    gc_reference_paths(module, field.ty, prefix, out);
                    prefix.pop();
                }
            }
        }
        ir::IrType::Closure(id) => {
            for (index, capture) in module.closures[id as usize].captures.iter().enumerate() {
                if capture.is_managed_reference(module) {
                    prefix.push(index as u32 + CLOSURE_HEADER_FIELDS);
                    gc_reference_paths(module, *capture, prefix, out);
                    prefix.pop();
                }
            }
        }
        _ => {}
    }
}

/// [`gc_reference_paths`] with a fresh, empty prefix — the shape a bare
/// slot's own type needs (there is no enclosing field index to seed it
/// with).
fn gc_reference_paths_of(module: &ir::Module, ty: ir::IrType) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    gc_reference_paths(module, ty, &mut Vec::new(), &mut out);
    out
}

/// The byte offset `path` (design D6, [`gc_reference_paths`]) reaches inside
/// a value of `struct_type`, as an LLVM constant expression — the classic
/// null-pointer-GEP-then-`ptrtoint` idiom, which folds to the real,
/// target-specific offset when LLVM lowers it, the same way
/// `StructType::size_of` (used for `Alloc`'s own size argument) already
/// resolves a target-specific size without this crate ever consulting
/// `TargetData` itself.
fn const_field_offset<'ctx>(
    context: &'ctx Context,
    struct_type: inkwell::types::StructType<'ctx>,
    path: &[u32],
) -> inkwell::values::IntValue<'ctx> {
    let ptr_type = context.ptr_type(AddressSpace::default());
    let null = ptr_type.const_null();
    let i32_type = context.i32_type();
    let mut indices: Vec<inkwell::values::IntValue> = vec![i32_type.const_int(0, false)];
    indices.extend(
        path.iter()
            .map(|&index| i32_type.const_int(u64::from(index), false)),
    );
    let field_ptr = unsafe { null.const_gep(struct_type, &indices) };
    field_ptr.const_to_int(context.i64_type())
}

/// The struct an object of this layout occupies.
///
/// ```text
///    [ type descriptor | next (mark bit) | size | field₁ | field₂ | … ]
/// ```
///
/// The descriptor comes first so every object starts the same way, whatever
/// its fields — which is what lets a subclass share its base's prefix. The
/// two collector-bookkeeping words after it (`fase-4e-colector-mark-sweep`,
/// design D1) are new: `next` threads every allocation onto the sweep's
/// intrusive list (its low bit doubles as the mark bit — nobody but the
/// collector itself reads this field), and `size` is the allocation's own
/// byte size, so sweep can `dealloc` correctly. Both are initialized by
/// `zirk_rt_alloc` itself, not here (task 2.1's own resolution — see its
/// note in `tasks.md`): the allocator already receives `size` as a
/// parameter, so it is the single source of truth for it, the same way the
/// descriptor word is this function's own.
fn object_struct<'ctx>(
    context: &'ctx Context,
    layout: &ir::ObjectLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let ptr = context.ptr_type(AddressSpace::default());
    let mut fields: Vec<BasicTypeEnum> = vec![ptr.into(), ptr.into(), context.i64_type().into()];
    for field in &layout.fields {
        fields.push(
            llvm_type_in(context, field.ty, closures, values, enums)
                .expect("an object field is not Void"),
        );
    }
    context.struct_type(&fields, false)
}

/// Where a field sits inside the struct, header included (design D1: three
/// words now — descriptor, `next`, `size` — not one).
const OBJECT_HEADER_FIELDS: u32 = 3;

fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    function: &ir::Function,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> FunctionValue<'ctx> {
    let params: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|slot| {
            llvm_type_in(
                context,
                function.slots[slot.0 as usize].ty,
                closures,
                values,
                enums,
            )
            .expect("a parameter cannot be Void")
            .into()
        })
        .collect();

    let signature = match llvm_type_in(context, function.return_type, closures, values, enums) {
        Some(ty) => ty.fn_type(&params, false),
        None => context.void_type().fn_type(&params, false),
    };

    llvm.add_function(
        &format!("{FUNCTION_PREFIX}{}", function.name),
        signature,
        None,
    )
}

/// `extern "C" fn` (roadmap Phase 4e, design D7, `ADR-015`): an ordinary
/// LLVM external function declaration, with no body and the real (unprefixed)
/// native symbol name — unlike an ordinary Zirk function, which gets
/// [`FUNCTION_PREFIX`] and a `define`. Resolving it at link time is left
/// entirely to the system linker, per ADR-015.
fn declare_extern_fn<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    extern_fn: &ir::ExternFn,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> FunctionValue<'ctx> {
    let params: Vec<BasicMetadataTypeEnum> = extern_fn
        .params
        .iter()
        .map(|&ty| {
            llvm_type_in(context, ty, closures, values, enums)
                .expect("an extern parameter cannot be Void")
                .into()
        })
        .collect();

    let signature = match llvm_type_in(context, extern_fn.return_type, closures, values, enums) {
        Some(ty) => ty.fn_type(&params, false),
        None => context.void_type().fn_type(&params, false),
    };

    let declared = llvm.add_function(&extern_fn.name, signature, None);
    // LLVM's default call convention is already C (`CallConv::C == 0`);
    // set explicitly so the declaration states it rather than relying on
    // the default (spec scenario "Declaration becomes an external symbol").
    declared.set_call_conventions(0);
    declared
}

/// Emits the C `main` the operating system invokes.
///
/// It is what materializes the lifecycle of `ZIRK_RUNTIME_SPEC.md` section 2:
/// the runtime is initialized, Zirk's `main` runs, and the runtime is shut down
/// before returning the exit code.
fn emit_c_entrypoint<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    llvm: &LlvmModule<'ctx>,
    runtime: &runtime::Runtime<'ctx>,
    zirk_main: FunctionValue<'ctx>,
) {
    let i32_type = context.i32_type();
    let main = llvm.add_function(
        "main",
        i32_type.fn_type(&[], false),
        Some(Linkage::External),
    );
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);

    builder
        .build_call(runtime.init, &[], "")
        .expect("call to the runtime initializer");
    builder
        .build_call(zirk_main, &[], "")
        .expect("call to Zirk main");

    // `main` itself may declare `throws` and still leave an exception
    // pending — nothing above it in the call chain checks (roadmap Phase
    // 4b, `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3: "an
    // uncaught `main` exception ... exits nonzero"). `zirk_rt_uncaught_exception`
    // never returns, so the block after it is unreachable.
    let pending = builder
        .build_call(runtime.has_pending_exception, &[], "")
        .expect("call to the pending-exception query")
        .try_as_basic_value()
        .basic()
        .expect("has_pending_exception returns a value")
        .into_int_value();
    let uncaught_block = context.append_basic_block(main, "uncaught_exception");
    let clean_block = context.append_basic_block(main, "clean_exit");
    builder
        .build_conditional_branch(pending, uncaught_block, clean_block)
        .expect("branch on the pending exception");

    builder.position_at_end(uncaught_block);
    builder
        .build_call(runtime.uncaught_exception, &[], "")
        .expect("call to the uncaught-exception handler");
    builder.build_unreachable().expect("never returns");

    builder.position_at_end(clean_block);
    builder
        .build_call(runtime.shutdown, &[], "")
        .expect("call to the runtime shutdown");

    builder
        .build_return(Some(&i32_type.const_int(0, false)))
        .expect("exit code");
}

struct FunctionEmitter<'ctx, 'a> {
    context: &'ctx Context,
    builder: &'a Builder<'ctx>,
    llvm: &'a LlvmModule<'ctx>,
    module: &'a ir::Module,
    runtime: &'a runtime::Runtime<'ctx>,
    functions: &'a HashMap<String, FunctionValue<'ctx>>,
    strings: &'a [PointerValue<'ctx>],
    /// The type descriptor of each class, by layout id.
    descriptors: &'a [PointerValue<'ctx>],

    values: HashMap<ir::ValueId, BasicValueEnum<'ctx>>,
    /// The IR type of each emitted value, which a pointer alone does not carry.
    value_types: HashMap<ir::ValueId, ir::IrType>,
    slots: HashMap<ir::SlotId, PointerValue<'ctx>>,
    blocks: HashMap<ir::BlockId, inkwell::basic_block::BasicBlock<'ctx>>,
}

impl<'ctx> FunctionEmitter<'ctx, '_> {
    fn emit(mut self, function: &ir::Function, llvm_function: FunctionValue<'ctx>) {
        // Every block is created up front so a branch can reference one that
        // has not been filled yet.
        for block in &function.blocks {
            let name = format!("bb{}", block.id.0);
            self.blocks.insert(
                block.id,
                self.context.append_basic_block(llvm_function, &name),
            );
        }

        // Slot allocations live in the entry block: LLVM expects them there to
        // promote them to registers, which is what makes our own SSA
        // unnecessary (design D2).
        self.builder.position_at_end(self.blocks[&function.entry]);
        for (index, slot) in function.slots.iter().enumerate() {
            let ty = llvm_type_in(
                self.context,
                slot.ty,
                &self.module.closures,
                &self.module.values,
                &self.module.enums,
            )
            .expect("a slot cannot be Void");
            let pointer = self
                .builder
                .build_alloca(ty, &slot.name)
                .expect("slot allocation");
            self.slots.insert(ir::SlotId(index as u32), pointer);
        }

        // Design D5 (`fase-4e-colector-mark-sweep`): every reference-typed
        // slot is zero-initialized immediately after its own `alloca`,
        // before any other codegen for this function runs — so a
        // collection that happens before the slot's first real write never
        // reads whatever garbage the stack held as a candidate pointer.
        for &slot_id in &function.gc_roots {
            let slot = &function.slots[slot_id.0 as usize];
            let ty = self
                .llvm_type(slot.ty)
                .expect("a gc-root slot cannot be Void");
            let pointer = self.slots[&slot_id];
            self.builder
                .build_store(pointer, ty.const_zero())
                .expect("zero-initialize a reference slot");
        }

        // Design D2: this activation's shadow-stack frame is pushed once,
        // right here — after every reference slot above is zeroed, before
        // anything else in the function body runs. Each root is the
        // *address* of a reference-typed slot (or, for a `Value`/`Enum`/
        // `Closure` slot, one of its own reference-typed fields, found via
        // `gc_reference_paths`) — not the reference's value itself, which
        // is why the collector's own mark phase dereferences each entry
        // once more to reach the candidate object.
        let mut root_addrs: Vec<PointerValue> = Vec::new();
        for &slot_id in &function.gc_roots {
            let slot = &function.slots[slot_id.0 as usize];
            let base_ptr = self.slots[&slot_id];
            for path in gc_reference_paths_of(self.module, slot.ty) {
                root_addrs.push(self.apply_gc_path(base_ptr, slot.ty, &path));
            }
        }
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        if root_addrs.is_empty() {
            self.builder
                .build_call(
                    self.runtime.push_frame,
                    &[
                        ptr_type.const_null().into(),
                        i64_type.const_int(0, false).into(),
                    ],
                    "",
                )
                .expect("push an empty gc frame");
        } else {
            let array_ty = ptr_type.array_type(root_addrs.len() as u32);
            let array = self
                .builder
                .build_alloca(array_ty, "gc_roots")
                .expect("gc roots array allocation");
            for (index, addr) in root_addrs.iter().enumerate() {
                let element_ptr = unsafe {
                    self.builder.build_gep(
                        array_ty,
                        array,
                        &[
                            self.context.i32_type().const_int(0, false),
                            self.context.i32_type().const_int(index as u64, false),
                        ],
                        "gc_root_slot",
                    )
                }
                .expect("gc root array index");
                self.builder
                    .build_store(element_ptr, *addr)
                    .expect("store a gc root address");
            }
            self.builder
                .build_call(
                    self.runtime.push_frame,
                    &[
                        array.into(),
                        i64_type.const_int(root_addrs.len() as u64, false).into(),
                    ],
                    "",
                )
                .expect("push the gc frame");
        }

        // Parameters arrive as values and are stored into their slots, so
        // reading them is uniform with any other local.
        for (index, slot) in function.params.iter().enumerate() {
            let value = llvm_function
                .get_nth_param(index as u32)
                .expect("declared parameter");
            self.builder
                .build_store(self.slots[slot], value)
                .expect("parameter store");
        }

        for block in &function.blocks {
            self.builder.position_at_end(self.blocks[&block.id]);

            for instruction in &block.instructions {
                self.emit_instruction(instruction, llvm_function);
            }

            let terminator = block
                .terminator
                .as_ref()
                .expect("verified IR always has a terminator");
            self.emit_terminator(terminator);
        }
    }

    fn operand(&self, operand: ir::Operand) -> BasicValueEnum<'ctx> {
        self.values[&operand.0]
    }

    /// Shorthand for [`llvm_type_in`] against this function's own module
    /// tables (roadmap Phase 4e — pointer lowering needs this repeatedly for
    /// a pointee's own type, which carries no LLVM representation of its
    /// own the way an opaque `ptr` does).
    fn llvm_type(&self, ty: ir::IrType) -> Option<BasicTypeEnum<'ctx>> {
        llvm_type_in(
            self.context,
            ty,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        )
    }

    /// Walks a live GC reference path (`gc_reference_paths`) from a real base
    /// address, with real `build_struct_gep` instructions — the shadow-stack
    /// frame's own applier (design D2), as opposed to [`const_field_offset`]
    /// which walks the identical path shape as a compile-time constant for a
    /// class's static descriptor (design D6). Terminates the moment `path`
    /// runs out: that is the leaf, the address of the reference itself.
    fn apply_gc_path(
        &self,
        ptr: PointerValue<'ctx>,
        ty: ir::IrType,
        path: &[u32],
    ) -> PointerValue<'ctx> {
        let Some((&index, rest)) = path.split_first() else {
            return ptr;
        };
        match ty {
            ir::IrType::Nullable(n) => {
                let inner = n.inner();
                let struct_type = nullable_struct(
                    self.context,
                    inner,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let next = self
                    .builder
                    .build_struct_gep(struct_type, ptr, index, "gc_step")
                    .expect("gc nullable step");
                self.apply_gc_path(next, inner, rest)
            }
            ir::IrType::Value(id) => {
                let layout = &self.module.values[id as usize];
                let struct_type = value_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let field_ty = layout.fields[index as usize].ty;
                let next = self
                    .builder
                    .build_struct_gep(struct_type, ptr, index, "gc_step")
                    .expect("gc value step");
                self.apply_gc_path(next, field_ty, rest)
            }
            ir::IrType::Enum(id) => {
                let layout = &self.module.enums[id as usize];
                let struct_type = enum_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let field_ty = layout.fields[(index - ENUM_HEADER_FIELDS) as usize].ty;
                let next = self
                    .builder
                    .build_struct_gep(struct_type, ptr, index, "gc_step")
                    .expect("gc enum step");
                self.apply_gc_path(next, field_ty, rest)
            }
            ir::IrType::Closure(id) => {
                let layout = &self.module.closures[id as usize];
                let struct_type = closure_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let field_ty = layout.captures[(index - CLOSURE_HEADER_FIELDS) as usize];
                let next = self
                    .builder
                    .build_struct_gep(struct_type, ptr, index, "gc_step")
                    .expect("gc closure step");
                self.apply_gc_path(next, field_ty, rest)
            }
            _ => unreachable!("a gc path only steps through Value/Enum/Closure/Nullable"),
        }
    }

    /// The address of one field inside an object.
    fn field_pointer(&self, object: ir::Operand, index: u32) -> PointerValue<'ctx> {
        let id = self.object_layout_of(object);
        let layout = &self.module.objects[id as usize];
        let struct_type = object_struct(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );

        self.builder
            .build_struct_gep(
                struct_type,
                self.operand(object).into_pointer_value(),
                index + OBJECT_HEADER_FIELDS,
                "field_ptr",
            )
            .expect("a verified module addresses a field the layout has")
    }

    /// The signature of one contract method, taken from any class that
    /// supplies it.
    ///
    /// Every implementation shares it — that is what conformance checked — so
    /// the first one found describes them all.
    fn contract_signature(&self, contract: u32, index: u32) -> inkwell::types::FunctionType<'ctx> {
        let symbol = self
            .module
            .objects
            .iter()
            .find_map(|layout| {
                layout
                    .contracts
                    .iter()
                    .find(|t| t.contract == contract)
                    .and_then(|t| t.methods.get(index as usize))
            })
            .expect("a verified module has an implementation of every reachable contract");

        self.functions[symbol].get_type()
    }

    /// The layout an object operand belongs to.
    fn object_layout_of(&self, operand: ir::Operand) -> u32 {
        let ir::IrType::Object(id) = self.value_types[&operand.0] else {
            unreachable!("a verified field access reads an object")
        };
        id
    }

    fn emit_instruction(&mut self, instruction: &ir::Instruction, function: FunctionValue<'ctx>) {
        let value: Option<BasicValueEnum> = match &instruction.kind {
            ir::InstKind::ConstInt(value) => Some(
                self.context
                    .i32_type()
                    .const_int(*value as u64, true)
                    .into(),
            ),
            ir::InstKind::ConstBool(value) => Some(
                self.context
                    .bool_type()
                    .const_int(*value as u64, false)
                    .into(),
            ),
            ir::InstKind::Concat { left, right } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_concat,
                        &[self.operand(*left).into(), self.operand(*right).into()],
                        "concat",
                    )
                    .expect("concatenate");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::Repeat { string, count } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_repeat,
                        &[self.operand(*string).into(), self.operand(*count).into()],
                        "repeat",
                    )
                    .expect("repeat");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::CallContract {
                object,
                contract,
                index,
                args,
            } => {
                let receiver = self.operand(*object).into_pointer_value();
                let ptr = self.context.ptr_type(AddressSpace::default());

                let descriptor = self
                    .builder
                    .build_load(ptr, receiver, "descriptor")
                    .expect("load the descriptor")
                    .into_pointer_value();

                // Which table answers for this contract is not statically
                // known — that is what a contract is for — so the runtime
                // finds it in the descriptor.
                let id = self
                    .context
                    .i64_type()
                    .const_int(u64::from(*contract), false);
                let table = self
                    .builder
                    .build_call(
                        self.runtime.contract_table,
                        &[descriptor.into(), id.into()],
                        "itable",
                    )
                    .expect("find the contract table")
                    .try_as_basic_value()
                    .basic()
                    .expect("the lookup returns a pointer")
                    .into_pointer_value();

                let offset = self.context.i32_type().const_int(u64::from(*index), false);
                let slot = unsafe {
                    self.builder
                        .build_in_bounds_gep(ptr, table, &[offset], "method_slot")
                        .expect("a verified module calls a method the table has")
                };
                let target = self
                    .builder
                    .build_load(ptr, slot, "method")
                    .expect("load the method")
                    .into_pointer_value();

                let signature = self.contract_signature(*contract, *index);
                let mut arguments: Vec<BasicMetadataValueEnum> = vec![receiver.into()];
                arguments.extend(
                    args.iter()
                        .map(|a| BasicMetadataValueEnum::from(self.operand(*a))),
                );

                let call = self
                    .builder
                    .build_indirect_call(signature, target, &arguments, "call")
                    .expect("call through the contract table");
                call.try_as_basic_value().basic()
            }

            // `throw`'s own early-return placeholder (roadmap Phase 4b) —
            // see `InstKind::Undefined`'s own doc comment. `Void`/`Never`
            // produce no value at all, the same as any other instruction
            // whose type is one of those.
            ir::InstKind::Undefined => llvm_type_in(
                self.context,
                instruction.ty,
                &self.module.closures,
                &self.module.values,
                &self.module.enums,
            )
            .map(|ty| match ty {
                BasicTypeEnum::IntType(t) => t.get_undef().into(),
                BasicTypeEnum::FloatType(t) => t.get_undef().into(),
                BasicTypeEnum::PointerType(t) => t.get_undef().into(),
                BasicTypeEnum::StructType(t) => t.get_undef().into(),
                BasicTypeEnum::ArrayType(t) => t.get_undef().into(),
                BasicTypeEnum::VectorType(t) => t.get_undef().into(),
                BasicTypeEnum::ScalableVectorType(t) => t.get_undef().into(),
            }),

            ir::InstKind::CheckedCast {
                object,
                target_class,
            } => {
                // The runtime terminates the process if the check fails
                // (roadmap task 11.6) — there is no branch to build here,
                // the same shape `CallContract`'s own descriptor lookup
                // aborts through when a contract turns out to be missing.
                // A successful check changes nothing about the pointer
                // itself: only its declared type differs from here on.
                let receiver = self.operand(*object).into_pointer_value();
                let ptr = self.context.ptr_type(AddressSpace::default());
                let descriptor = self
                    .builder
                    .build_load(ptr, receiver, "descriptor")
                    .expect("load the descriptor")
                    .into_pointer_value();
                let target = self
                    .context
                    .i64_type()
                    .const_int(u64::from(*target_class), false);
                self.builder
                    .build_call(
                        self.runtime.check_cast,
                        &[descriptor.into(), target.into()],
                        "check_cast",
                    )
                    .expect("confirm the checked cast");
                Some(receiver.into())
            }

            // `catch Type(name)`'s own runtime test (roadmap Phase 4b): the
            // same descriptor load `CheckedCast` does, but through
            // `zirk_rt_is_instance`, which answers rather than terminates.
            ir::InstKind::IsInstance {
                object,
                target_class,
            } => {
                let receiver = self.operand(*object).into_pointer_value();
                let ptr = self.context.ptr_type(AddressSpace::default());
                let descriptor = self
                    .builder
                    .build_load(ptr, receiver, "descriptor")
                    .expect("load the descriptor")
                    .into_pointer_value();
                let target = self
                    .context
                    .i64_type()
                    .const_int(u64::from(*target_class), false);
                let call = self
                    .builder
                    .build_call(
                        self.runtime.is_instance,
                        &[descriptor.into(), target.into()],
                        "is_instance",
                    )
                    .expect("test the instance");
                call.try_as_basic_value().basic()
            }

            // A proven-safe widening (a subclass where its base is
            // expected, or a class where a contract it implements is): the
            // pointer itself is unchanged, only its declared type differs
            // from here on.
            ir::InstKind::Retype(operand) => Some(self.operand(*operand)),

            // `value as <a different integer width>` (roadmap Phase 3b,
            // task 4.3) — truncate, or sign/zero-extend by the *source*
            // operand's own signedness (LLVM's `IntType` itself carries no
            // sign; only the operation choosing sext vs zext does). Equal
            // width is a pure reinterpretation — the same bits, so no
            // instruction is needed at all.
            ir::InstKind::IntCast(operand) => {
                let value = self.operand(*operand).into_int_value();
                let source_signed = self.is_signed(self.value_types[&operand.0]);
                let ir::IrType::Int(target_width) = instruction.ty else {
                    unreachable!("IntCast always declares an integer destination")
                };
                let target_ty = self
                    .context
                    .custom_width_int_type(
                        std::num::NonZeroU32::new(target_width.bits())
                            .expect("every IntWidth is nonzero"),
                    )
                    .expect("every IntWidth is a valid LLVM integer width");

                let src_bits = value.get_type().get_bit_width();
                let dst_bits = target_width.bits();
                let converted = match src_bits.cmp(&dst_bits) {
                    std::cmp::Ordering::Equal => value,
                    std::cmp::Ordering::Less if source_signed => self
                        .builder
                        .build_int_s_extend(value, target_ty, "sext")
                        .expect("sign extension"),
                    std::cmp::Ordering::Less => self
                        .builder
                        .build_int_z_extend(value, target_ty, "zext")
                        .expect("zero extension"),
                    std::cmp::Ordering::Greater => self
                        .builder
                        .build_int_truncate(value, target_ty, "trunc")
                        .expect("truncation"),
                };
                Some(converted.into())
            }

            // A fractional or scientific literal (roadmap Phase 3b). Parsed
            // once, here, straight from the source text through LLVM's own
            // literal parser — never through a Rust `f64` in between, which
            // is what lets a `Float128` literal keep precision an `f64`
            // could not hold.
            ir::InstKind::ConstFloat(width, text) => {
                let float_ty = self.float_type(*width);
                Some(unsafe { float_ty.const_float_from_string(text) }.into())
            }

            // `value as <a different Float width>` (roadmap Phase 3b) —
            // `fptrunc`/`fpext`, unchecked. Equal width is a pure
            // reinterpretation, needing no instruction, mirroring `IntCast`.
            ir::InstKind::FloatCast(operand) => {
                let value = self.operand(*operand).into_float_value();
                let ir::IrType::Float(target_width) = instruction.ty else {
                    unreachable!("FloatCast always declares a Float destination")
                };
                let target_ty = self.float_type(target_width);

                let converted = match self.value_types[&operand.0] {
                    ir::IrType::Float(source_width)
                        if source_width.bits() == target_width.bits() =>
                    {
                        value
                    }
                    ir::IrType::Float(source_width)
                        if source_width.bits() < target_width.bits() =>
                    {
                        self.builder
                            .build_float_ext(value, target_ty, "fpext")
                            .expect("float extension")
                    }
                    _ => self
                        .builder
                        .build_float_trunc(value, target_ty, "fptrunc")
                        .expect("float truncation"),
                };
                Some(converted.into())
            }

            // An integer operand converted to `Float`, either an implicit
            // mixed-arithmetic widening or an explicit `as` (roadmap Phase
            // 3b) — `sitofp`/`uitofp`, chosen by the *source*'s own
            // signedness, unchecked: precision loss for a very wide integer
            // is possible and accepted, the same as every other conversion
            // in this family.
            ir::InstKind::IntToFloat(operand) => {
                let value = self.operand(*operand).into_int_value();
                let source_signed = self.is_signed(self.value_types[&operand.0]);
                let ir::IrType::Float(target_width) = instruction.ty else {
                    unreachable!("IntToFloat always declares a Float destination")
                };
                let target_ty = self.float_type(target_width);
                let converted = if source_signed {
                    self.builder
                        .build_signed_int_to_float(value, target_ty, "sitofp")
                        .expect("signed int to float")
                } else {
                    self.builder
                        .build_unsigned_int_to_float(value, target_ty, "uitofp")
                        .expect("unsigned int to float")
                };
                Some(converted.into())
            }

            // `value as <an integer width>` from a `Float` operand (roadmap
            // Phase 3b) — `fptosi`/`fptoui`, chosen by the *destination*'s
            // signedness. Unchecked: a `Float` outside the destination's
            // representable range is undefined at the LLVM level, exactly
            // like Rust's own lossy `as` between float and integer.
            ir::InstKind::FloatToInt(operand) => {
                let value = self.operand(*operand).into_float_value();
                let ir::IrType::Int(target_width) = instruction.ty else {
                    unreachable!("FloatToInt always declares an integer destination")
                };
                let target_ty = self
                    .context
                    .custom_width_int_type(
                        std::num::NonZeroU32::new(target_width.bits())
                            .expect("every IntWidth is nonzero"),
                    )
                    .expect("every IntWidth is a valid LLVM integer width");
                let converted = if target_width.signed() {
                    self.builder
                        .build_float_to_signed_int(value, target_ty, "fptosi")
                        .expect("float to signed int")
                } else {
                    self.builder
                        .build_float_to_unsigned_int(value, target_ty, "fptoui")
                        .expect("float to unsigned int")
                };
                Some(converted.into())
            }

            ir::InstKind::CallVirtual {
                object,
                index,
                args,
            } => {
                let receiver = self.operand(*object).into_pointer_value();

                // The descriptor lives in the header, so which body runs is
                // decided by the object itself and not by the static type of
                // whoever is holding it. Its first field is the method table.
                let ptr = self.context.ptr_type(AddressSpace::default());
                let descriptor = self
                    .builder
                    .build_load(ptr, receiver, "descriptor")
                    .expect("load the descriptor")
                    .into_pointer_value();
                let table = self
                    .builder
                    .build_load(ptr, descriptor, "vtable")
                    .expect("load the method table")
                    .into_pointer_value();

                // The table is an array of pointers, so the slot is the base
                // displaced by the index.
                let offset = self.context.i32_type().const_int(u64::from(*index), false);
                let slot = unsafe {
                    self.builder
                        .build_in_bounds_gep(ptr, table, &[offset], "method_slot")
                        .expect("a verified module calls a method the table has")
                };
                let target = self
                    .builder
                    .build_load(ptr, slot, "method")
                    .expect("load the method")
                    .into_pointer_value();

                let basic_values: Vec<BasicValueEnum> = std::iter::once(receiver.into())
                    .chain(args.iter().map(|a| self.operand(*a)))
                    .collect();

                // Built from the call's own known return type and each
                // argument's own LLVM type, not from
                // `self.functions[&layout.methods[index]]`'s declared type
                // (roadmap Phase 4b) — that name is whichever class the
                // *static* receiver type happens to be, which for a value
                // statically typed as an `abstract class` (`catch
                // Throwable(e)`, `e.message()`) is
                // `UNREACHABLE_ABSTRACT_METHOD`'s dummy `Void`, no-argument
                // shape, not `message`'s real `(): String`. The dynamic
                // target loaded above already resolves to the concrete
                // implementation regardless; only the indirect call's own
                // ABI shape was ever at risk of disagreeing with it.
                let param_types: Vec<BasicMetadataTypeEnum> = basic_values
                    .iter()
                    .map(|v| BasicMetadataTypeEnum::from(v.get_type()))
                    .collect();
                let arguments: Vec<BasicMetadataValueEnum> = basic_values
                    .into_iter()
                    .map(BasicMetadataValueEnum::from)
                    .collect();
                let signature = match llvm_type_in(
                    self.context,
                    instruction.ty,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                ) {
                    Some(ty) => ty.fn_type(&param_types, false),
                    None => self.context.void_type().fn_type(&param_types, false),
                };

                let call = self
                    .builder
                    .build_indirect_call(signature, target, &arguments, "call")
                    .expect("call through the table");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::Alloc(id) => {
                let layout = &self.module.objects[*id as usize];
                let struct_type = object_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );

                // The size and alignment come from LLVM's own data layout, so
                // the runtime is told what the target actually needs rather
                // than what a hand-written table guessed.
                let size = struct_type.size_of().expect("a sized object");
                let align = struct_type.get_alignment();

                let object = self
                    .builder
                    .build_call(self.runtime.alloc, &[size.into(), align.into()], "object")
                    .expect("call the allocator")
                    .try_as_basic_value()
                    .basic()
                    .expect("the allocator returns a pointer")
                    .into_pointer_value();

                // The descriptor goes into the header right away: it is what
                // makes the object know its own type, which is what every
                // dynamic dispatch reads.
                self.builder
                    .build_store(object, self.descriptors[*id as usize])
                    .expect("store the descriptor");

                Some(object.into())
            }

            // A `Void` field (roadmap Phase 4c, `Result<Void,E>`) has no
            // value to extract — the same reason a `Void` call's own result
            // is never inserted into `self.values` below.
            ir::InstKind::LoadField { .. } if instruction.ty == ir::IrType::Void => None,

            ir::InstKind::LoadField { object, index } => {
                // A record, value class or enum payload field comes straight
                // out of the value with `extractvalue`: there is no pointer
                // to GEP into (roadmap tasks 11.3/11.5). An enum's field
                // sits past its discriminant, the same way an object's sits
                // past its descriptor. Everything else keeps reading through
                // the object's address the way it always has.
                match self.value_types[&object.0] {
                    ir::IrType::Value(_) => {
                        let struct_value = self.operand(*object).into_struct_value();
                        Some(
                            self.builder
                                .build_extract_value(struct_value, *index, "field")
                                .expect("a verified module reads a field the layout has"),
                        )
                    }
                    ir::IrType::Enum(_) => {
                        let struct_value = self.operand(*object).into_struct_value();
                        Some(
                            self.builder
                                .build_extract_value(
                                    struct_value,
                                    index + ENUM_HEADER_FIELDS,
                                    "field",
                                )
                                .expect("a verified module reads a field the layout has"),
                        )
                    }
                    _ => {
                        let pointer = self.field_pointer(*object, *index);
                        let ty = llvm_type_in(
                            self.context,
                            instruction.ty,
                            &self.module.closures,
                            &self.module.values,
                            &self.module.enums,
                        )
                        .expect("a field is not Void");
                        Some(
                            self.builder
                                .build_load(ty, pointer, "field")
                                .expect("load a field"),
                        )
                    }
                }
            }

            ir::InstKind::BuildValue { class, fields } => {
                let layout = &self.module.values[*class as usize];
                let struct_type = value_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let mut built = struct_type.get_undef();
                for (index, field) in fields.iter().enumerate() {
                    built = self
                        .builder
                        .build_insert_value(
                            built,
                            self.operand(*field),
                            index as u32,
                            "value_field",
                        )
                        .expect("insert a value's field")
                        .into_struct_value();
                }
                Some(built.into())
            }

            ir::InstKind::BuildEnum {
                enum_id,
                variant,
                fields,
            } => {
                let layout = &self.module.enums[*enum_id as usize];
                let struct_type = enum_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let mut built = struct_type.get_undef();
                built = self
                    .builder
                    .build_insert_value(
                        built,
                        self.context
                            .i32_type()
                            .const_int(u64::from(*variant), false),
                        0,
                        "discriminant",
                    )
                    .expect("insert the discriminant")
                    .into_struct_value();
                let indices = &layout.variants[*variant as usize];
                for (&index, field) in indices.iter().zip(fields) {
                    // A `Void` field (roadmap Phase 4c, `Result<Void,E>`) has
                    // no operand to insert — its slot stays `undef`, which is
                    // sound: `LoadField`'s own `Void` guard never reads it.
                    if layout.fields[index as usize].ty == ir::IrType::Void {
                        continue;
                    }
                    built = self
                        .builder
                        .build_insert_value(
                            built,
                            self.operand(*field),
                            index + ENUM_HEADER_FIELDS,
                            "enum_field",
                        )
                        .expect("insert an enum's field")
                        .into_struct_value();
                }
                Some(built.into())
            }

            ir::InstKind::Discriminant(operand) => Some(
                self.builder
                    .build_extract_value(self.operand(*operand).into_struct_value(), 0, "tag")
                    .expect("a verified module reads the discriminant of an enum"),
            ),

            ir::InstKind::StoreField {
                object,
                index,
                value,
            } => {
                let pointer = self.field_pointer(*object, *index);
                self.builder
                    .build_store(pointer, self.operand(*value))
                    .expect("store a field");
                None
            }

            ir::InstKind::ConstString(id) => {
                // The literal becomes a `String` through the runtime: the
                // compiler never builds one itself (ADR-005).
                let pointer = self.strings[id.0 as usize];
                let length = self.module.strings[id.0 as usize].len();
                let length = self.context.i64_type().const_int(length as u64, false);

                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_from_utf8,
                        &[pointer.into(), length.into()],
                        "str",
                    )
                    .expect("call to the string constructor");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the constructor returns a value"),
                )
            }

            // A `Char` literal builds through the exact same runtime
            // constructor as a `String` one (ADR-014): the checker already
            // proved the text is one grapheme, so there is nothing left for
            // the runtime to validate at this call.
            ir::InstKind::ConstChar(id) => {
                let pointer = self.strings[id.0 as usize];
                let length = self.module.strings[id.0 as usize].len();
                let length = self.context.i64_type().const_int(length as u64, false);

                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_from_utf8,
                        &[pointer.into(), length.into()],
                        "char",
                    )
                    .expect("call to the character constructor");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the constructor returns a value"),
                )
            }

            // Byte length of the grapheme at `offset`, or `-1` past the end
            // (roadmap Phase 3b, task 6.3: `for ... in` over `String`).
            ir::InstKind::GraphemeLenAt { string, offset } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_grapheme_len_at,
                        &[self.operand(*string).into(), self.operand(*offset).into()],
                        "grapheme_len",
                    )
                    .expect("call to the grapheme length lookup");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the lookup returns a value"),
                )
            }

            // Builds a `Char` from a byte range already known to be one
            // grapheme.
            ir::InstKind::GraphemeSlice {
                string,
                offset,
                len,
            } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_grapheme_slice,
                        &[
                            self.operand(*string).into(),
                            self.operand(*offset).into(),
                            self.operand(*len).into(),
                        ],
                        "grapheme",
                    )
                    .expect("call to the grapheme slice constructor");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the constructor returns a value"),
                )
            }

            // `fatalError(message)` (roadmap Phase 4a): reports `message`
            // and terminates. No value: the caller's own `Terminator::Unreachable`
            // right after this (`lower_fatal_error_call`'s own doc comment)
            // is what actually stops codegen from emitting anything that
            // would run afterward, the same way `Println` produces nothing
            // either.
            ir::InstKind::FatalError(message) => {
                self.builder
                    .build_call(
                        self.runtime.fatal_error,
                        &[self.operand(*message).into()],
                        "",
                    )
                    .expect("call to the fatal error handler");
                None
            }

            ir::InstKind::Throw(exception) => {
                self.builder
                    .build_call(self.runtime.throw, &[self.operand(*exception).into()], "")
                    .expect("record the pending exception");
                None
            }

            ir::InstKind::HasPendingException => {
                let call = self
                    .builder
                    .build_call(self.runtime.has_pending_exception, &[], "has_pending")
                    .expect("query the pending exception");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::TakePendingException => {
                let call = self
                    .builder
                    .build_call(self.runtime.take_pending_exception, &[], "take_pending")
                    .expect("take the pending exception");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::Load(slot) => {
                let ty = llvm_type_in(
                    self.context,
                    instruction.ty,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a load cannot be Void");
                Some(
                    self.builder
                        .build_load(ty, self.slots[slot], "load")
                        .expect("slot load"),
                )
            }

            ir::InstKind::Store(slot, operand) => {
                self.builder
                    .build_store(self.slots[slot], self.operand(*operand))
                    .expect("slot store");
                None
            }

            ir::InstKind::Unary { op, operand } => {
                let signed = self.is_signed(self.value_types[&operand.0]);
                Some(self.emit_unary(*op, self.operand(*operand), signed, function))
            }

            ir::InstKind::Binary { op, left, right } => {
                // The left operand's own width/signedness — the checker
                // already required both sides to share it for every operator
                // this function still guards; a shift amount's own
                // (possibly different, roadmap Phase 3b task 4.3/4.4)
                // signedness is no longer read here — `zirk-ir`'s own
                // `Lowering::guard_shift` uses it upstream instead
                // (`fase-4d-runtimeerror`, design D10).
                let left_ty = self.value_types[&left.0];
                let signed = self.is_signed(left_ty);
                Some(self.emit_binary(
                    *op,
                    self.operand(*left),
                    self.operand(*right),
                    signed,
                    function,
                    left_ty,
                ))
            }

            ir::InstKind::Call { callee, args } => {
                let arguments: Vec<_> = args.iter().map(|a| self.operand(*a).into()).collect();
                let call = self
                    .builder
                    .build_call(self.functions[callee], &arguments, "call")
                    .expect("function call");
                call.try_as_basic_value().basic()
            }

            // Converts a native scalar to `String` (roadmap Phase 3b, task
            // 8) — always through the runtime, since the compiler does not
            // know how a `String` is built (ADR-005). A class/record/value
            // class with its own `to_string()` method never reaches this
            // instruction at all: `zirk-ir/lower.rs`'s `lower_to_string`
            // emits a real method call for those instead, the same shape any
            // other method call is (`lower_method_call`) — this dispatches
            // purely by the operand's own recorded width/signedness, the
            // same pattern `is_signed`/`emit_binary` already established.
            ir::InstKind::ToString(operand) => {
                // `Char` shares `String`'s representation bit for bit
                // (ADR-014): "converting" one is retagging the same pointer,
                // no runtime call at all.
                if self.value_types[&operand.0] == ir::IrType::Char {
                    Some(self.operand(*operand))
                } else {
                    let mut value = self.operand(*operand);
                    let converter = match self.value_types[&operand.0] {
                        ir::IrType::Boolean => self.runtime.str_from_bool,
                        ir::IrType::Int(width) => {
                            use ir::IntWidth::*;
                            match width {
                                I8 => self.runtime.str_from_i8,
                                U8 => self.runtime.str_from_u8,
                                I16 => self.runtime.str_from_i16,
                                U16 => self.runtime.str_from_u16,
                                I32 => self.runtime.str_from_i32,
                                U32 => self.runtime.str_from_u32,
                                I64 => self.runtime.str_from_i64,
                                U64 => self.runtime.str_from_u64,
                                I128 => self.runtime.str_from_i128,
                                U128 => self.runtime.str_from_u128,
                            }
                        }
                        // `Float16` has no stable Rust primitive to format
                        // through, but widening it to `Float32` first is
                        // always exact (`Type::accepts`'s float-widening
                        // rule) — every `f16` value is representable in
                        // `f32` without loss, so `f32`'s own `Display`
                        // prints the same value `f16` held, not an
                        // approximation of it.
                        ir::IrType::Float(ir::FloatWidth::F16) => {
                            value = self
                                .builder
                                .build_float_ext(
                                    value.into_float_value(),
                                    self.context.f32_type(),
                                    "to_string.f16_ext",
                                )
                                .expect("widen Float16 for printing")
                                .into();
                            self.runtime.str_from_f32
                        }
                        ir::IrType::Float(width) => match width {
                            ir::FloatWidth::F32 => self.runtime.str_from_f32,
                            ir::FloatWidth::F64 => self.runtime.str_from_f64,
                            // `F16` is caught by the dedicated arm above —
                            // never reaches here, but the match still needs
                            // it to stay exhaustive. `Float128` has no
                            // equivalent safe widening: an `f64` cannot
                            // represent every `f128` value exactly the way
                            // `f32` can every `f16`, so there is no honest
                            // conversion to fall back to. The checker's
                            // `is_printable` never accepts it (roadmap Phase
                            // 3b task 8.3's own documented gap), so a
                            // verified program never reaches this arm with
                            // one.
                            ir::FloatWidth::F16 | ir::FloatWidth::F128 => {
                                unreachable!("ToString over Float128")
                            }
                        },
                        // A `String` needs no conversion; the lowering does
                        // not emit `ToString` over one, so reaching here
                        // means malformed IR.
                        other => unreachable!("ToString over {other:?}"),
                    };

                    // `Int128`/`UInt128` cross the runtime boundary by
                    // pointer, not by value (no stable cross-target ABI for
                    // a by-value 128-bit integer — see the converter's own
                    // doc comment in `zirk-runtime/src/string.rs`).
                    let argument: BasicMetadataValueEnum = if matches!(
                        self.value_types[&operand.0],
                        ir::IrType::Int(ir::IntWidth::I128 | ir::IntWidth::U128)
                    ) {
                        let int_value = value.into_int_value();
                        let slot = self
                            .builder
                            .build_alloca(int_value.get_type(), "to_string.i128")
                            .expect("stack slot for a 128-bit conversion");
                        self.builder
                            .build_store(slot, int_value)
                            .expect("store the value to convert");
                        slot.into()
                    } else {
                        value.into()
                    };

                    let call = self
                        .builder
                        .build_call(converter, &[argument], "str")
                        .expect("call to the converter");
                    Some(
                        call.try_as_basic_value()
                            .basic()
                            .expect("the converter returns a value"),
                    )
                }
            }

            ir::InstKind::Println(operand) => {
                self.builder
                    .build_call(
                        self.runtime.io_println,
                        &[self.operand(*operand).into()],
                        "",
                    )
                    .expect("call to println");
                None
            }

            // A nullable value is `{ i1 present, T value }`. The absent form
            // still carries a payload slot, left undefined: nothing reads it
            // without checking the flag first, and the verifier enforces that.
            ir::InstKind::NullValue(base) => {
                let ty = llvm_type_in(
                    self.context,
                    ir::IrType::Nullable(*base),
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a nullable type has a representation")
                .into_struct_type();
                Some(ty.get_undef().into()).map(|value: BasicValueEnum| {
                    self.builder
                        .build_insert_value(
                            value.into_struct_value(),
                            self.context.bool_type().const_zero(),
                            0,
                            "absent",
                        )
                        .expect("present flag")
                        .as_basic_value_enum()
                })
            }

            ir::InstKind::Wrap { base, value } => {
                let ty = llvm_type_in(
                    self.context,
                    ir::IrType::Nullable(*base),
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a nullable type has a representation")
                .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(
                        ty.get_undef(),
                        self.context.bool_type().const_int(1, false),
                        0,
                        "present",
                    )
                    .expect("present flag");
                Some(
                    self.builder
                        .build_insert_value(
                            with_flag.into_struct_value(),
                            self.operand(*value),
                            1,
                            "wrapped",
                        )
                        .expect("payload")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::IsNull(operand) => {
                let present = self
                    .builder
                    .build_extract_value(self.operand(*operand).into_struct_value(), 0, "present")
                    .expect("present flag");
                Some(
                    self.builder
                        .build_not(present.into_int_value(), "absent")
                        .expect("negation")
                        .into(),
                )
            }

            ir::InstKind::Unwrap(operand) => Some(
                self.builder
                    .build_extract_value(self.operand(*operand).into_struct_value(), 1, "unwrapped")
                    .expect("payload"),
            ),

            ir::InstKind::MakeClosure {
                id,
                captures,
                target,
            } => {
                let ty = llvm_type_in(
                    self.context,
                    ir::IrType::Closure(*id),
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a closure has a representation")
                .into_struct_type();

                let target = self.functions[target.as_str()];

                let mut value = self
                    .builder
                    .build_insert_value(
                        ty.get_undef(),
                        target.as_global_value().as_pointer_value(),
                        0,
                        "fn",
                    )
                    .expect("function pointer")
                    .into_struct_value();

                for (index, capture) in captures.iter().enumerate() {
                    value = self
                        .builder
                        .build_insert_value(
                            value,
                            self.operand(*capture),
                            index as u32 + 1,
                            "capture",
                        )
                        .expect("capture")
                        .into_struct_value();
                }

                Some(value.into())
            }

            ir::InstKind::CallClosure { id, callee, args } => {
                let layout = self.module.closures[*id as usize].clone();
                let value = self.operand(*callee).into_struct_value();

                let pointer = self
                    .builder
                    .build_extract_value(value, 0, "fn")
                    .expect("function pointer")
                    .into_pointer_value();

                // The captures travel inside the value and go ahead of the
                // arguments, which is the order the lifted body declares.
                let mut arguments: Vec<BasicMetadataValueEnum> = Vec::new();
                for index in 0..layout.captures.len() {
                    arguments.push(
                        self.builder
                            .build_extract_value(value, index as u32 + 1, "capture")
                            .expect("capture")
                            .into(),
                    );
                }
                for arg in args {
                    arguments.push(self.operand(*arg).into());
                }

                let signature = closure_signature(
                    self.context,
                    &layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let call = self
                    .builder
                    .build_indirect_call(signature, pointer, &arguments, "closure")
                    .expect("indirect call");

                call.try_as_basic_value().basic()
            }

            // `Pointer.from(place)` (roadmap Phase 4e, design D8): the
            // already-computed `alloca` of the slot — no new storage.
            ir::InstKind::PointerFromSlot(slot) => Some(self.slots[slot].into()),

            // `Pointer.from(place)` where `place` is a field (design D8):
            // the field's own already-computed GEP. Only an `Object`
            // receiver is supported by this slice — a record/value class's
            // field has no address of its own to take without deciding
            // where the value itself lives first, which is out of scope
            // here (the checker's escape rule and the FFI-safe element
            // restriction already keep this narrow in practice).
            ir::InstKind::PointerFromField { object, index } => {
                Some(self.field_pointer(*object, *index).into())
            }

            // `.read()` (design D8): a plain LLVM `load`, typed by `T`.
            ir::InstKind::PointerRead(pointer) => {
                let element_ty = self
                    .llvm_type(instruction.ty)
                    .expect("Pointer<Void> is not constructed by this slice");
                Some(
                    self.builder
                        .build_load(
                            element_ty,
                            self.operand(*pointer).into_pointer_value(),
                            "ptr_read",
                        )
                        .expect("pointer read"),
                )
            }

            // `.write(value)` (design D8): a plain LLVM `store`.
            ir::InstKind::PointerWrite { pointer, value } => {
                self.builder
                    .build_store(
                        self.operand(*pointer).into_pointer_value(),
                        self.operand(*value),
                    )
                    .expect("pointer write");
                None
            }

            // `.offset(n)` (design D8): `getelementptr` in units of `T`.
            ir::InstKind::PointerOffset { pointer, amount } => {
                let ir::IrType::Pointer(id) = instruction.ty else {
                    unreachable!("a verified module types a pointer offset as a Pointer")
                };
                let element_ty = self
                    .llvm_type(self.module.pointer_types[id as usize])
                    .expect("Pointer<Void> is not constructed by this slice");
                let offset = unsafe {
                    self.builder
                        .build_gep(
                            element_ty,
                            self.operand(*pointer).into_pointer_value(),
                            &[self.operand(*amount).into_int_value()],
                            "ptr_offset",
                        )
                        .expect("pointer offset")
                };
                Some(offset.into())
            }

            // `.offset_bytes(n)` (design D8): `getelementptr` over an
            // `i8`-typed view of the same pointer.
            ir::InstKind::PointerOffsetBytes { pointer, amount } => {
                let byte_ty = self.context.i8_type();
                let offset = unsafe {
                    self.builder
                        .build_gep(
                            byte_ty,
                            self.operand(*pointer).into_pointer_value(),
                            &[self.operand(*amount).into_int_value()],
                            "ptr_offset_bytes",
                        )
                        .expect("pointer byte offset")
                };
                Some(offset.into())
            }

            // `.cast<U>()` (design D8): an LLVM pointer bitcast. Under
            // LLVM's opaque-pointer model a `ptr` value carries no pointee
            // type of its own, so the operand is reused unchanged — only
            // its declared `IrType` differs from here on.
            ir::InstKind::PointerCast(operand) => Some(self.operand(*operand)),

            // `.is_null` (roadmap Phase 4e): the one pointer operation with
            // no `unsafe` requirement.
            ir::InstKind::PointerIsNull(operand) => {
                let ptr = self.operand(*operand).into_pointer_value();
                Some(
                    self.builder
                        .build_is_null(ptr, "is_null")
                        .expect("pointer null check")
                        .into(),
                )
            }
        };

        if let (Some(result), Some(value)) = (instruction.result, value) {
            self.values.insert(result, value);
            // An object is an opaque pointer once emitted, so which layout it
            // belongs to has to be remembered from the IR: it is what a field
            // access needs to know where to point.
            self.value_types.insert(result, instruction.ty);
        }
    }

    /// Whether an `IrType::Int` is signed — `false` for anything else,
    /// which never matters: only an integer operand ever asks.
    fn is_signed(&self, ty: ir::IrType) -> bool {
        matches!(ty, ir::IrType::Int(width) if width.signed())
    }

    /// LLVM's native float type for a given IR width.
    fn float_type(&self, width: ir::FloatWidth) -> FloatType<'ctx> {
        match width {
            ir::FloatWidth::F16 => self.context.f16_type(),
            ir::FloatWidth::F32 => self.context.f32_type(),
            ir::FloatWidth::F64 => self.context.f64_type(),
            ir::FloatWidth::F128 => self.context.f128_type(),
        }
    }

    fn emit_unary(
        &mut self,
        op: ir::UnaryOp,
        operand: BasicValueEnum<'ctx>,
        signed: bool,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        // A `Float` operand only ever reaches `Neg` (`~`/`!` are integer- and
        // Boolean-only, per the checker) — flipping the sign bit cannot turn
        // a non-`NaN` input into `NaN`, so unlike the binary arithmetic
        // operators below, this needs no post-check (design D2).
        if operand.is_float_value() {
            let value = operand.into_float_value();
            return self
                .builder
                .build_float_neg(value, "fneg")
                .expect("float negation")
                .into();
        }

        match op {
            // Negation is a subtraction from zero, so it goes through the same
            // overflow check: `-Int32.MIN` does not fit in Int32. The
            // checker only ever lets this reach a signed width (roadmap
            // Phase 3b): negating unsigned has no result in its own type.
            ir::UnaryOp::Neg => {
                debug_assert!(signed, "the checker only allows `-` on a signed width");
                let value = operand.into_int_value();
                let zero = value.get_type().const_zero();
                self.checked_arithmetic("ssub", zero, value, function)
            }
            // `not` is bitwise complement at the LLVM level regardless of
            // whether the checker calls it logical negation or `~`: `!true`
            // and `~5` are the same instruction on different widths.
            ir::UnaryOp::Not | ir::UnaryOp::BitNot => self
                .builder
                .build_not(operand.into_int_value(), "not")
                .expect("bitwise complement")
                .into(),
        }
    }

    fn emit_binary(
        &mut self,
        op: ir::BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        signed: bool,
        function: FunctionValue<'ctx>,
        left_ty: ir::IrType,
    ) -> BasicValueEnum<'ctx> {
        use ir::BinaryOp::*;

        // `is` between two closure values (roadmap Phase 4d, design D15,
        // `CORE_LANGUAGE_SEMANTICS.md`: "callable identity uses `is`")
        // compares the whole `{function pointer, capture...}` struct
        // field-by-field — the function pointer by address, each capture
        // recursively through this same dispatcher (so a captured closure,
        // or a captured `T?`, compares the same way it would on its own).
        // Generalizes the same struct-shaped `is` pattern `fase-4c` already
        // built for `T?` right below.
        if op == Identical && left.is_struct_value() && matches!(left_ty, ir::IrType::Closure(_)) {
            return self.identical_value(left, right, left_ty).into();
        }

        // `is` between two nullable references (`Node? is Node?`, or `T is T?`
        // once `Self::lower_coalesce`'s sibling in `zirk-ir/lower.rs` has
        // widened the narrower side) sees the `{i1, ptr}` struct a `T?`
        // lowers to, not a bare pointer — `left.is_pointer_value()` below is
        // false for it, and falling through to the integer/float paths
        // panicked on the struct. Two absent references are identical to
        // each other; an absent one is identical to nothing present; two
        // present ones compare by address exactly like the non-nullable case.
        if op == Identical && left.is_struct_value() {
            let l = left.into_struct_value();
            let r = right.into_struct_value();
            let l_present = self
                .builder
                .build_extract_value(l, 0, "l_present")
                .expect("nullable present flag")
                .into_int_value();
            let r_present = self
                .builder
                .build_extract_value(r, 0, "r_present")
                .expect("nullable present flag")
                .into_int_value();
            let l_ptr = self
                .builder
                .build_extract_value(l, 1, "l_ptr")
                .expect("nullable payload")
                .into_pointer_value();
            let r_ptr = self
                .builder
                .build_extract_value(r, 1, "r_ptr")
                .expect("nullable payload")
                .into_pointer_value();
            let l_addr = self
                .builder
                .build_ptr_to_int(l_ptr, self.context.i64_type(), "lhs")
                .expect("compare addresses");
            let r_addr = self
                .builder
                .build_ptr_to_int(r_ptr, self.context.i64_type(), "rhs")
                .expect("compare addresses");
            let same_presence = self
                .builder
                .build_int_compare(IntPredicate::EQ, l_present, r_present, "same_presence")
                .expect("compare presence");
            let same_address = self
                .compare(IntPredicate::EQ, l_addr, r_addr)
                .into_int_value();
            // When both are absent the address comparison is over whatever
            // garbage the payload holds, so it is only consulted when `l` is
            // actually present — `same_presence` already covers the
            // both-absent and mixed cases on its own.
            let identical_when_present = self
                .builder
                .build_or(
                    self.builder
                        .build_not(l_present, "l_absent")
                        .expect("negate"),
                    same_address,
                    "identical_when_present",
                )
                .expect("combine");
            return self
                .builder
                .build_and(same_presence, identical_when_present, "identical")
                .expect("combine")
                .into();
        }

        // `ZIRK_LANGUAGE_SPEC.md` section 4: `==` compares structurally. Over a
        // `String` the operands are opaque handles, so comparing them as
        // integers would compare identity — which is what `is` means, not `==`.
        if left.is_pointer_value() {
            // `is` compares the references themselves, whatever they point at:
            // identity is the address, so there is nothing to call into.
            if op == ir::BinaryOp::Identical {
                let l = self
                    .builder
                    .build_ptr_to_int(left.into_pointer_value(), self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r = self
                    .builder
                    .build_ptr_to_int(right.into_pointer_value(), self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                return self.compare(IntPredicate::EQ, l, r);
            }
            return self.compare_strings(op, left, right);
        }

        if left.is_float_value() {
            return self.emit_float_binary(op, left.into_float_value(), right.into_float_value());
        }

        let l = left.into_int_value();
        let r = right.into_int_value();

        match op {
            // `ZIRK_LANGUAGE_SPEC.md` section 3: ordinary overflow produces a
            // controlled error. The wrapping variants are explicit operations
            // that do not exist in this subset, so every arithmetic operation
            // is checked.
            Add => self.checked_arithmetic(if signed { "sadd" } else { "uadd" }, l, r, function),
            Sub => self.checked_arithmetic(if signed { "ssub" } else { "usub" }, l, r, function),
            Mul => self.checked_arithmetic(if signed { "smul" } else { "umul" }, l, r, function),

            Div | Rem => self.checked_division(op, l, r, signed, function),

            Eq => self.compare(IntPredicate::EQ, l, r),
            NotEq => self.compare(IntPredicate::NE, l, r),
            Lt => self.compare(
                if signed {
                    IntPredicate::SLT
                } else {
                    IntPredicate::ULT
                },
                l,
                r,
            ),
            LtEq => self.compare(
                if signed {
                    IntPredicate::SLE
                } else {
                    IntPredicate::ULE
                },
                l,
                r,
            ),
            Gt => self.compare(
                if signed {
                    IntPredicate::SGT
                } else {
                    IntPredicate::UGT
                },
                l,
                r,
            ),
            GtEq => self.compare(
                if signed {
                    IntPredicate::SGE
                } else {
                    IntPredicate::UGE
                },
                l,
                r,
            ),
            // A value has no identity to compare, and the checker said so.
            Identical => unreachable!("`is` needs a reference"),

            // `&&` and `||` do not short-circuit in this phase: both operands
            // were already evaluated when the IR was lowered. Short-circuiting
            // needs its own blocks and is Phase 2, together with `if` as an
            // expression.
            And => self
                .builder
                .build_and(l, r, "and")
                .expect("conjunction")
                .into(),
            Or => self
                .builder
                .build_or(l, r, "or")
                .expect("disjunction")
                .into(),

            BitAnd => self
                .builder
                .build_and(l, r, "bitand")
                .expect("bitand")
                .into(),
            BitOr => self.builder.build_or(l, r, "bitor").expect("bitor").into(),
            BitXor => self
                .builder
                .build_xor(l, r, "bitxor")
                .expect("bitxor")
                .into(),

            Shl => self.checked_shift(op, l, r, signed),
            Shr => self.checked_shift(op, l, r, signed),
        }
    }

    /// `is` over one value of any type — used both at the top of
    /// `Self::emit_binary` and recursively over a closure's own capture
    /// fields (design D15), which may themselves be of any type the
    /// language admits inside a closure's environment (an `Int32`, a
    /// `String`, another closure, a nullable reference, ...). Unlike
    /// `Self::emit_binary`'s own `Identical` arm — reachable only where the
    /// checker already proved the *top-level* operand has identity — a
    /// capture field has no such guarantee from the checker (it never
    /// separately typechecks `is` over a capture on its own), so every
    /// shape is handled here rather than assuming one.
    fn identical_value(
        &mut self,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        ty: ir::IrType,
    ) -> inkwell::values::IntValue<'ctx> {
        match ty {
            ir::IrType::Closure(id) => {
                let l = left.into_struct_value();
                let r = right.into_struct_value();
                let layout = self.module.closures[id as usize].clone();

                let l_fn = self
                    .builder
                    .build_extract_value(l, 0, "l_fn")
                    .expect("function pointer")
                    .into_pointer_value();
                let r_fn = self
                    .builder
                    .build_extract_value(r, 0, "r_fn")
                    .expect("function pointer")
                    .into_pointer_value();
                let l_addr = self
                    .builder
                    .build_ptr_to_int(l_fn, self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r_addr = self
                    .builder
                    .build_ptr_to_int(r_fn, self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                let mut identical = self
                    .compare(IntPredicate::EQ, l_addr, r_addr)
                    .into_int_value();

                for (index, capture_ty) in layout.captures.iter().enumerate() {
                    let field = index as u32 + 1;
                    let l_field = self
                        .builder
                        .build_extract_value(l, field, "l_capture")
                        .expect("capture");
                    let r_field = self
                        .builder
                        .build_extract_value(r, field, "r_capture")
                        .expect("capture");
                    let field_identical = self.identical_value(l_field, r_field, *capture_ty);
                    identical = self
                        .builder
                        .build_and(identical, field_identical, "identical")
                        .expect("combine");
                }

                identical
            }
            // `T?`: absent-vs-absent is identical, absent-vs-present is not,
            // two present ones compare by address (`fase-4c`'s own fix for
            // `is` over a nullable reference, generalized here to a nested
            // position instead of only the top level).
            ir::IrType::Nullable(_) => {
                let l = left.into_struct_value();
                let r = right.into_struct_value();
                let l_present = self
                    .builder
                    .build_extract_value(l, 0, "l_present")
                    .expect("nullable present flag")
                    .into_int_value();
                let r_present = self
                    .builder
                    .build_extract_value(r, 0, "r_present")
                    .expect("nullable present flag")
                    .into_int_value();
                let l_ptr = self
                    .builder
                    .build_extract_value(l, 1, "l_ptr")
                    .expect("nullable payload")
                    .into_pointer_value();
                let r_ptr = self
                    .builder
                    .build_extract_value(r, 1, "r_ptr")
                    .expect("nullable payload")
                    .into_pointer_value();
                let l_addr = self
                    .builder
                    .build_ptr_to_int(l_ptr, self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r_addr = self
                    .builder
                    .build_ptr_to_int(r_ptr, self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                let same_presence = self
                    .builder
                    .build_int_compare(IntPredicate::EQ, l_present, r_present, "same_presence")
                    .expect("compare presence");
                let same_address = self
                    .compare(IntPredicate::EQ, l_addr, r_addr)
                    .into_int_value();
                let identical_when_present = self
                    .builder
                    .build_or(
                        self.builder
                            .build_not(l_present, "l_absent")
                            .expect("negate"),
                        same_address,
                        "identical_when_present",
                    )
                    .expect("combine");
                self.builder
                    .build_and(same_presence, identical_when_present, "identical")
                    .expect("combine")
            }
            _ if left.is_pointer_value() => {
                let l = self
                    .builder
                    .build_ptr_to_int(left.into_pointer_value(), self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r = self
                    .builder
                    .build_ptr_to_int(right.into_pointer_value(), self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                self.compare(IntPredicate::EQ, l, r).into_int_value()
            }
            _ if left.is_float_value() => self
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    left.into_float_value(),
                    right.into_float_value(),
                    "identical",
                )
                .expect("compare floats"),
            _ => self
                .compare(
                    IntPredicate::EQ,
                    left.into_int_value(),
                    right.into_int_value(),
                )
                .into_int_value(),
        }
    }

    /// Structural equality between strings, through the runtime.
    fn compare_strings(
        &self,
        op: ir::BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let equal = self
            .builder
            .build_call(self.runtime.str_eq, &[left.into(), right.into()], "streq")
            .expect("call to string equality")
            .try_as_basic_value()
            .basic()
            .expect("equality returns a value")
            .into_int_value();

        match op {
            ir::BinaryOp::Eq => equal.into(),
            ir::BinaryOp::NotEq => self
                .builder
                .build_not(equal, "strneq")
                .expect("negation")
                .into(),
            // The checker only admits `==` and `!=` between strings: ordering
            // needs a comparison contract of the type, which is Phase 3.
            other => unreachable!("operator {other:?} over String"),
        }
    }

    fn compare(
        &self,
        predicate: IntPredicate,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        self.builder
            .build_int_compare(predicate, left, right, "cmp")
            .expect("comparison")
            .into()
    }

    /// Arithmetic with overflow detection through LLVM intrinsics.
    ///
    /// The intrinsic returns `{result, overflowed}`. If it overflowed, control
    /// transfers to the runtime, which reports and terminates; otherwise
    /// execution continues with the result.
    fn checked_arithmetic(
        &mut self,
        operation: &str,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let intrinsic =
            inkwell::intrinsics::Intrinsic::find(&format!("llvm.{operation}.with.overflow"))
                .expect("LLVM provides the overflow intrinsics");

        // The overflow intrinsics are parameterized by width alone — `left`
        // already carries the operand's real one (roadmap Phase 3b); the
        // signed/unsigned choice lives in `operation`'s own name (`sadd` vs
        // `uadd`, chosen by the caller), not in this declaration.
        let declaration = intrinsic
            .get_declaration(self.llvm, &[left.get_type().into()])
            .expect("the intrinsic accepts this integer width");

        let call = self
            .builder
            .build_call(declaration, &[left.into(), right.into()], "arith")
            .expect("call to the intrinsic")
            .try_as_basic_value()
            .basic()
            .expect("the intrinsic returns a struct");

        let result = self
            .builder
            .build_extract_value(call.into_struct_value(), 0, "value")
            .expect("result of the operation");
        let overflowed = self
            .builder
            .build_extract_value(call.into_struct_value(), 1, "overflowed")
            .expect("overflow flag");

        self.trap_if(overflowed.into_int_value(), self.runtime.overflow, function);
        result
    }

    /// Division and remainder, checking the one remaining overflow case.
    ///
    /// The divisor is no longer checked here (`fase-4d-runtimeerror`, design
    /// D10): `zirk-ir/src/lower.rs`'s own `Lowering::guard_division` now
    /// compares it to zero and throws `DivisionByZeroError` *before* this
    /// point is ever reached, as an ordinary `Branch` in the IR rather than a
    /// codegen-level `trap_if` — a `sdiv`/`udiv` by zero here would be
    /// undefined behaviour in LLVM (`ZIRK_LANGUAGE_SPEC.md` section 9), but
    /// the guard upstream already guarantees `right` is nonzero whenever
    /// this function runs.
    ///
    /// `Int32::MIN / -1` is a separate case this function still guards
    /// directly: its result is one past the maximum, so it overflows, and in
    /// LLVM it is undefined rather than wrapping. `OverflowError` is out of
    /// this pass's scope (`proposal.md`), so it keeps aborting via
    /// `trap_if`/`fatalError`, unchanged.
    fn checked_division(
        &mut self,
        op: ir::BinaryOp,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        signed: bool,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let int_ty = left.get_type();

        // The one pair that overflows a division: `MIN / -1`, one past the
        // widest value the width can hold. Unsigned has no such minimum —
        // its `MIN` is `0`, and `0 / anything nonzero` never overflows — so
        // the check only applies to a signed width.
        if signed {
            // The signed minimum's bit pattern is a lone `1` at the top bit
            // — `2^(bits-1)` — built from 64-bit words since `Int128`
            // itself does not fit in one (`const_int_arbitrary_precision`,
            // little-endian words).
            let bits = int_ty.get_bit_width();
            let words: Vec<u64> = if bits <= 64 {
                vec![1u64 << (bits - 1)]
            } else {
                vec![0, 1u64 << (bits - 65)]
            };
            let min = int_ty.const_int_arbitrary_precision(&words);
            let minus_one = int_ty.const_all_ones();

            let left_is_min = self
                .builder
                .build_int_compare(IntPredicate::EQ, left, min, "is_min")
                .expect("dividend comparison");
            let right_is_minus_one = self
                .builder
                .build_int_compare(IntPredicate::EQ, right, minus_one, "is_minus_one")
                .expect("divisor comparison");
            let overflows = self
                .builder
                .build_and(left_is_min, right_is_minus_one, "div_overflows")
                .expect("conjunction");

            self.trap_if(overflows, self.runtime.overflow, function);
        }

        match (op, signed) {
            (ir::BinaryOp::Div, true) => self
                .builder
                .build_int_signed_div(left, right, "div")
                .expect("division")
                .into(),
            (ir::BinaryOp::Div, false) => self
                .builder
                .build_int_unsigned_div(left, right, "div")
                .expect("division")
                .into(),
            (_, true) => self
                .builder
                .build_int_signed_rem(left, right, "rem")
                .expect("remainder")
                .into(),
            (_, false) => self
                .builder
                .build_int_unsigned_rem(left, right, "rem")
                .expect("remainder")
                .into(),
        }
    }

    /// `<<`/`>>`.
    ///
    /// No longer checked here (`fase-4d-runtimeerror`, design D10): an
    /// amount outside `0..bits` of the *shifted* operand's own width — the
    /// one thing that makes either shift undefined at the LLVM level — is
    /// now checked upstream, in `zirk-ir/src/lower.rs`'s own
    /// `Lowering::guard_shift`, as an ordinary IR `Branch` that throws
    /// `InvalidShiftError` before this point is ever reached, in place of
    /// this function's own former `trap_if`.
    fn checked_shift(
        &mut self,
        op: ir::BinaryOp,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        signed: bool,
    ) -> BasicValueEnum<'ctx> {
        match op {
            ir::BinaryOp::Shl => self
                .builder
                .build_left_shift(left, right, "shl")
                .expect("left shift")
                .into(),
            // A logical shift for unsigned, arithmetic (sign-extending) for
            // signed — the value being shifted decides, not the amount.
            _ => self
                .builder
                .build_right_shift(left, right, signed, "shr")
                .expect("right shift")
                .into(),
        }
    }

    /// Arithmetic and comparison over `Float` operands (roadmap Phase 3b).
    ///
    /// Unlike integer arithmetic, there is no width/signedness table to pick
    /// an intrinsic from: LLVM's `fadd`/`fsub`/`fmul`/`fdiv`/`frem` already
    /// work over any float width uniformly. An overflowing result becomes a
    /// valid infinity (design D2) — only a result IEEE 754 itself defines as
    /// `NaN` is a controlled error, and that is no longer checked here:
    /// `zirk-ir/src/lower.rs`'s own `Lowering::guard_nan` now compares the
    /// already-lowered `InstKind::Binary` result against itself and throws
    /// `FloatNanError` after this operation runs, as an ordinary IR
    /// `Branch` rather than this function's own former `trap_if`
    /// (`fase-4d-runtimeerror`, design D10).
    fn emit_float_binary(
        &mut self,
        op: ir::BinaryOp,
        left: FloatValue<'ctx>,
        right: FloatValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        use ir::BinaryOp::*;

        match op {
            Add => self
                .builder
                .build_float_add(left, right, "fadd")
                .expect("addition")
                .into(),
            Sub => self
                .builder
                .build_float_sub(left, right, "fsub")
                .expect("subtraction")
                .into(),
            Mul => self
                .builder
                .build_float_mul(left, right, "fmul")
                .expect("multiplication")
                .into(),
            Div => self
                .builder
                .build_float_div(left, right, "fdiv")
                .expect("division")
                .into(),
            Rem => self
                .builder
                .build_float_rem(left, right, "frem")
                .expect("remainder")
                .into(),
            Eq => self.compare_float(FloatPredicate::OEQ, left, right),
            NotEq => self.compare_float(FloatPredicate::ONE, left, right),
            Lt => self.compare_float(FloatPredicate::OLT, left, right),
            LtEq => self.compare_float(FloatPredicate::OLE, left, right),
            Gt => self.compare_float(FloatPredicate::OGT, left, right),
            GtEq => self.compare_float(FloatPredicate::OGE, left, right),
            // The checker never lets a `Float` operand reach `is`, a logical
            // operator, bitwise or shift.
            Identical | And | Or | BitAnd | BitOr | BitXor | Shl | Shr => {
                unreachable!("operator {op:?} over Float")
            }
        }
    }

    fn compare_float(
        &self,
        predicate: FloatPredicate,
        left: FloatValue<'ctx>,
        right: FloatValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        self.builder
            .build_float_compare(predicate, left, right, "fcmp")
            .expect("comparison")
            .into()
    }

    /// Transfers control to the runtime when a condition holds.
    ///
    /// Splits the current block: the failure branch calls the runtime and ends
    /// in `unreachable`, because the runtime does not return; the other branch
    /// continues normally and becomes the block instructions keep landing in.
    fn trap_if(
        &mut self,
        condition: inkwell::values::IntValue<'ctx>,
        handler: FunctionValue<'ctx>,
        function: FunctionValue<'ctx>,
    ) {
        let failure = self.context.append_basic_block(function, "trap");
        let continuation = self.context.append_basic_block(function, "cont");

        self.builder
            .build_conditional_branch(condition, failure, continuation)
            .expect("conditional branch");

        self.builder.position_at_end(failure);
        self.builder
            .build_call(handler, &[], "")
            .expect("call to the runtime handler");
        self.builder
            .build_unreachable()
            .expect("the handler does not return");

        self.builder.position_at_end(continuation);
    }

    /// Design D2: pops this activation's shadow-stack frame, pushed once at
    /// entry — called right before every `Terminator::Return` lowers to
    /// `ret`, the only way a function ends in this IR (roadmap Phase 4b's
    /// own exceptions propagate through an explicit pending-exception check
    /// and an ordinary early `Return`, never LLVM unwind tables — so this
    /// one call site covers every exit path, early returns included).
    fn pop_gc_frame(&self) {
        self.builder
            .build_call(self.runtime.pop_frame, &[], "")
            .expect("pop the gc frame");
    }

    fn emit_terminator(&mut self, terminator: &ir::Terminator) {
        match terminator {
            // LLVM has this exact concept, so nothing is invented here.
            ir::Terminator::Unreachable => {
                self.builder
                    .build_unreachable()
                    .expect("unreachable terminator");
            }
            ir::Terminator::Return(None) => {
                self.pop_gc_frame();
                self.builder.build_return(None).expect("empty return");
            }
            ir::Terminator::Return(Some(operand)) => {
                let value = self.operand(*operand);
                self.pop_gc_frame();
                self.builder
                    .build_return(Some(&value as &dyn BasicValue))
                    .expect("return with a value");
            }
            ir::Terminator::Jump(target) => {
                self.builder
                    .build_unconditional_branch(self.blocks[target])
                    .expect("jump");
            }
            ir::Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                self.builder
                    .build_conditional_branch(
                        self.operand(*condition).into_int_value(),
                        self.blocks[then_block],
                        self.blocks[else_block],
                    )
                    .expect("conditional branch");
            }
        }
    }
}
