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
use inkwell::debug_info::{
    AsDIScope, DICompileUnit, DIFile, DIFlags, DIFlagsConstants, DILocalVariable, DIScope,
    DISubprogram, DIType, DWARFEmissionKind, DWARFSourceLanguage, DebugInfoBuilder,
};
use inkwell::module::{FlagBehavior, Linkage, Module as LlvmModule};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FloatType};
use inkwell::values::{
    AsValueRef, BasicMetadataValueEnum, BasicValue, BasicValueEnum, FloatValue, FunctionValue,
    IntValue, PointerValue,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};
use llvm_sys::core::LLVMSetIsNewDbgInfoFormat;
use llvm_sys::debuginfo::LLVMDIBuilderInsertDeclareRecordBefore;
use std::collections::HashMap;
use std::path::Path;
use zirk_diagnostics::{FileId, SourceMap, Span};
use zirk_ir as ir;

fn filesystem_operation(binding: &str) -> Option<i32> {
    Some(match binding {
        "std.path.Path.join" => 1,
        "std.path.Path.components" => 2,
        "std.path.Path.parent" => 3,
        "std.path.Path.name" => 4,
        "std.path.Path.normalize" => 9,
        "std.path.Path.absolute" => 10,
        "std.path.Path.canonicalize" => 11,
        "std.fs.File.read_bytes" => 100,
        "std.fs.File.read_text" => 101,
        "std.fs.File.write_bytes" => 102,
        "std.fs.File.write_text" => 103,
        "std.fs.File.open" => 104,
        "std.fs.File.close" => 105,
        "std.fs.File.is_closed" => 106,
        "std.fs.File.read_bytes_handle" => 107,
        "std.fs.File.read_exact" => 108,
        "std.fs.File.read_line" => 109,
        "std.fs.File.append_bytes" => 110,
        "std.fs.File.append_text" => 111,
        "std.fs.File.append_handle" => 112,
        "std.fs.File.metadata" => 113,
        "std.fs.File.exists" => 114,
        "std.fs.File.write_bytes_atomic" => 115,
        "std.fs.File.write_text_atomic" => 116,
        "std.fs.File.metadata_for" => 117,
        "std.fs.Directory.create" => 200,
        "std.fs.Directory.create_all" => 201,
        "std.fs.Directory.entries" => 202,
        "std.fs.DirectoryEntries.next_entry" => 203,
        "std.fs.DirectoryEntries.close" => 204,
        "std.fs.Directory.walk" => 205,
        "std.fs.DirectoryWalker.next_entry" => 206,
        "std.fs.DirectoryWalker.close" => 207,
        "std.fs.Directory.is_empty" => 208,
        "std.fs.Directory.size" => 209,
        "std.fs.Directory.copy" => 210,
        "std.fs.Directory.move" => 211,
        "std.fs.Directory.remove" => 212,
        "std.fs.Directory.remove_all" => 213,
        "std.fs.Directory.watch" => 214,
        "std.fs.FileWatcher.next" => 215,
        "std.fs.FileWatcher.next_async" => 216,
        "std.fs.FileWatcher.close" => 217,
        "std.fs.Link.is_link" => 220,
        "std.fs.Link.read_target" => 221,
        "std.fs.Link.create" => 222,
        "std.fs.File.set_readonly" => 230,
        "std.fs.Temporary.file" => 240,
        "std.fs.Temporary.directory" => 241,
        "std.fs.TemporaryPath.path" => 242,
        "std.fs.TemporaryPath.close" => 243,
        "std.fs.TemporaryPath.release" => 244,
        "std.fs.File.lock" => 250,
        "std.fs.File.unlock" => 251,
        "std.fs.File.lock_async" => 252,
        "std.fs.File.read_bytes_async" => 300,
        "std.fs.File.read_text_async" => 301,
        "std.fs.File.write_bytes_async" => 302,
        "std.fs.File.write_text_async" => 303,
        "std.fs.File.open_async" => 304,
        "std.fs.File.read_async" => 305,
        "std.fs.File.write_async" => 306,
        "std.fs.File.read_exact_async" => 307,
        "std.fs.File.append_bytes_async" => 308,
        "std.fs.File.append_text_async" => 309,
        "std.fs.File.append_async" => 310,
        "std.fs.Directory.entries_async" => 320,
        "std.fs.DirectoryEntries.next_entry_async" => 321,
        "std.fs.Directory.walk_async" => 322,
        "std.fs.DirectoryWalker.next_entry_async" => 323,
        _ => return None,
    })
}

/// Prefix applied to every Zirk function in the generated code.
///
/// It avoids collisions with C symbols — a Zirk function named `printf` must
/// not become one — and frees the name `main` for the C entrypoint the runtime
/// needs. The scheme is internal and will be revisited when packages arrive in
/// Phase 8.
const FUNCTION_PREFIX: &str = "zk_";

/// Generated application body. Source-level `main` is an ordinary function.
const ENTRYPOINT: &str = "$zirk.entry";

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

/// Function-typed values erase their concrete capture layout. Their dispatch
/// entry therefore receives the managed capture block as one opaque leading
/// argument, followed by the declared source parameters.
fn callable_dispatch_signature<'ctx>(
    context: &'ctx Context,
    layout: &ir::ClosureLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::FunctionType<'ctx> {
    let mut params = vec![context.ptr_type(AddressSpace::default()).into()];
    for ty in &layout.params {
        params.push(
            llvm_type_in(context, *ty, closures, values, enums)
                .expect("a callable parameter cannot be Void")
                .into(),
        );
    }
    match llvm_type_in(context, layout.returns, closures, values, enums) {
        Some(ty) => ty.fn_type(&params, false),
        None => context.void_type().fn_type(&params, false),
    }
}

/// Translates an IR module into an LLVM module.
pub fn emit<'ctx>(context: &'ctx Context, module: &ir::Module, name: &str) -> LlvmModule<'ctx> {
    emit_inner(context, module, name, None)
}

/// Translates an IR module into LLVM IR and attaches source-level DWARF data.
///
/// The source map is kept outside the portable IR: it supplies filesystem
/// paths and line/column coordinates without making the IR backend depend on
/// frontend project-loading policy.
pub fn emit_with_sources<'ctx>(
    context: &'ctx Context,
    module: &ir::Module,
    name: &str,
    sources: &SourceMap,
) -> LlvmModule<'ctx> {
    emit_inner(context, module, name, Some(sources))
}

fn emit_inner<'ctx>(
    context: &'ctx Context,
    module: &ir::Module,
    name: &str,
    sources: Option<&SourceMap>,
) -> LlvmModule<'ctx> {
    let llvm = context.create_module(name);
    if sources.is_some() {
        // LLVM 20 stores source variable locations as debug records rather
        // than ordinary instructions. Inkwell 0.10's convenience wrapper
        // attempts to wrap those records as instructions, so use LLVM's C API
        // directly when inserting `dbg.declare` records below.
        unsafe { LLVMSetIsNewDbgInfoFormat(llvm.as_mut_ptr(), 1) };
    }
    let builder = context.create_builder();
    let debug = sources.map(|sources| DebugContext::new(context, &llvm, sources));

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
        let declared = if let Some(existing) = llvm.get_function(&extern_fn.name) {
            existing
        } else {
            declare_extern_fn(
                context,
                &llvm,
                extern_fn,
                &module.closures,
                &module.values,
                &module.enums,
            )
        };
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

    // Application state cells are shared by every source function in this
    // emitted module. The runtime root registry keeps each allocated cell
    // alive for the process lifetime; these private globals hold their handles.
    let project_global_handles: Vec<PointerValue> = module
        .project_globals
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let global = llvm.add_global(
                context.ptr_type(AddressSpace::default()),
                None,
                &format!("zk.project.global.{index}"),
            );
            global.set_initializer(&context.ptr_type(AddressSpace::default()).const_null());
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
                    .map(|symbol| {
                        functions
                            .get(symbol)
                            .unwrap_or_else(|| {
                                panic!("object method `{symbol}` has no LLVM declaration")
                            })
                            .as_global_value()
                            .as_pointer_value()
                    })
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
                let mut dispatch = table.methods.clone();
                dispatch.extend(table.specializations.iter().cloned());
                fields.push(table_of(&dispatch, &symbol).into());
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
                if field.ty.is_managed_reference(module) && direct_union_type(field.ty).is_none() {
                    let mut prefix = vec![index as u32 + OBJECT_HEADER_FIELDS];
                    gc_reference_paths(module, field.ty, &mut prefix, &mut gc_paths);
                }
            }
            fields.push(word.const_int(gc_paths.len() as u64, false).into());
            for path in &gc_paths {
                fields.push(const_field_offset(context, struct_type, path).into());
            }

            // Name every declared instance field in the class descriptor so
            // the open `Object` view can use the same dynamic property API
            // for class fields and sidecar properties. Each field record is:
            // key pointer, key byte length, byte offset, byte size, Dynamic
            // tag, reference-offset count, then offsets relative to the
            // field payload. The collector still uses the compact aggregate
            // reference list above; these per-field paths let a dynamically
            // read field preserve its references when boxed as `Dynamic`.
            fields.push(word.const_int(layout.fields.len() as u64, false).into());
            let mut union_fields = Vec::new();
            for (field_index, field) in layout.fields.iter().enumerate() {
                let key_bytes = context.const_string(field.name.as_bytes(), false);
                let key_global = llvm.add_global(
                    key_bytes.get_type(),
                    None,
                    &format!("zk.field.{}.{}", layout.name, field_index),
                );
                key_global.set_initializer(&key_bytes);
                key_global.set_constant(true);
                key_global.set_linkage(Linkage::Private);

                let field_type = llvm_type_in(
                    context,
                    field.ty,
                    &module.closures,
                    &module.values,
                    &module.enums,
                )
                .expect("class instance fields have sized LLVM types");
                let field_path = vec![field_index as u32 + OBJECT_HEADER_FIELDS];
                let field_offset = const_field_offset(context, struct_type, &field_path);
                let mut reference_paths = Vec::new();
                if direct_union_type(field.ty).is_none() {
                    gc_reference_paths(
                        module,
                        field.ty,
                        &mut field_path.clone(),
                        &mut reference_paths,
                    );
                }

                fields.push(key_global.as_pointer_value().into());
                fields.push(word.const_int(field.name.len() as u64, false).into());
                fields.push(field_offset.into());
                fields.push(
                    field_type
                        .size_of()
                        .expect("class instance fields are sized")
                        .into(),
                );
                fields.push(
                    word.const_int(u64::from(field.ty.dynamic_tag().unwrap_or(0)), false)
                        .into(),
                );
                fields.push(word.const_int(reference_paths.len() as u64, false).into());
                for path in &reference_paths {
                    let reference_offset = const_field_offset(context, struct_type, path);
                    fields.push(reference_offset.const_sub(field_offset).into());
                }

                if let Some((representation, nullable)) = direct_union_type(field.ty)
                    && let Some((union_id, _)) =
                        module.union_type_for_representation(representation)
                {
                    let (union_value_offset, present_offset) = if nullable {
                        let union_type = ir::IrType::Union(representation);
                        let nullable_type = nullable_struct(
                            context,
                            union_type,
                            &module.closures,
                            &module.values,
                            &module.enums,
                        );
                        (
                            const_field_offset(context, nullable_type, &[1]),
                            const_field_offset(context, nullable_type, &[0]),
                        )
                    } else {
                        (word.const_zero(), word.const_int(u64::MAX, false))
                    };
                    union_fields.push((
                        field_offset.const_add(union_value_offset),
                        union_gc_descriptor(context, &llvm, module, union_id),
                        if nullable {
                            field_offset.const_add(present_offset)
                        } else {
                            word.const_int(u64::MAX, false)
                        },
                    ));
                }
            }

            // Tag-aware union field roots follow the ordinary field metadata.
            // Each record stores the active value offset, its static union
            // descriptor, and either the nullable `present` offset or the
            // non-nullable sentinel.
            fields.push(word.const_int(union_fields.len() as u64, false).into());
            for (value_offset, union_descriptor, present_offset) in union_fields {
                fields.push(value_offset.into());
                fields.push(union_descriptor.const_to_int(word).into());
                fields.push(present_offset.into());
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

    // One descriptor per closure layout.  It mirrors a class descriptor's
    // shape at the tail (`word0` is ignored, `ancestor_count`/`contract_count`
    // are zero, then the GC field count and offsets) so `zirk_rt_clone` and
    // the collector can both walk the captured managed references.  `word0`
    // points back at the descriptor itself so `clone_recursive` copies a
    // non-null descriptor into the new block and continues the traversal.
    let ptr = context.ptr_type(AddressSpace::default());
    let word = context.i64_type();
    let closure_descriptors: Vec<PointerValue> = module
        .closures
        .iter()
        .enumerate()
        .map(|(index, layout)| {
            // Build the payload struct exactly as `MakeCallable`/`CallCallable`
            // do, so the GC-offset table and the live stores agree on every
            // field's position (including alignment padding).
            let payload_types: Vec<BasicTypeEnum> = layout
                .captures
                .iter()
                .map(|&ty| {
                    llvm_type_in(context, ty, &module.closures, &module.values, &module.enums)
                        .expect("a capture has an LLVM representation")
                })
                .collect();
            let payload_struct = context.struct_type(&payload_types, false);

            let mut gc_paths: Vec<Vec<u32>> = Vec::new();
            let mut union_fields = Vec::new();
            for (capture_index, capture) in layout.captures.iter().enumerate() {
                if let Some((representation, nullable)) = direct_union_type(*capture) {
                    if let Some((union_id, _)) =
                        module.union_type_for_representation(representation)
                    {
                        let (value_offset, present_offset) = if nullable {
                            let union_type = ir::IrType::Union(representation);
                            let nullable_type = nullable_struct(
                                context,
                                union_type,
                                &module.closures,
                                &module.values,
                                &module.enums,
                            );
                            (
                                const_field_offset(context, nullable_type, &[1]),
                                const_field_offset(context, nullable_type, &[0]),
                            )
                        } else {
                            (word.const_zero(), word.const_int(u64::MAX, false))
                        };
                        union_fields.push((
                            const_field_offset(context, payload_struct, &[capture_index as u32])
                                .const_add(value_offset)
                                .const_add(
                                    word.const_int(CAPTURE_BLOCK_HEADER_BYTES as u64, false),
                                ),
                            union_gc_descriptor(context, &llvm, module, union_id),
                            if nullable {
                                const_field_offset(context, payload_struct, &[capture_index as u32])
                                    .const_add(present_offset)
                                    .const_add(
                                        word.const_int(CAPTURE_BLOCK_HEADER_BYTES as u64, false),
                                    )
                            } else {
                                word.const_int(u64::MAX, false)
                            },
                        ));
                    }
                } else if capture.is_managed_reference(module) {
                    let mut prefix = vec![capture_index as u32];
                    gc_reference_paths(module, *capture, &mut prefix, &mut gc_paths);
                }
            }

            let gc_offsets: Vec<BasicValueEnum> = gc_paths
                .iter()
                .map(|path| {
                    let field_offset = const_field_offset(context, payload_struct, path);
                    let header = context
                        .i64_type()
                        .const_int(CAPTURE_BLOCK_HEADER_BYTES as u64, false);
                    field_offset.const_add(header).into()
                })
                .collect();

            let mut fields: Vec<BasicValueEnum> = vec![
                ptr.const_null().into(),
                word.const_int(0, false).into(),
                word.const_int(0, false).into(),
                word.const_int(gc_offsets.len() as u64, false).into(),
            ];
            fields.extend(gc_offsets);
            // Closure capture blocks share the class-descriptor tail consumed
            // by the collector: zero declared-field metadata followed by
            // tag-aware roots for captured union values.
            fields.push(word.const_int(0, false).into());
            fields.push(word.const_int(union_fields.len() as u64, false).into());
            for (value_offset, union_descriptor, present_offset) in union_fields {
                fields.push(value_offset.into());
                fields.push(union_descriptor.const_to_int(word).into());
                fields.push(present_offset.into());
            }

            let placeholder = context.const_struct(&fields, false);
            let global = llvm.add_global(
                placeholder.get_type(),
                None,
                &format!("zk.closure.type.{index}"),
            );
            global.set_constant(true);
            global.set_linkage(Linkage::Private);

            fields[0] = global.as_pointer_value().into();
            global.set_initializer(&context.const_struct(&fields, false));

            global.as_pointer_value()
        })
        .collect();

    for function in &module.functions {
        let subprogram = debug
            .as_ref()
            .filter(|_| !function.span.is_empty())
            .map(|debug| debug.function(function, functions[&function.name]));
        FunctionEmitter {
            context,
            builder: &builder,
            llvm: &llvm,
            module,
            runtime: &runtime,
            functions: &functions,
            strings: &strings,
            project_global_handles: &project_global_handles,
            descriptors: &descriptors,
            closure_descriptors: &closure_descriptors,
            debug: debug.as_ref(),
            subprogram,
            debug_variables: HashMap::new(),
            declared_debug_variables: std::collections::HashSet::new(),
            values: HashMap::new(),
            value_types: HashMap::new(),
            slots: HashMap::new(),
            slot_types: HashMap::new(),
            blocks: HashMap::new(),
        }
        .emit(function, functions[&function.name]);
    }

    builder.unset_current_debug_location();
    if let Some(entry) = functions.get(ENTRYPOINT) {
        emit_c_entrypoint(context, &builder, &llvm, &runtime, *entry);
    }

    if let Some(debug) = &debug {
        debug.builder.finalize();
    }

    llvm
}

/// Source and type metadata shared by the functions in one emitted module.
struct DebugContext<'ctx, 'sources> {
    builder: DebugInfoBuilder<'ctx>,
    unit: DICompileUnit<'ctx>,
    files: HashMap<FileId, DIFile<'ctx>>,
    sources: &'sources SourceMap,
}

impl<'ctx, 'sources> DebugContext<'ctx, 'sources> {
    fn new(context: &'ctx Context, llvm: &LlvmModule<'ctx>, sources: &'sources SourceMap) -> Self {
        let entry_path = Path::new(sources.entry().name());
        let filename = entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(sources.entry().name());
        let directory = entry_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy();
        let (builder, unit) = llvm.create_debug_info_builder(
            true,
            // DWARF has no Zirk language code; use C's generic source-location
            // model so debuggers can consume the line and variable records.
            DWARFSourceLanguage::C,
            filename,
            &directory,
            &format!("Zirk compiler {}", env!("CARGO_PKG_VERSION")),
            false,
            "",
            0,
            "",
            DWARFEmissionKind::Full,
            0,
            false,
            false,
            "",
            "",
        );
        llvm.add_basic_value_flag(
            "Debug Info Version",
            FlagBehavior::Warning,
            context.i32_type().const_int(
                u64::from(inkwell::debug_info::debug_metadata_version()),
                false,
            ),
        );

        let mut files = HashMap::new();
        for (id, source) in sources.files() {
            let file = if id == FileId::ENTRY {
                unit.get_file()
            } else {
                let path = Path::new(source.name());
                let filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(source.name());
                let directory = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."))
                    .to_string_lossy();
                builder.create_file(filename, &directory)
            };
            files.insert(id, file);
        }
        Self {
            builder,
            unit,
            files,
            sources,
        }
    }

    fn file(&self, file: FileId) -> DIFile<'ctx> {
        self.files
            .get(&file)
            .copied()
            .unwrap_or_else(|| self.unit.get_file())
    }

    fn location(
        &self,
        span: Span,
        scope: DIScope<'ctx>,
        context: &'ctx Context,
    ) -> inkwell::debug_info::DILocation<'ctx> {
        let source = self.sources.file(span.file);
        let (line, column) = source.line_column(span.start);
        self.builder
            .create_debug_location(context, line, column, scope, None)
    }

    fn function(
        &self,
        function: &ir::Function,
        llvm_function: FunctionValue<'ctx>,
    ) -> DISubprogram<'ctx> {
        let file = self.file(function.span.file);
        let (line, _) = self
            .sources
            .file(function.span.file)
            .line_column(function.span.start);
        let parameter_types: Vec<_> = function
            .params
            .iter()
            .map(|slot| debug_type(&self.builder, function.slots[slot.0 as usize].ty))
            .collect();
        let result_type = (function.return_type != ir::IrType::Void)
            .then(|| debug_type(&self.builder, function.return_type));
        let signature = self.builder.create_subroutine_type(
            file,
            result_type,
            &parameter_types,
            DIFlags::PUBLIC,
        );
        let linkage_name = llvm_function.get_name().to_string_lossy().into_owned();
        let subprogram = self.builder.create_function(
            self.unit.as_debug_info_scope(),
            &function.name,
            Some(&linkage_name),
            file,
            line,
            signature,
            true,
            true,
            line,
            DIFlags::PUBLIC,
            false,
        );
        llvm_function.set_subprogram(subprogram);
        subprogram
    }
}

fn debug_type<'ctx>(builder: &DebugInfoBuilder<'ctx>, ty: ir::IrType) -> DIType<'ctx> {
    let (name, size, encoding) = match ty {
        ir::IrType::Int(width) => (
            width.as_str(),
            u64::from(width.bits()),
            if width.signed() { 0x05 } else { 0x07 },
        ),
        ir::IrType::Float(width) => (width.as_str(), u64::from(width.bits()), 0x04),
        ir::IrType::Boolean => ("Boolean", 1, 0x02),
        ir::IrType::Void | ir::IrType::Never => ("Void", 0, 0x00),
        _ => (ty.as_str(), 0, 0x00),
    };
    builder
        .create_basic_type(name, size, encoding, DIFlags::PUBLIC)
        .expect("Zirk debug type has a non-empty name")
        .as_type()
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
        // The exact base-ten `Float`: a `{ i128, i8 }` aggregate by value in
        // SSA, spilled to a stack slot whenever it crosses the runtime
        // boundary (which every operation on it does).
        ir::IrType::Decimal => context
            .struct_type(
                &[context.i128_type().into(), context.i8_type().into()],
                false,
            )
            .into(),
        // `Boolean` is `i1`: LLVM's natural type for a condition, and what a
        // conditional branch expects.
        ir::IrType::Boolean => context.bool_type().into(),
        // `String` is an opaque pointer. Its layout belongs to the runtime.
        ir::IrType::String => context.ptr_type(AddressSpace::default()).into(),
        // `Regex` is an opaque pointer to a process-wide cache entry.
        ir::IrType::Regex => context.ptr_type(AddressSpace::default()).into(),
        // `Char` shares `String`'s opaque runtime representation (ADR-014).
        ir::IrType::Char => context.ptr_type(AddressSpace::default()).into(),
        // An object is reached through its address: identity *is* the address,
        // so the value carried around is a pointer. The struct behind it is
        // only needed where a field is addressed.
        ir::IrType::Object(_) => context.ptr_type(AddressSpace::default()).into(),
        ir::IrType::ObjectAny | ir::IrType::Dynamic => {
            context.ptr_type(AddressSpace::default()).into()
        }
        // Reached through the contract, but still just the object's address:
        // the descriptor it already carries answers which body to run.
        ir::IrType::Contract(_) => context.ptr_type(AddressSpace::default()).into(),
        // BorrowedView is a verifier-only lifetime token over the same
        // protected managed handle. It has no runtime wrapper or allocation.
        ir::IrType::BorrowedView { .. } => context.ptr_type(AddressSpace::default()).into(),
        // A record carries no identity, so it is the struct
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
        ir::IrType::Union(id) => {
            let layout = enums
                .get(id as usize)
                .expect("a verified module declares every union bridge layout");
            let descriptor = layout
                .union_layout
                .as_ref()
                .expect("a verified union bridge has physical layout metadata");
            union_struct(context, descriptor).into()
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
        // A boxed callable keeps its concrete target/capture pair for the
        // native callback ABIs and a uniform dispatch entry for calls through
        // a function-typed parameter whose concrete capture layout is erased.
        ir::IrType::Callable(_) => callable_struct(context).into(),
        // Jobs cross the runtime boundary as packed generational ids.
        ir::IrType::Job => context.i64_type().into(),
        // Channels are collector-tracked opaque endpoints.
        ir::IrType::Channel(_) => context.ptr_type(AddressSpace::default()).into(),
        ir::IrType::Atomic(_) => context.ptr_type(AddressSpace::default()).into(),
        ir::IrType::Mutex(_) => context.ptr_type(AddressSpace::default()).into(),
        // `Pointer<T>` (roadmap Phase 4e, design D8): an ordinary LLVM
        // pointer — opaque at this level, since LLVM's own `ptr` type
        // carries no pointee type; a load/store through it supplies `T`'s
        // own LLVM type separately, at the instruction that needs it.
        ir::IrType::Pointer(_) => context.ptr_type(AddressSpace::default()).into(),
        // `Weak<T>` (roadmap Phase 4e, `fase-4e-weak`, design D1): a pointer
        // to a WeakCell — an ordinary collector-tracked allocation, opaque
        // at this level the same way `Object`/`Contract` are.
        ir::IrType::Weak(_) => context.ptr_type(AddressSpace::default()).into(),
        // The `*mut Journal` handle of an `unsafe {}` block's own undo log
        // (roadmap Phase 4e, `fase-4e-unsafe-journal`, design D1/D4): an
        // ordinary opaque LLVM pointer, never collector-tracked.
        ir::IrType::JournalHandle => context.ptr_type(AddressSpace::default()).into(),
        // `NativeSlice<T>`/`NativeSliceMut<T>` (roadmap Phase 4e,
        // `fase-4e-native-slice`, design D2): a plain two-word
        // `(pointer, length)` struct — no allocation, no header, not a
        // collector-tracked value. `length` is `UInt64` (`IrType::Int(U64)`
        // maps to `i64` the same way every other integer width does).
        ir::IrType::NativeSlice(_) | ir::IrType::NativeSliceMut(_) => {
            native_slice_struct(context).into()
        }
        // `Dependent<T>`/`Pin<T>` (roadmap Phase 4e, `phase-4e-memory`,
        // design D1): surface-only, represented as an opaque pointer until
        // the two-word dependent form and the per-thread pin list are wired
        // through codegen and the runtime.
        ir::IrType::Dependent(_) | ir::IrType::Pin(_) => {
            context.ptr_type(AddressSpace::default()).into()
        }
        // `Array<T>`/`List<T>`/`Map`/`Set` are opaque GC-managed object
        // handles, exactly like `String`/`Object`.
        ir::IrType::Array(_)
        | ir::IrType::List(_)
        | ir::IrType::Range
        | ir::IrType::Map(_)
        | ir::IrType::Set(_) => context.ptr_type(AddressSpace::default()).into(),
    })
}

/// The struct a `NativeSlice<T>`/`NativeSliceMut<T>` value occupies (design
/// D2): `{ ptr, i64 }` — base address, then element count. Both view types
/// share this one shape; only the checker's dispatch table (design D5)
/// distinguishes what is permitted through each.
fn native_slice_struct<'ctx>(context: &'ctx Context) -> inkwell::types::StructType<'ctx> {
    context.struct_type(
        &[
            context.ptr_type(AddressSpace::default()).into(),
            context.i64_type().into(),
        ],
        false,
    )
}

/// Field indices inside [`native_slice_struct`].
const NATIVE_SLICE_POINTER_FIELD: u32 = 0;
const NATIVE_SLICE_LENGTH_FIELD: u32 = 1;

/// The struct a record occupies — its fields, in declaration
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

/// Compact internal representation for a source union: a 32-bit tag followed
/// by one max-sized payload buffer. A zero-length alignment anchor gives the
/// payload the descriptor's alignment without consuming bytes in the value.
fn union_struct<'ctx>(
    context: &'ctx Context,
    descriptor: &ir::UnionLayoutDescriptor,
) -> inkwell::types::StructType<'ctx> {
    let payload_alignment_bytes = descriptor
        .alternatives
        .iter()
        .map(|alternative| usize::from(alternative.payload_alignment_bits).div_ceil(8))
        .max()
        .unwrap_or(1)
        .max(1);
    let payload_size_bytes = descriptor
        .alternatives
        .iter()
        .map(|alternative| alternative.payload_size_bits.div_ceil(8) as usize)
        .max()
        .unwrap_or(0);
    let payload_offset = 4usize.div_ceil(payload_alignment_bytes) * payload_alignment_bytes;
    let padding = context.i8_type().array_type((payload_offset - 4) as u32);
    let alignment_anchor: BasicTypeEnum = if payload_alignment_bytes <= 4 {
        context.i32_type().array_type(0).into()
    } else if payload_alignment_bytes <= 8 {
        context.i64_type().array_type(0).into()
    } else if payload_alignment_bytes <= 16 {
        context.i64_type().vec_type(2).array_type(0).into()
    } else {
        panic!("unsupported union payload alignment: {payload_alignment_bytes} bytes")
    };
    context.struct_type(
        &[
            context.i32_type().into(),
            padding.into(),
            alignment_anchor,
            context
                .i8_type()
                .array_type(payload_size_bytes as u32)
                .into(),
        ],
        false,
    )
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

/// A boxed callable contains its concrete function pointer, managed capture
/// block, and uniform dispatch adapter pointer.
fn callable_struct<'ctx>(context: &'ctx Context) -> inkwell::types::StructType<'ctx> {
    context.struct_type(
        &[
            context.ptr_type(AddressSpace::default()).into(),
            context.ptr_type(AddressSpace::default()).into(),
            context.ptr_type(AddressSpace::default()).into(),
        ],
        false,
    )
}

const CALLABLE_FUNCTION_FIELD: u32 = 0;
const CALLABLE_CAPTURE_FIELD: u32 = 1;
const CALLABLE_DISPATCH_FIELD: u32 = 2;

/// The capture block returned by `zirk_rt_alloc_callable` starts with the
/// same three-word GC header as every object (`crate::collector` design D1);
/// the actual captured values follow.
const CAPTURE_BLOCK_HEADER_BYTES: usize = std::mem::size_of::<usize>() * 3;

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
        // A `Weak<T>` field is a managed pointer to a WeakCell, walked the
        // same way an `Object`/`Contract` field is (roadmap Phase 4e,
        // `fase-4e-weak`) — the path stops at the field itself; the
        // collector's own mark pass is what stops short of tracing *through*
        // the WeakCell (design D2), not this table.
        // `Array<T>`/`List<T>`/`Range`/`Map<K,V>`/`Set<T>` are each a single
        // managed pointer at the slot itself (their own descriptor is what
        // tells the collector's `mark_object` how to trace *through* them,
        // same as `Object`/`Contract` above) — not decomposed into further
        // fields here. Missing these five arms means `IrType::
        // is_managed_reference`'s zero-init/gc_roots pass still walks such a
        // slot and zero-initializes it (safe on its own), but no root
        // address is ever registered for it: a local `List<T>` (etc.)
        // variable would be invisible to every collection, so an
        // allocation-triggered sweep could reclaim it — and anything only
        // reachable through it — while it is still very much in use. Found
        // and fixed while investigating a real, reproducible `List<T>`
        // data-loss/crash bug under GC pressure (an ordinary `for x in list`
        // with no `parallel` involved).
        ir::IrType::Object(_)
        | ir::IrType::Contract(_)
        | ir::IrType::ObjectAny
        | ir::IrType::Dynamic
        | ir::IrType::Weak(_)
        | ir::IrType::Dependent(_)
        | ir::IrType::Pin(_)
        | ir::IrType::String
        | ir::IrType::Char
        | ir::IrType::Array(_)
        | ir::IrType::List(_)
        | ir::IrType::Channel(_)
        | ir::IrType::Atomic(_)
        | ir::IrType::Mutex(_)
        | ir::IrType::Range
        | ir::IrType::Map(_)
        | ir::IrType::Set(_) => out.push(prefix.clone()),
        ir::IrType::Nullable(n) if !matches!(n, ir::Nullable::Union(_)) => {
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
        ir::IrType::Union(id) => {
            // Tagged unions are traced by their active-tag descriptor. Flattening
            // all alternatives into ordinary reference paths would treat inactive
            // scalar bytes as pointers.
            let _ = id;
        }
        ir::IrType::Nullable(ir::Nullable::Union(_)) => {
            // Nullable tagged unions are likewise handled by a presence-aware
            // union root, never by the legacy flattened enum path.
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
        // A boxed `Callable` is `{ fn_ptr, capture_ptr, dispatch_ptr }`; only
        // the capture block is managed, and its descriptor traces the values
        // held by a closure.
        ir::IrType::Callable(_) => {
            prefix.push(CALLABLE_CAPTURE_FIELD);
            out.push(prefix.clone());
            prefix.pop();
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

fn direct_union_type(ty: ir::IrType) -> Option<(u32, bool)> {
    match ty {
        ir::IrType::Union(representation) => Some((representation, false)),
        ir::IrType::Nullable(ir::Nullable::Union(representation)) => Some((representation, true)),
        _ => None,
    }
}

#[derive(Debug)]
struct NestedUnionPath {
    representation: u32,
    value_path: Vec<u32>,
    present_path: Option<Vec<u32>>,
    enum_guards: Vec<(Vec<u32>, u32)>,
}

/// Finds inline union values nested in aggregate alternatives. The returned
/// field paths are interpreted against the LLVM type of the enclosing union
/// alternative, so layout offsets stay target-derived.
fn nested_union_paths(module: &ir::Module, ty: ir::IrType) -> Vec<NestedUnionPath> {
    fn walk(
        module: &ir::Module,
        ty: ir::IrType,
        path: &mut Vec<u32>,
        enum_guards: &mut Vec<(Vec<u32>, u32)>,
        out: &mut Vec<NestedUnionPath>,
    ) {
        match ty {
            ir::IrType::Union(representation) => out.push(NestedUnionPath {
                representation,
                value_path: path.clone(),
                present_path: None,
                enum_guards: enum_guards.clone(),
            }),
            ir::IrType::Nullable(ir::Nullable::Union(representation)) => {
                let prefix_len = path.len();
                path.push(1);
                out.push(NestedUnionPath {
                    representation,
                    value_path: path.clone(),
                    present_path: Some({
                        let mut present = path[..prefix_len].to_vec();
                        present.push(0);
                        present
                    }),
                    enum_guards: enum_guards.clone(),
                });
                path.pop();
            }
            ir::IrType::Value(id) => {
                if let Some(layout) = module.values.get(id as usize) {
                    for (index, field) in layout.fields.iter().enumerate() {
                        path.push(index as u32);
                        walk(module, field.ty, path, enum_guards, out);
                        path.pop();
                    }
                }
            }
            ir::IrType::Enum(id) => {
                if let Some(layout) = module.enums.get(id as usize) {
                    for (variant, fields) in layout.variants.iter().enumerate() {
                        enum_guards.push((
                            path.iter().copied().chain(std::iter::once(0)).collect(),
                            variant as u32,
                        ));
                        for field in fields {
                            path.push(*field + ENUM_HEADER_FIELDS);
                            walk(
                                module,
                                layout.fields[*field as usize].ty,
                                path,
                                enum_guards,
                                out,
                            );
                            path.pop();
                        }
                        enum_guards.pop();
                    }
                }
            }
            ir::IrType::Closure(id) => {
                if let Some(layout) = module.closures.get(id as usize) {
                    for (index, capture) in layout.captures.iter().enumerate() {
                        path.push(CLOSURE_HEADER_FIELDS + index as u32);
                        walk(module, *capture, path, enum_guards, out);
                        path.pop();
                    }
                }
            }
            ir::IrType::Nullable(nullable) => {
                path.push(1);
                walk(module, nullable.inner(), path, enum_guards, out);
                path.pop();
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    walk(module, ty, &mut Vec::new(), &mut Vec::new(), &mut out);
    out
}

/// Emits a compiler-owned descriptor for a union's active-alternative GC
/// roots. Each alternative stores its direct reference offsets followed by
/// nested-union entries `(value_offset, descriptor, present_offset, guard_count, guards...)`.
/// Each guard is `(enum_tag_offset, expected_tag)`; a `u64::MAX` presence
/// offset denotes a non-nullable nested union.
fn union_gc_descriptor<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    module: &ir::Module,
    union_id: u32,
) -> PointerValue<'ctx> {
    let name = format!("zk.union.gc.{union_id}");
    if let Some(global) = llvm.get_global(&name) {
        return global.as_pointer_value();
    }
    let layout = &module.union_types[union_id as usize];
    let enum_id = layout
        .representation_layout
        .expect("verified executable union has an enum bridge");
    let enum_layout = &module.enums[enum_id as usize];
    let physical_layout = enum_layout
        .union_layout
        .as_ref()
        .expect("verified union bridge has physical layout metadata");
    let union_type = union_struct(context, physical_layout);
    let payload_offset = const_field_offset(context, union_type, &[3]);
    let i64_type = context.i64_type();
    let mut words = vec![i64_type.const_int(layout.alternatives.len() as u64, false)];
    for (alternative, fields) in enum_layout.variants.iter().enumerate() {
        let field = fields[0];
        let payload_type = enum_layout.fields[field as usize].ty;
        let paths = gc_reference_paths_of(module, payload_type);
        words.push(i64_type.const_int(paths.len() as u64, false));
        for path in paths {
            let nested_offset = if path.is_empty() {
                i64_type.const_zero()
            } else {
                let payload = llvm_type_in(
                    context,
                    module.union_types[union_id as usize].alternatives[alternative],
                    &module.closures,
                    &module.values,
                    &module.enums,
                )
                .expect("union payload has an LLVM representation")
                .into_struct_type();
                const_field_offset(context, payload, &path)
            };
            words.push(payload_offset.const_add(nested_offset));
        }
        let payload = llvm_type_in(
            context,
            layout.alternatives[alternative],
            &module.closures,
            &module.values,
            &module.enums,
        )
        .expect("union payload has an LLVM representation");
        let nested_unions = nested_union_paths(module, payload_type);
        words.push(i64_type.const_int(nested_unions.len() as u64, false));
        for nested in nested_unions {
            let representation = nested.representation;
            let (nested_id, _) = module
                .union_type_for_representation(representation)
                .expect("nested union representation is verified");
            let nested_descriptor = union_gc_descriptor(context, llvm, module, nested_id);
            let payload_type = payload.into_struct_type();
            let value_offset = if nested.value_path.is_empty() {
                i64_type.const_zero()
            } else {
                const_field_offset(context, payload_type, &nested.value_path)
            };
            let present_offset = nested.present_path.as_ref().map_or_else(
                || i64_type.const_int(u64::MAX, false),
                |path| const_field_offset(context, payload_type, path),
            );
            words.push(payload_offset.const_add(value_offset));
            words.push(nested_descriptor.const_to_int(i64_type));
            words.push(match nested.present_path {
                Some(_) => payload_offset.const_add(present_offset),
                None => i64_type.const_int(u64::MAX, false),
            });
            words.push(i64_type.const_int(nested.enum_guards.len() as u64, false));
            for (tag_path, expected_tag) in nested.enum_guards {
                let tag_offset = const_field_offset(context, payload_type, &tag_path);
                words.push(payload_offset.const_add(tag_offset));
                words.push(i64_type.const_int(expected_tag as u64, false));
            }
        }
    }
    let descriptor_type = i64_type.array_type(words.len() as u32);
    let initializer = i64_type.const_array(&words);
    let global = llvm.add_global(descriptor_type, None, &name);
    global.set_initializer(&initializer);
    global.set_constant(true);
    global.set_linkage(Linkage::Private);
    global.as_pointer_value()
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

/// Runtime allocation shape for a non-void task result. The result payload is
/// deliberately the exact aggregate/scalar LLVM type, preceded by the common
/// collector header so the normal descriptor walker can trace nested roots.
fn task_result_box_type<'ctx>(
    context: &'ctx Context,
    module: &ir::Module,
    result: ir::IrType,
) -> inkwell::types::StructType<'ctx> {
    let ptr = context.ptr_type(AddressSpace::default());
    context.struct_type(
        &[
            ptr.into(),
            context.i64_type().into(),
            context.i64_type().into(),
            llvm_type_in(
                context,
                result,
                &module.closures,
                &module.values,
                &module.enums,
            )
            .expect("a non-void task result has an LLVM representation"),
        ],
        false,
    )
}

/// The struct a WeakCell occupies (roadmap Phase 4e, `fase-4e-weak`, design
/// D1/D2):
///
/// ```text
///    [ sentinel descriptor | next (mark bit) | size | target ]
/// ```
///
/// The same three-word header every collector allocation carries
/// ([`object_struct`]'s own doc comment), then one field: the target's own
/// address. Unlike [`object_struct`], this is not per-class — every
/// WeakCell in the program shares this one shape, since `Weak<T>`'s
/// operation set is closed and compiler-built-in (design D1).
fn weak_cell_struct_type<'ctx>(context: &'ctx Context) -> inkwell::types::StructType<'ctx> {
    let ptr = context.ptr_type(AddressSpace::default());
    context.struct_type(
        &[
            ptr.into(),
            ptr.into(),
            context.i64_type().into(),
            ptr.into(),
        ],
        false,
    )
}

/// Marks a generated function as needing an unwind table.
///
/// The runtime propagates a cancelled branch (`CancelledError`) and a branch's
/// unhandled exception as a native unwind through the branch's own stack
/// (`concurrent-blocks-and-timers`, ADR-017's stackful model). Without a
/// `uwtable` the unwinder hits generated frames it cannot cross and the
/// process aborts with "panic in a function that cannot unwind".
fn set_uwtable<'ctx>(context: &'ctx Context, function: FunctionValue<'ctx>) {
    let kind = inkwell::attributes::Attribute::get_named_enum_kind_id("uwtable");
    if kind != 0 {
        function.add_attribute(
            inkwell::attributes::AttributeLoc::Function,
            context.create_enum_attribute(kind, 2),
        );
    }
}

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

    let declared = llvm.add_function(
        &format!("{FUNCTION_PREFIX}{}", function.name),
        signature,
        None,
    );
    set_uwtable(context, declared);
    declared
}

/// `extern "C" fn` (roadmap Phase 4e, design D7, `ADR-015`): an ordinary
/// LLVM external function declaration, with no body and the real (unprefixed)
/// native symbol name — unlike an ordinary Zirk function, which gets
/// [`FUNCTION_PREFIX`] and a `define`. Resolving it at link time is left
/// entirely to the system linker, per ADR-015.
/// Parses an exact-decimal literal (`"0.1"`, `"6.25e-2"`, `"1e3"`) into its
/// `(coefficient, scale)` pair for a `{ i128, i8 }` struct constant.
///
/// The checker already rejected any literal that needs more than 38
/// significant digits, so an `i128` always holds the coefficient here.
fn parse_decimal_literal(text: &str) -> (i128, u8) {
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i64>().unwrap_or(0)),
        None => (text, 0),
    };
    let (negative, rest) = match mantissa.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, mantissa.strip_prefix('+').unwrap_or(mantissa)),
    };
    let (int_part, frac_part) = rest.split_once('.').unwrap_or((rest, ""));
    let digits: String = format!("{int_part}{frac_part}");
    let mut coef: i128 = digits.trim_start_matches('0').parse().unwrap_or(0);
    // scale = fractional digits minus the scientific exponent.
    let mut scale: i64 = frac_part.len() as i64 - exponent;
    while scale < 0 {
        coef = coef.saturating_mul(10);
        scale += 1;
    }
    // Normalize trailing zeros so the constant matches the runtime's form.
    while scale > 0 && coef % 10 == 0 {
        coef /= 10;
        scale -= 1;
    }
    if negative {
        coef = -coef;
    }
    (coef, scale.clamp(0, 38) as u8)
}

fn declare_extern_fn<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    extern_fn: &ir::ExternFn,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> FunctionValue<'ctx> {
    // The exact-`Float` runtime helpers, the `DateTime` civil-timestamp
    // ones, and the `Int`/`UInt` scalar-member ones (`zirk_int_*`) all cross
    // the C boundary with every `Decimal`/128-bit-integer value passed by
    // pointer, and such a *result* returned through a leading out-pointer
    // instead — the same reason `zirk_str_from_i128` takes its argument by
    // pointer rather than by value. Rust's own `i128`/`u128` C ABI is not
    // guaranteed to agree with LLVM's default lowering for a plain 128-bit
    // integer on every target, even when both sides' IR/source says `i128`:
    // confirmed on Windows x64, where a by-value `i128` crossing into
    // `zirk-runtime` (itself compiled by rustc for the same target) produced
    // a genuine access violation, first in `zirk_rt_datetime_new`'s return
    // and then in `zirk_int_checked_pow_*` — both fixed by switching the
    // Rust side of each family to take/return the 128-bit value by pointer
    // and matching that here. This is scoped *by name* to the families
    // actually rewritten that way in `zirk-runtime`, not to "every i128/u128
    // anywhere": an earlier attempt at generalizing this unconditionally by
    // type alone (before `zirk_int_*` itself had been converted) broke
    // ordinary checked-arithmetic, because the two sides of the ABI
    // disagreed about which convention to use — the fix there is to convert
    // both sides together, not to avoid the type-based gate in principle.
    // LLVM declarations are `ptr`-shaped accordingly; `emit_decimal_call`
    // (generic over any such value, despite its name) materializes the
    // matching call sites.
    let needs_pointer_abi = extern_fn.name.starts_with("zirk_rt_decimal_")
        || extern_fn.name == "zirk_str_from_decimal"
        || extern_fn.name.starts_with("zirk_rt_datetime_")
        || extern_fn.name.starts_with("zirk_int_");
    let ptr = context.ptr_type(AddressSpace::default());
    let by_pointer = |ty: ir::IrType| {
        needs_pointer_abi
            && matches!(
                ty,
                ir::IrType::Decimal | ir::IrType::Int(ir::IntWidth::I128 | ir::IntWidth::U128)
            )
    };

    let mut params: Vec<BasicMetadataTypeEnum> = Vec::new();
    let returns_by_pointer = by_pointer(extern_fn.return_type);
    if returns_by_pointer {
        params.push(ptr.into());
    }
    for &ty in &extern_fn.params {
        if by_pointer(ty) {
            params.push(ptr.into());
        } else {
            params.push(
                llvm_type_in(context, ty, closures, values, enums)
                    .expect("an extern parameter cannot be Void")
                    .into(),
            );
        }
    }

    // `zirk_regex_find` is the only extern function returning a
    // `Nullable<T>` (a `{ bool, ptr }` aggregate) — every other one is
    // built and consumed entirely by our own codegen, never crossing this
    // boundary. x86-64 Windows/MSVC and SysV disagree on how a 16-byte
    // `{ bool, ptr }` is returned by value (confirmed: access violation on
    // Windows), so — like the 128-bit families above — it takes a leading
    // out-pointer instead; see `zirk_regex_find`'s own doc for the ABI
    // specifics. This is scoped by name, the same as the families above.
    let returns_nullable_by_pointer = extern_fn.name == "zirk_regex_find";
    if returns_nullable_by_pointer {
        params.insert(0, ptr.into());
    }

    let signature = if returns_by_pointer || returns_nullable_by_pointer {
        context.void_type().fn_type(&params, false)
    } else {
        match llvm_type_in(context, extern_fn.return_type, closures, values, enums) {
            Some(ty) => ty.fn_type(&params, false),
            None => context.void_type().fn_type(&params, false),
        }
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
/// It materializes the lifecycle in `ZIRK_RUNTIME_SPEC.md` section 2: the
/// runtime is initialized, the selected file's generated entry runs, and the
/// runtime is shut down before returning the exit code.
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
    set_uwtable(context, main);
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);

    builder
        .build_call(runtime.init, &[], "")
        .expect("call to the runtime initializer");
    // `zirk_main` runs as the body of the cooperative executor's root task
    // (`ADR-017`) rather than being called directly, so a `main` that spawns
    // tasks is driven to completion before this returns.
    let zirk_main_ptr = zirk_main.as_global_value().as_pointer_value();
    builder
        .build_call(runtime.run_main_cli, &[zirk_main_ptr.into()], "")
        .expect("call to the executor entrypoint");

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
    project_global_handles: &'a [PointerValue<'ctx>],
    /// The type descriptor of each class, by layout id.
    descriptors: &'a [PointerValue<'ctx>],
    /// The capture-block descriptor of each closure layout, by layout id.
    closure_descriptors: &'a [PointerValue<'ctx>],
    debug: Option<&'a DebugContext<'ctx, 'a>>,
    subprogram: Option<DISubprogram<'ctx>>,
    debug_variables: HashMap<ir::SlotId, DILocalVariable<'ctx>>,
    declared_debug_variables: std::collections::HashSet<ir::SlotId>,

    values: HashMap<ir::ValueId, BasicValueEnum<'ctx>>,
    /// The IR type of each emitted value, which a pointer alone does not carry.
    value_types: HashMap<ir::ValueId, ir::IrType>,
    slots: HashMap<ir::SlotId, PointerValue<'ctx>>,
    /// The IR type of each slot (roadmap Phase 4e, `fase-4e-unsafe-journal`)
    /// — `JournalRecordSlot`'s own byte-size computation needs a slot's
    /// static type, which its `alloca` alone (an opaque `ptr` under LLVM's
    /// opaque-pointer model) does not carry.
    slot_types: HashMap<ir::SlotId, ir::IrType>,
    blocks: HashMap<ir::BlockId, inkwell::basic_block::BasicBlock<'ctx>>,
}

impl<'ctx> FunctionEmitter<'ctx, '_> {
    /// Builds static byte offsets for every managed reference inside one
    /// collection element. These offsets use the emitted LLVM layout of the
    /// element, matching the runtime's word-buffer representation.
    fn gc_offsets_for(
        &self,
        ty: ir::IrType,
        global_name: &str,
    ) -> (PointerValue<'ctx>, inkwell::values::IntValue<'ctx>) {
        let paths = gc_reference_paths_of(self.module, ty);
        let offsets: Vec<_> = paths
            .iter()
            .map(|path| {
                if path.is_empty() {
                    self.context.i64_type().const_zero()
                } else {
                    const_field_offset(
                        self.context,
                        self.llvm_type(ty)
                            .expect("managed collection element has an LLVM layout")
                            .into_struct_type(),
                        path,
                    )
                }
            })
            .collect();
        if offsets.is_empty() {
            return (
                self.context.ptr_type(AddressSpace::default()).const_null(),
                self.context.i64_type().const_zero(),
            );
        }
        let values = self.context.i64_type().const_array(&offsets);
        let global = self.llvm.add_global(values.get_type(), None, global_name);
        global.set_initializer(&values);
        global.set_constant(true);
        (
            global.as_pointer_value(),
            self.context
                .i64_type()
                .const_int(offsets.len() as u64, false),
        )
    }

    fn emit(mut self, function: &ir::Function, llvm_function: FunctionValue<'ctx>) {
        self.builder.unset_current_debug_location();
        if let (Some(debug), Some(subprogram)) = (self.debug, self.subprogram) {
            for (index, slot) in function.slots.iter().enumerate() {
                // Names wrapped in angle brackets are compiler-generated
                // implementation slots (GC roots, exception guards, call
                // temporaries, and similar). They are not Zirk declarations;
                // publishing them to DWARF makes native debuggers show
                // internal storage beside the user's locals.
                if slot.name.starts_with('<') {
                    continue;
                }
                let slot_id = ir::SlotId(index as u32);
                let file = debug.file(slot.span.file);
                let (line, _) = debug
                    .sources
                    .file(slot.span.file)
                    .line_column(slot.span.start);
                let is_parameter = function
                    .params
                    .iter()
                    .any(|parameter| *parameter == slot_id);
                let variable = if is_parameter {
                    // Parameters are copied into Zirk slots by the function
                    // prologue. Describing them as formal LLVM arguments
                    // makes LLDB treat the stack slot as live before that
                    // copy, exposing uninitialized stack contents at entry.
                    // Their source name and type remain intact; the location
                    // is attached once the slot is initialized below.
                    debug.builder.create_auto_variable(
                        subprogram.as_debug_info_scope(),
                        &slot.name,
                        file,
                        line,
                        debug_type(&debug.builder, slot.ty),
                        true,
                        DIFlags::PUBLIC,
                        0,
                    )
                } else {
                    debug.builder.create_auto_variable(
                        subprogram.as_debug_info_scope(),
                        &slot.name,
                        file,
                        line,
                        debug_type(&debug.builder, slot.ty),
                        true,
                        DIFlags::PUBLIC,
                        0,
                    )
                };
                self.debug_variables.insert(slot_id, variable);
            }
        }

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
            self.slot_types.insert(ir::SlotId(index as u32), slot.ty);
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
        let mut union_roots: Vec<(PointerValue, PointerValue, PointerValue)> = Vec::new();
        for &slot_id in &function.gc_roots {
            let slot = &function.slots[slot_id.0 as usize];
            let base_ptr = self.slots[&slot_id];
            let union_root = match slot.ty {
                ir::IrType::Union(representation) => Some((
                    representation,
                    base_ptr,
                    self.context.ptr_type(AddressSpace::default()).const_null(),
                )),
                ir::IrType::Nullable(ir::Nullable::Union(representation)) => {
                    let union_type = ir::IrType::Union(representation);
                    let nullable = nullable_struct(
                        self.context,
                        union_type,
                        &self.module.closures,
                        &self.module.values,
                        &self.module.enums,
                    );
                    let present = self
                        .builder
                        .build_struct_gep(nullable, base_ptr, 0, "gc_union_present")
                        .expect("nullable union present field");
                    let union_value = self
                        .builder
                        .build_struct_gep(nullable, base_ptr, 1, "gc_nullable_union_value")
                        .expect("nullable union value field");
                    Some((representation, union_value, present))
                }
                _ => None,
            };
            if let Some((representation, union_value, present)) = union_root
                && let Some((union_id, _)) =
                    self.module.union_type_for_representation(representation)
            {
                union_roots.push((
                    union_value,
                    union_gc_descriptor(self.context, self.llvm, self.module, union_id),
                    present,
                ));
                continue;
            }
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
        if !union_roots.is_empty() {
            let root_type = self
                .context
                .struct_type(&[ptr_type.into(), ptr_type.into(), ptr_type.into()], false);
            let array_type = root_type.array_type(union_roots.len() as u32);
            let roots = self
                .builder
                .build_alloca(array_type, "gc_union_roots")
                .expect("allocate union GC roots");
            for (index, (base, descriptor, present)) in union_roots.iter().enumerate() {
                let mut root = root_type.const_zero();
                root = self
                    .builder
                    .build_insert_value(root, *base, 0, "gc_union_root_base")
                    .expect("insert union root base")
                    .into_struct_value();
                root = self
                    .builder
                    .build_insert_value(root, *descriptor, 1, "gc_union_root_descriptor")
                    .expect("insert union root descriptor")
                    .into_struct_value();
                root = self
                    .builder
                    .build_insert_value(root, *present, 2, "gc_union_root_present")
                    .expect("insert union root present pointer")
                    .into_struct_value();
                let entry = unsafe {
                    self.builder.build_gep(
                        array_type,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_int(index as u64, false),
                        ],
                        "gc_union_root_entry",
                    )
                }
                .expect("index union root array");
                self.builder
                    .build_store(entry, root)
                    .expect("store union root entry");
            }
            self.builder
                .build_call(
                    self.runtime.push_union_roots,
                    &[
                        roots.into(),
                        i64_type.const_int(union_roots.len() as u64, false).into(),
                    ],
                    "",
                )
                .expect("install tag-aware union roots");
        }

        // Parameters arrive as values and are stored into their slots, so
        // reading them is uniform with any other local.
        let mut parameter_debug_declarations = Vec::new();
        let mut last_parameter_store = None;
        for (index, slot) in function.params.iter().enumerate() {
            let value = llvm_function
                .get_nth_param(index as u32)
                .expect("declared parameter");
            self.builder.unset_current_debug_location();
            let store = self
                .builder
                .build_store(self.slots[slot], value)
                .expect("parameter store");
            last_parameter_store = Some(store);
            if self.debug_variables.contains_key(slot) {
                parameter_debug_declarations.push(*slot);
            }
        }

        for block in &function.blocks {
            self.builder.position_at_end(self.blocks[&block.id]);

            for instruction in &block.instructions {
                if let (Some(debug), Some(subprogram)) = (self.debug, self.subprogram) {
                    self.builder.set_current_debug_location(debug.location(
                        instruction.span,
                        subprogram.as_debug_info_scope(),
                        self.context,
                    ));
                }
                self.emit_instruction(instruction);
            }

            let terminator = block
                .terminator
                .as_ref()
                .expect("verified IR always has a terminator");
            self.emit_terminator(terminator);
        }

        if let Some(next_instruction) =
            last_parameter_store.and_then(|store| store.get_next_instruction())
        {
            for slot in parameter_debug_declarations {
                let span = function.slots[slot.0 as usize].span;
                self.declare_debug_slot(slot, span, next_instruction);
            }
        }
    }

    fn declare_debug_slot(
        &mut self,
        slot: ir::SlotId,
        span: Span,
        store: inkwell::values::InstructionValue<'ctx>,
    ) {
        if !self.declared_debug_variables.insert(slot) {
            return;
        }
        let (Some(debug), Some(subprogram), Some(variable)) = (
            self.debug,
            self.subprogram,
            self.debug_variables.get(&slot).copied(),
        ) else {
            return;
        };
        let location = debug.location(span, subprogram.as_debug_info_scope(), self.context);
        let expression = debug.builder.create_expression(Vec::new());
        unsafe {
            LLVMDIBuilderInsertDeclareRecordBefore(
                debug.builder.as_mut_ptr(),
                self.slots[&slot].as_value_ref(),
                variable.as_mut_ptr(),
                expression.as_mut_ptr(),
                location.as_mut_ptr(),
                store.as_value_ref(),
            );
        }
    }

    fn operand(&self, operand: ir::Operand) -> BasicValueEnum<'ctx> {
        self.values[&operand.0]
    }

    /// Copies a key or element into a stack slot using its complete LLVM
    /// layout. Floating zero is canonicalized so equality and hashing agree.
    fn collection_value_slot(
        &self,
        operand: ir::Operand,
        name: &str,
    ) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let ty = self.value_types[&operand.0];
        let llvm_ty = self
            .llvm_type(ty)
            .expect("collection key has an LLVM layout");
        let slot = self
            .builder
            .build_alloca(llvm_ty, name)
            .expect("collection key slot");
        self.store_canonical_collection_value(slot, ty, self.operand(operand));
        (slot, self.byte_size_of(ty))
    }

    /// Writes a collection key field by field. The runtime compares the
    /// resulting allocation bytes, so aggregate padding must be zeroed and
    /// nested floating zero values normalized recursively.
    fn store_canonical_collection_value(
        &self,
        destination: PointerValue<'ctx>,
        ty: ir::IrType,
        value: BasicValueEnum<'ctx>,
    ) {
        match ty {
            ir::IrType::Float(ir::FloatWidth::F128) => {
                // Avoid an LLVM fp128 comparison here. On MSVC it can lower
                // through an ABI-incompatible soft-float helper. Signed zero
                // is canonicalized directly from the binary128 bits instead.
                let bits = self
                    .builder
                    .build_bit_cast(
                        value.into_float_value(),
                        self.context.i128_type(),
                        "key.f128.bits",
                    )
                    .expect("bitcast Float128 collection key")
                    .into_int_value();
                let low = self
                    .builder
                    .build_int_truncate(bits, self.context.i64_type(), "key.f128.low")
                    .expect("truncate Float128 low bits");
                let high_bits = self
                    .builder
                    .build_right_shift(
                        bits,
                        self.context.i128_type().const_int(64, false),
                        false,
                        "key.f128.high.bits",
                    )
                    .expect("extract Float128 high bits");
                let high = self
                    .builder
                    .build_int_truncate(high_bits, self.context.i64_type(), "key.f128.high")
                    .expect("truncate Float128 high bits");
                let high_magnitude = self
                    .builder
                    .build_and(
                        high,
                        self.context
                            .i64_type()
                            .const_int(0x7fff_ffff_ffff_ffff, false),
                        "key.f128.high.magnitude",
                    )
                    .expect("clear Float128 sign bit");
                let low_is_zero = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        low,
                        self.context.i64_type().const_zero(),
                        "key.f128.low.is_zero",
                    )
                    .expect("compare Float128 low bits");
                let high_is_zero = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        high_magnitude,
                        self.context.i64_type().const_zero(),
                        "key.f128.high.is_zero",
                    )
                    .expect("compare Float128 high magnitude");
                let is_zero = self
                    .builder
                    .build_and(low_is_zero, high_is_zero, "key.f128.is_zero")
                    .expect("detect signed Float128 zero");
                let normalized_bits = self
                    .builder
                    .build_select(
                        is_zero,
                        self.context.i128_type().const_zero(),
                        bits,
                        "key.f128.canonical_bits",
                    )
                    .expect("canonicalize Float128 signed zero")
                    .into_int_value();
                let normalized = self
                    .builder
                    .build_bit_cast(
                        normalized_bits,
                        self.context.f128_type(),
                        "key.f128.canonical_value",
                    )
                    .expect("restore Float128 collection key");
                self.builder
                    .build_store(destination, normalized)
                    .expect("store canonical Float128 collection key");
            }
            ir::IrType::Float(_) => {
                let float = value.into_float_value();
                let zero = float.get_type().const_zero();
                let is_zero = self
                    .builder
                    .build_float_compare(inkwell::FloatPredicate::OEQ, float, zero, "key.is_zero")
                    .expect("compare floating collection key with zero");
                let normalized = self
                    .builder
                    .build_select(is_zero, zero, float, "key.canonical_zero")
                    .expect("canonicalize floating collection zero");
                self.builder
                    .build_store(destination, normalized)
                    .expect("store canonical floating collection key");
            }
            ir::IrType::Value(id) => {
                let layout = &self.module.values[id as usize];
                let struct_ty = self.llvm_type(ty).expect("value layout").into_struct_type();
                self.zero_collection_aggregate(destination, ty);
                let value = value.into_struct_value();
                for (index, field) in layout.fields.iter().enumerate() {
                    let source = self
                        .builder
                        .build_extract_value(value, index as u32, "key.field")
                        .expect("extract collection key field");
                    let target = self
                        .builder
                        .build_struct_gep(struct_ty, destination, index as u32, "key.field.ptr")
                        .expect("collection key field pointer");
                    self.store_canonical_collection_value(target, field.ty, source);
                }
            }
            ir::IrType::Enum(id) => {
                let layout = &self.module.enums[id as usize];
                let struct_ty = self.llvm_type(ty).expect("enum layout").into_struct_type();
                self.zero_collection_aggregate(destination, ty);
                let value = value.into_struct_value();
                for index in 0..=layout.fields.len() {
                    let field_ty = if index == 0 {
                        ir::IrType::Int(ir::IntWidth::I32)
                    } else {
                        layout.fields[index - 1].ty
                    };
                    if field_ty == ir::IrType::Void {
                        continue;
                    }
                    let source = self
                        .builder
                        .build_extract_value(value, index as u32, "key.enum.field")
                        .expect("extract enum collection key field");
                    let target = self
                        .builder
                        .build_struct_gep(
                            struct_ty,
                            destination,
                            index as u32,
                            "key.enum.field.ptr",
                        )
                        .expect("enum collection key field pointer");
                    self.store_canonical_collection_value(target, field_ty, source);
                }
            }
            ir::IrType::Nullable(nullable) => {
                let struct_ty = self
                    .llvm_type(ty)
                    .expect("nullable layout")
                    .into_struct_type();
                self.zero_collection_aggregate(destination, ty);
                let value = value.into_struct_value();
                let present = self
                    .builder
                    .build_extract_value(value, 0, "key.nullable.present")
                    .expect("extract nullable presence");
                let present_ptr = self
                    .builder
                    .build_struct_gep(struct_ty, destination, 0, "key.nullable.present.ptr")
                    .expect("nullable presence pointer");
                self.builder
                    .build_store(present_ptr, present)
                    .expect("store nullable presence");
                let inner = self
                    .builder
                    .build_extract_value(value, 1, "key.nullable.value")
                    .expect("extract nullable value");
                let inner_ptr = self
                    .builder
                    .build_struct_gep(struct_ty, destination, 1, "key.nullable.value.ptr")
                    .expect("nullable value pointer");
                self.store_canonical_collection_value(inner_ptr, nullable.inner(), inner);
            }
            ir::IrType::Decimal => {
                let struct_ty = self
                    .llvm_type(ty)
                    .expect("Decimal layout")
                    .into_struct_type();
                self.zero_collection_aggregate(destination, ty);
                let value = value.into_struct_value();
                for index in 0..2 {
                    let field = self
                        .builder
                        .build_extract_value(value, index, "key.decimal.field")
                        .expect("extract Decimal collection key field");
                    let target = self
                        .builder
                        .build_struct_gep(struct_ty, destination, index, "key.decimal.field.ptr")
                        .expect("Decimal collection key field pointer");
                    self.builder
                        .build_store(target, field)
                        .expect("store Decimal key field");
                }
            }
            _ => {
                self.builder
                    .build_store(destination, value)
                    .expect("store scalar collection key");
            }
        }
    }

    fn zero_collection_aggregate(&self, destination: PointerValue<'ctx>, ty: ir::IrType) {
        self.builder
            .build_memset(
                destination,
                1,
                self.context.i8_type().const_zero(),
                self.byte_size_of(ty),
            )
            .expect("zero collection key aggregate padding");
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

    fn emit_project_global_init(&mut self, global: u32, value_slot: ir::SlotId) -> IntValue<'ctx> {
        let payload_ty = self.module.project_globals[global as usize].payload;
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.init.status")
            .expect("allocate project-global initialization status");
        self.builder
            .build_store(status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global status");
        let (offsets, offset_count) =
            self.gc_offsets_for(payload_ty, &format!("zk.project.global.offsets.{global}"));
        let handle = self
            .builder
            .build_call(
                self.runtime.gc_mutex_alloc,
                &[
                    self.slots[&value_slot].into(),
                    self.byte_size_of(payload_ty).into(),
                    self.byte_align_of(payload_ty).into(),
                    offsets.into(),
                    offset_count.into(),
                    status_out.into(),
                ],
                "project.global.handle",
            )
            .expect("allocate project-global state cell")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex allocator returns a handle")
            .into_pointer_value();
        let root_handle = self
            .builder
            .build_call(
                self.runtime.global_root_add,
                &[handle.into()],
                "project.global.root.handle",
            )
            .expect("register project-global state root")
            .try_as_basic_value()
            .basic()
            .expect("global root registration returns a handle")
            .into_int_value();
        let allocation_status = self
            .builder
            .build_load(i32_ty, status_out, "project.global.alloc.status")
            .expect("load project-global allocation status")
            .into_int_value();
        let allocation_ok = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                allocation_status,
                i32_ty.const_zero(),
                "project.global.alloc.ok",
            )
            .expect("check project-global allocation");
        let root_ok = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                root_handle,
                i64_ty.const_zero(),
                "project.global.root.ok",
            )
            .expect("check project-global root registration");
        let fully_initialized = self
            .builder
            .build_and(allocation_ok, root_ok, "project.global.ready")
            .expect("combine project-global initialization checks");
        let safe_handle = self
            .builder
            .build_select(
                fully_initialized,
                handle,
                ptr_ty.const_null(),
                "project.global.safe.handle",
            )
            .expect("select rooted project-global handle")
            .into_pointer_value();
        self.builder
            .build_call(
                self.runtime.global_watch_reset,
                &[safe_handle.into()],
                "project.global.watch.reset",
            )
            .expect("initialize project-global watch state");
        self.builder
            .build_store(self.project_global_handles[global as usize], safe_handle)
            .expect("publish project-global state handle");
        let root_failure = self
            .builder
            .build_select(
                root_ok,
                i32_ty.const_zero(),
                i32_ty.const_int(1, false),
                "project.global.root.status",
            )
            .expect("select root registration status")
            .into_int_value();
        self.builder
            .build_select(
                allocation_ok,
                root_failure,
                allocation_status,
                "project.global.init.result",
            )
            .expect("select project-global initialization status")
            .into_int_value()
    }

    fn emit_project_global_get(&mut self, global: u32, out: ir::SlotId) -> IntValue<'ctx> {
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let handle = self
            .builder
            .build_load(
                ptr_ty,
                self.project_global_handles[global as usize],
                "project.global.handle",
            )
            .expect("load project-global handle")
            .into_pointer_value();
        let payload_out = self
            .builder
            .build_alloca(ptr_ty, "project.global.payload.out")
            .expect("allocate project-global payload pointer");
        let status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.get.status.out")
            .expect("allocate project-global read status");
        self.builder
            .build_store(payload_out, ptr_ty.const_null())
            .expect("initialize project-global payload pointer");
        self.builder
            .build_store(status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global read status");
        let acquired = self
            .builder
            .build_call(
                self.runtime.gc_mutex_acquire,
                &[handle.into(), payload_out.into(), status_out.into()],
                "project.global.get.acquire.status",
            )
            .expect("acquire project-global state cell")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex acquire returns status")
            .into_int_value();
        let okay = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                acquired,
                i32_ty.const_zero(),
                "project.global.get.acquired",
            )
            .expect("check project-global acquire status");
        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .expect("project-global read is emitted inside a function");
        let failed_block = self
            .context
            .append_basic_block(function, "project.global.get.failed");
        let success_block = self
            .context
            .append_basic_block(function, "project.global.get.success");
        let done_block = self
            .context
            .append_basic_block(function, "project.global.get.done");
        self.builder
            .build_conditional_branch(okay, success_block, failed_block)
            .expect("branch on project-global acquire status");

        self.builder.position_at_end(failed_block);
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join failed project-global read");

        self.builder.position_at_end(success_block);
        let payload_ty = self.module.project_globals[global as usize].payload;
        let payload_pointer = self
            .builder
            .build_load(ptr_ty, payload_out, "project.global.payload")
            .expect("load project-global payload address")
            .into_pointer_value();
        let payload = self
            .builder
            .build_load(
                self.llvm_type(payload_ty)
                    .expect("global payload has a value type"),
                payload_pointer,
                "project.global.snapshot.source",
            )
            .expect("load project-global snapshot");
        self.builder
            .build_store(self.slots[&out], payload)
            .expect("root project-global snapshot payload");
        let release_status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.get.release.status.out")
            .expect("allocate project-global release status");
        self.builder
            .build_store(release_status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global release status");
        let released = self
            .builder
            .build_call(
                self.runtime.gc_mutex_release,
                &[handle.into(), release_status_out.into()],
                "project.global.get.release.status",
            )
            .expect("release project-global state cell")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex release returns status")
            .into_int_value();
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join successful project-global read");

        self.builder.position_at_end(done_block);
        let status = self
            .builder
            .build_phi(i32_ty, "project.global.get.status")
            .expect("merge project-global read status");
        status.add_incoming(&[(&acquired, failed_block), (&released, success_block)]);
        status.as_basic_value().into_int_value()
    }

    fn emit_project_global_set(
        &mut self,
        global: u32,
        value: ir::Operand,
        event: Option<ir::ProjectGlobalWatchEvent>,
    ) -> IntValue<'ctx> {
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let handle = self
            .builder
            .build_load(
                ptr_ty,
                self.project_global_handles[global as usize],
                "project.global.handle",
            )
            .expect("load project-global handle")
            .into_pointer_value();
        let payload_out = self
            .builder
            .build_alloca(ptr_ty, "project.global.payload.out")
            .expect("allocate project-global payload pointer");
        let status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.set.status.out")
            .expect("allocate project-global write status");
        self.builder
            .build_store(payload_out, ptr_ty.const_null())
            .expect("initialize project-global payload pointer");
        self.builder
            .build_store(status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global write status");
        let acquired = self
            .builder
            .build_call(
                self.runtime.gc_mutex_acquire,
                &[handle.into(), payload_out.into(), status_out.into()],
                "project.global.set.acquire.status",
            )
            .expect("acquire project-global state cell for replacement")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex acquire returns status")
            .into_int_value();
        let okay = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                acquired,
                i32_ty.const_zero(),
                "project.global.set.acquired",
            )
            .expect("check project-global acquire status");
        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .expect("project-global replacement is emitted inside a function");
        let failed_block = self
            .context
            .append_basic_block(function, "project.global.set.failed");
        let success_block = self
            .context
            .append_basic_block(function, "project.global.set.success");
        let done_block = self
            .context
            .append_basic_block(function, "project.global.set.done");
        self.builder
            .build_conditional_branch(okay, success_block, failed_block)
            .expect("branch on project-global acquire status");

        self.builder.position_at_end(failed_block);
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join failed project-global replacement");

        self.builder.position_at_end(success_block);
        let payload_ty = self.module.project_globals[global as usize].payload;
        let payload_pointer = self
            .builder
            .build_load(ptr_ty, payload_out, "project.global.payload")
            .expect("load project-global payload address")
            .into_pointer_value();
        if let Some(event) = event {
            let event_ty = self.slot_types[&event.object_slot];
            let event_object = self
                .builder
                .build_load(
                    ptr_ty,
                    self.slots[&event.object_slot],
                    "project.global.event",
                )
                .expect("load project-global event object")
                .into_pointer_value();
            let previous_pointer = self.field_pointer_of_type(
                event_object,
                event_ty,
                event.previous_field,
                "project.global.event.previous",
                false,
            );
            let previous = self
                .builder
                .build_load(
                    self.llvm_type(payload_ty)
                        .expect("watched global payload has a value type"),
                    payload_pointer,
                    "project.global.previous",
                )
                .expect("read project-global previous value");
            self.builder
                .build_store(previous_pointer, previous)
                .expect("store project-global event previous value");
            let changed =
                self.emit_project_global_values_equal(previous, self.operand(value), payload_ty);
            let changed = self
                .builder
                .build_not(changed, "project.global.changed")
                .expect("invert project-global equality");
            let revision_pointer = self.field_pointer_of_type(
                event_object,
                event_ty,
                event.revision_field,
                "project.global.event.revision",
                false,
            );
            let event_size = self.byte_size_of(event_ty);
            self.builder
                .build_call(
                    self.runtime.global_watch_publish,
                    &[
                        handle.into(),
                        self.slots[&event.object_slot].into(),
                        event_size.into(),
                        revision_pointer.into(),
                        changed.into(),
                    ],
                    "project.global.watch.publish",
                )
                .expect("publish project-global change event");
        }
        self.builder
            .build_store(payload_pointer, self.operand(value))
            .expect("replace project-global snapshot");
        let release_status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.set.release.status.out")
            .expect("allocate project-global release status");
        self.builder
            .build_store(release_status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global release status");
        let released = self
            .builder
            .build_call(
                self.runtime.gc_mutex_release,
                &[handle.into(), release_status_out.into()],
                "project.global.set.release.status",
            )
            .expect("release project-global state cell after replacement")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex release returns status")
            .into_int_value();
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join successful project-global replacement");

        self.builder.position_at_end(done_block);
        let status = self
            .builder
            .build_phi(i32_ty, "project.global.set.status")
            .expect("merge project-global replacement status");
        status.add_incoming(&[(&acquired, failed_block), (&released, success_block)]);
        status.as_basic_value().into_int_value()
    }

    /// Equality used by watched project globals. The semantic checker admits
    /// only the same value families handled here: scalar equality, recursive
    /// tuple equality, and classes that provide `_equals`.
    fn emit_project_global_values_equal(
        &mut self,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        ty: ir::IrType,
    ) -> IntValue<'ctx> {
        match ty {
            ir::IrType::Value(id) => {
                let layout = self.module.values[id as usize].clone();
                let left = left.into_struct_value();
                let right = right.into_struct_value();
                let mut equal = self.context.bool_type().const_all_ones();
                for (index, field) in layout.fields.iter().enumerate() {
                    let left_field = self
                        .builder
                        .build_extract_value(left, index as u32, "global.equal.left.field")
                        .expect("extract watched tuple field");
                    let right_field = self
                        .builder
                        .build_extract_value(right, index as u32, "global.equal.right.field")
                        .expect("extract watched tuple field");
                    let field_equal =
                        self.emit_project_global_values_equal(left_field, right_field, field.ty);
                    equal = self
                        .builder
                        .build_and(equal, field_equal, "global.equal.fields")
                        .expect("combine watched tuple equality");
                }
                equal
            }
            ir::IrType::Object(id) => {
                let symbol = self.module.objects[id as usize]
                    .methods
                    .iter()
                    .find(|name| name.ends_with("$_equals"))
                    .expect("a watched class accepted by sema has `_equals`");
                self.builder
                    .build_call(
                        self.functions[symbol],
                        &[left.into(), right.into()],
                        "project.global.equals",
                    )
                    .expect("call watched class `_equals`")
                    .try_as_basic_value()
                    .basic()
                    .expect("class `_equals` returns a value")
                    .into_int_value()
            }
            ir::IrType::Decimal => {
                let ordering = self.emit_decimal_call(
                    "zirk_rt_decimal_cmp",
                    &[(left, ir::IrType::Decimal), (right, ir::IrType::Decimal)],
                    ir::IrType::Int(ir::IntWidth::I32),
                );
                self.builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        ordering.into_int_value(),
                        self.context.i32_type().const_zero(),
                        "project.global.decimal.equal",
                    )
                    .expect("compare watched decimals")
            }
            _ => self
                .emit_binary(ir::BinaryOp::Eq, left, right, false, ty)
                .into_int_value(),
        }
    }

    fn emit_project_global_watch_subscribe(
        &mut self,
        global: u32,
        channel: ir::Operand,
        snapshot_out: ir::SlotId,
        token_out: ir::SlotId,
        revision_out: ir::SlotId,
    ) -> IntValue<'ctx> {
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let handle = self
            .builder
            .build_load(
                ptr_ty,
                self.project_global_handles[global as usize],
                "project.global.watch.handle",
            )
            .expect("load project-global watch handle")
            .into_pointer_value();
        let payload_out = self
            .builder
            .build_alloca(ptr_ty, "project.global.watch.payload.out")
            .expect("allocate project-global watch payload pointer");
        let acquire_status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.watch.acquire.status.out")
            .expect("allocate project-global watch acquire status");
        self.builder
            .build_store(payload_out, ptr_ty.const_null())
            .expect("initialize project-global watch payload pointer");
        self.builder
            .build_store(acquire_status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global watch acquire status");
        let acquired = self
            .builder
            .build_call(
                self.runtime.gc_mutex_acquire,
                &[handle.into(), payload_out.into(), acquire_status_out.into()],
                "project.global.watch.acquire.status",
            )
            .expect("acquire project-global cell for watch registration")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex acquire returns status")
            .into_int_value();
        let okay = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                acquired,
                i32_ty.const_zero(),
                "project.global.watch.acquired",
            )
            .expect("check project-global watch acquire status");
        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .expect("project-global watch registration is emitted inside a function");
        let failed_block = self
            .context
            .append_basic_block(function, "project.global.watch.failed");
        let success_block = self
            .context
            .append_basic_block(function, "project.global.watch.success");
        let done_block = self
            .context
            .append_basic_block(function, "project.global.watch.done");
        self.builder
            .build_conditional_branch(okay, success_block, failed_block)
            .expect("branch on project-global watch acquire status");

        self.builder.position_at_end(failed_block);
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join failed project-global watch registration");

        self.builder.position_at_end(success_block);
        let payload_ty = self.module.project_globals[global as usize].payload;
        let payload_pointer = self
            .builder
            .build_load(ptr_ty, payload_out, "project.global.watch.payload")
            .expect("load project-global watch payload address")
            .into_pointer_value();
        let payload = self
            .builder
            .build_load(
                self.llvm_type(payload_ty)
                    .expect("global watch payload has a value type"),
                payload_pointer,
                "project.global.watch.snapshot.source",
            )
            .expect("load project-global watch snapshot");
        self.builder
            .build_store(self.slots[&snapshot_out], payload)
            .expect("root project-global watch snapshot payload");
        let registered = self
            .builder
            .build_call(
                self.runtime.global_watch_subscribe,
                &[
                    handle.into(),
                    self.operand(channel).into(),
                    self.slots[&token_out].into(),
                    self.slots[&revision_out].into(),
                ],
                "project.global.watch.subscribe.status",
            )
            .expect("register project-global watch channel")
            .try_as_basic_value()
            .basic()
            .expect("global watch subscribe returns status")
            .into_int_value();
        let registered_ok = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                registered,
                i32_ty.const_zero(),
                "project.global.watch.registered",
            )
            .expect("check project-global watch registration status");
        let release_status_out = self
            .builder
            .build_alloca(i32_ty, "project.global.watch.release.status.out")
            .expect("allocate project-global watch release status");
        self.builder
            .build_store(release_status_out, i32_ty.const_int(4, false))
            .expect("initialize project-global watch release status");
        let released = self
            .builder
            .build_call(
                self.runtime.gc_mutex_release,
                &[handle.into(), release_status_out.into()],
                "project.global.watch.release.status",
            )
            .expect("release project-global cell after watch registration")
            .try_as_basic_value()
            .basic()
            .expect("GC mutex release returns status")
            .into_int_value();
        let success_status = self
            .builder
            .build_select(
                registered_ok,
                released,
                registered,
                "project.global.watch.success.status",
            )
            .expect("select project-global watch registration status")
            .into_int_value();
        self.builder
            .build_unconditional_branch(done_block)
            .expect("join successful project-global watch registration");

        self.builder.position_at_end(done_block);
        let status = self
            .builder
            .build_phi(i32_ty, "project.global.watch.status")
            .expect("merge project-global watch registration status");
        status.add_incoming(&[(&acquired, failed_block), (&success_status, success_block)]);
        status.as_basic_value().into_int_value()
    }

    fn emit_project_global_watch_next(
        &mut self,
        token: ir::Operand,
        out: ir::SlotId,
    ) -> IntValue<'ctx> {
        self.builder
            .build_call(
                self.runtime.global_watch_next,
                &[
                    self.operand(token).into_int_value().into(),
                    self.slots[&out].into(),
                ],
                "project.global.watch.next.status",
            )
            .expect("read project-global watch event")
            .try_as_basic_value()
            .basic()
            .expect("global watch next returns status")
            .into_int_value()
    }

    fn emit_project_global_watch_close(&mut self, token: ir::Operand) {
        self.builder
            .build_call(
                self.runtime.global_watch_close,
                &[self.operand(token).into_int_value().into()],
                "project.global.watch.close",
            )
            .expect("close project-global subscription");
    }

    /// The byte size `ty` occupies, as an LLVM `i64` constant expression
    /// (roadmap Phase 4e, `fase-4e-unsafe-journal`, design D3) — `zirk_rt_journal_record`'s
    /// own `len` argument, resolved from LLVM's own data layout the same way
    /// `InstKind::Alloc`'s own size argument already is (`StructType::size_of`),
    /// rather than a hand-written per-`IrType` table.
    fn byte_size_of(&self, ty: ir::IrType) -> inkwell::values::IntValue<'ctx> {
        self.llvm_type(ty)
            .expect("a journaled write's target is not Void")
            .size_of()
            .expect("a journaled write's target is sized")
    }

    /// Alignment of `ty` in bytes, as an LLVM `i64` value.
    fn byte_align_of(&self, ty: ir::IrType) -> inkwell::values::IntValue<'ctx> {
        let llvm = self.llvm_type(ty).expect("a sized target is not Void");
        let bits = llvm.get_alignment();
        self.builder
            .build_int_z_extend(bits, self.context.i64_type(), "align")
            .expect("align to i64")
    }

    /// Whether `ty` is a single GC-managed pointer the runtime should trace.
    fn is_gc_reference(&self, ty: ir::IrType) -> bool {
        matches!(
            ty,
            ir::IrType::String
                | ir::IrType::Char
                | ir::IrType::Regex
                | ir::IrType::Object(_)
                | ir::IrType::Contract(_)
                | ir::IrType::ObjectAny
                | ir::IrType::Dynamic
                | ir::IrType::Weak(_)
                | ir::IrType::Dependent(_)
                | ir::IrType::Pin(_)
                | ir::IrType::Array(_)
                | ir::IrType::List(_)
                | ir::IrType::Channel(_)
                | ir::IrType::Atomic(_)
                | ir::IrType::Mutex(_)
        )
    }

    /// Element `IrType` and metadata for an `Array<T>` or `List<T>` id.
    fn array_list_element(&self, ty: ir::IrType) -> (u32, ir::IrType, bool) {
        let (id, table) = match ty {
            ir::IrType::Array(id) => (id, &self.module.array_types),
            ir::IrType::List(id) => (id, &self.module.list_types),
            _ => panic!("expected Array/List, got {ty:?}"),
        };
        let element = table[id as usize];
        let is_ref = self.is_gc_reference(element);
        (id, element, is_ref)
    }

    /// Builds an `i1` boolean constant for the runtime's `is_ref` flag.
    fn bool_const(&self, value: bool) -> inkwell::values::IntValue<'ctx> {
        self.context.bool_type().const_int(value as u64, false)
    }

    fn atomic_pack(&self, operand: ir::Operand) -> inkwell::values::IntValue<'ctx> {
        let value = self.operand(operand).into_int_value();
        let target = self.context.i64_type();
        if value.get_type().get_bit_width() == 64 {
            value
        } else if value.get_type().get_bit_width() < 64 {
            let signed = self.value_type(operand).is_some_and(|ty| {
                matches!(ty,
                ir::IrType::Int(width) if width.signed())
            });
            if signed {
                self.builder
                    .build_int_s_extend(value, target, "atomic.pack")
                    .expect("extend atomic payload")
            } else {
                self.builder
                    .build_int_z_extend(value, target, "atomic.pack")
                    .expect("extend atomic payload")
            }
        } else {
            self.builder
                .build_int_truncate(value, target, "atomic.pack")
                .expect("truncate atomic payload")
        }
    }

    fn atomic_unpack_to_slot(&mut self, packed: inkwell::values::IntValue<'ctx>, slot: ir::SlotId) {
        let ty = self.module_function_slot_type(slot);
        let llvm_ty = self
            .llvm_type(ty)
            .expect("atomic scalar output has LLVM type")
            .into_int_type();
        let value = if llvm_ty.get_bit_width() == 1 {
            self.builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    packed,
                    self.context.i64_type().const_zero(),
                    "atomic.bool",
                )
                .expect("convert atomic boolean")
        } else if llvm_ty.get_bit_width() == 64 {
            packed
        } else {
            self.builder
                .build_int_truncate(packed, llvm_ty, "atomic.unpack")
                .expect("truncate atomic result")
        };
        self.builder
            .build_store(self.slots[&slot], value)
            .expect("store atomic result");
    }

    fn value_type(&self, operand: ir::Operand) -> Option<ir::IrType> {
        self.value_types.get(&operand.0).copied()
    }

    fn module_function_slot_type(&self, slot: ir::SlotId) -> ir::IrType {
        self.slot_types[&slot]
    }

    /// Spills `value` to a stack slot of `element` type and returns its
    /// address; used to pass element values by pointer to list helpers.
    fn element_value_ptr(
        &self,
        value: inkwell::values::BasicValueEnum<'ctx>,
        element: ir::IrType,
    ) -> inkwell::values::PointerValue<'ctx> {
        let ty = self
            .llvm_type(element)
            .expect("an element type has an LLVM representation");
        let slot = self.builder.build_alloca(ty, "list_value").expect("alloca");
        self.builder
            .build_store(slot, value)
            .expect("store element");
        slot
    }

    /// The LLVM struct that describes the layout of a closure's captured
    /// values (without the GC header that `zirk_rt_alloc_callable` prefixes).
    fn closure_payload_struct(
        &self,
        layout: &ir::ClosureLayout,
    ) -> inkwell::types::StructType<'ctx> {
        let payload_types: Vec<BasicTypeEnum> = layout
            .captures
            .iter()
            .map(|&ty| {
                self.llvm_type(ty)
                    .expect("a capture has an LLVM representation")
            })
            .collect();
        self.context.struct_type(&payload_types, false)
    }

    /// Byte offset from the start of a capture block's payload to the
    /// `index`-th captured value, including any alignment padding LLVM inserts
    /// in the payload struct, plus the three-word GC header that precedes it.
    fn closure_capture_offset(
        &self,
        layout: &ir::ClosureLayout,
        index: usize,
    ) -> inkwell::values::IntValue<'ctx> {
        let payload_struct = self.closure_payload_struct(layout);
        let field_offset = const_field_offset(self.context, payload_struct, &[index as u32]);
        let header = self
            .context
            .i64_type()
            .const_int(CAPTURE_BLOCK_HEADER_BYTES as u64, false);
        field_offset.const_add(header)
    }

    /// Builds the synchronous `(payload, context) -> void` adapter required by
    /// the GC mutex ABI. The context owns only a stack copy of the callable and
    /// result address; a temporary shadow-stack frame roots its capture block
    /// across lock acquisition, which may wait and permit collection.
    fn mutex_with_thunk(&self, layout_id: u32) -> FunctionValue<'ctx> {
        let name = format!("zirk.mutex.with.thunk.{layout_id}");
        if let Some(existing) = self.llvm.get_function(&name) {
            return existing;
        }
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let thunk = self.llvm.add_function(
            &name,
            self.context
                .void_type()
                .fn_type(&[ptr.into(), ptr.into()], false),
            Some(Linkage::Internal),
        );
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(thunk, "entry");
        builder.position_at_end(entry);
        let callable_type = callable_struct(self.context);
        let context_type = self
            .context
            .struct_type(&[callable_type.into(), ptr.into()], false);
        let context_ptr = thunk
            .get_nth_param(1)
            .expect("mutex context")
            .into_pointer_value();
        let context_value = builder
            .build_load(context_type, context_ptr, "mutex.context")
            .expect("load mutex context")
            .into_struct_value();
        let callable = builder
            .build_extract_value(context_value, 0, "mutex.callable")
            .expect("extract callable")
            .into_struct_value();
        let result_out = builder
            .build_extract_value(context_value, 1, "mutex.result.out")
            .expect("extract result output")
            .into_pointer_value();
        let fn_ptr = builder
            .build_extract_value(callable, CALLABLE_FUNCTION_FIELD, "mutex.callback.fn")
            .expect("extract callable function")
            .into_pointer_value();
        let captures = builder
            .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "mutex.callback.captures")
            .expect("extract callable captures")
            .into_pointer_value();
        let mut arguments: Vec<BasicMetadataValueEnum> =
            Vec::with_capacity(layout.captures.len() + 1);
        for (index, capture_ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let address = unsafe {
                builder.build_gep(
                    self.context.i8_type(),
                    captures,
                    &[offset],
                    "mutex.capture.addr",
                )
            }
            .expect("capture address");
            let ty = self.llvm_type(*capture_ty).expect("capture has LLVM type");
            arguments.push(
                builder
                    .build_load(ty, address, "mutex.capture")
                    .expect("load capture")
                    .into(),
            );
        }
        let payload = thunk.get_nth_param(0).expect("mutex payload");
        arguments.push(payload.into());
        let signature = closure_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let call = builder
            .build_indirect_call(signature, fn_ptr, &arguments, "mutex.callback")
            .expect("call mutex callback");
        if layout.returns != ir::IrType::Void {
            let returned = call
                .try_as_basic_value()
                .basic()
                .expect("non-Void mutex callback result");
            let ty = self
                .llvm_type(layout.returns)
                .expect("callback result type");
            builder
                .build_store(result_out, returned)
                .expect("store callback result");
            // Keep the typed store visible to LLVM's verifier for opaque ptrs.
            let _ = ty;
        }
        builder
            .build_return(None)
            .expect("return from mutex adapter");
        thunk
    }

    fn http_server_handler_thunk(
        &self,
        layout_id: u32,
        request_class_id: u32,
        response_class_id: u32,
    ) -> FunctionValue<'ctx> {
        let name =
            format!("zirk.http.server.thunk.{layout_id}.{request_class_id}.{response_class_id}");
        if let Some(existing) = self.llvm.get_function(&name) {
            return existing;
        }
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let thunk = self.llvm.add_function(
            &name,
            i32_type.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            Some(Linkage::Internal),
        );
        // HTTP server callbacks use `C-unwind` only for cooperative Zirk task
        // cancellation. Keep unwind metadata on this generated owner-task
        // thunk so cancellation can reach the runtime's per-stream join.
        set_uwtable(self.context, thunk);
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(thunk, "entry");
        builder.position_at_end(entry);

        let request_layout = &self.module.objects[request_class_id as usize];
        let request_type = object_struct(
            self.context,
            request_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let request_object = builder
            .build_call(
                self.runtime.alloc,
                &[
                    request_type
                        .size_of()
                        .expect("HTTPRequest object size")
                        .into(),
                    request_type.get_alignment().into(),
                ],
                "http.request.object",
            )
            .expect("allocate HTTPRequest")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        builder
            .build_store(request_object, self.descriptors[request_class_id as usize])
            .expect("write HTTPRequest type descriptor");
        let raw_request_type = self
            .context
            .struct_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false);
        let raw_request = thunk
            .get_nth_param(0)
            .expect("HTTP request")
            .into_pointer_value();
        for index in 0..request_layout.fields.len() {
            let raw_field = builder
                .build_struct_gep(
                    raw_request_type,
                    raw_request,
                    index as u32,
                    "http.request.raw.field",
                )
                .expect("address raw HTTP request field");
            let field_value = builder
                .build_load(ptr, raw_field, "http.request.field")
                .expect("load raw HTTP request field");
            let object_field = builder
                .build_struct_gep(
                    request_type,
                    request_object,
                    index as u32 + OBJECT_HEADER_FIELDS,
                    "http.request.object.field",
                )
                .expect("address HTTPRequest field");
            builder
                .build_store(object_field, field_value)
                .expect("store HTTPRequest field");
        }

        let context_type = self
            .context
            .struct_type(&[callable_struct(self.context).into()], false);
        let context_ptr = thunk
            .get_nth_param(2)
            .expect("HTTP handler context")
            .into_pointer_value();
        let context_value = builder
            .build_load(context_type, context_ptr, "http.handler.context")
            .expect("load handler context")
            .into_struct_value();
        let callable = builder
            .build_extract_value(context_value, 0, "http.handler.callable")
            .expect("extract handler callable")
            .into_struct_value();
        let dispatch = builder
            .build_extract_value(callable, CALLABLE_DISPATCH_FIELD, "http.handler.dispatch")
            .expect("extract handler dispatch entry")
            .into_pointer_value();
        let captures = builder
            .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http.handler.captures")
            .expect("extract handler captures")
            .into_pointer_value();
        let arguments: Vec<BasicMetadataValueEnum> = vec![captures.into(), request_object.into()];
        let signature = callable_dispatch_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let response = builder
            .build_indirect_call(signature, dispatch, &arguments, "http.handler.call")
            .expect("invoke Zirk HTTP handler")
            .try_as_basic_value()
            .basic()
            .expect("HTTP handler returns response object")
            .into_pointer_value();

        // A cooperative cancellation (or another propagated exception) can
        // make a Zirk handler return the null sentinel. This callback runs
        // across a native ABI boundary, so reject that result before loading
        // fields from the managed response object.
        let response_is_null = builder
            .build_is_null(response, "http.handler.response.is_null")
            .expect("check HTTP handler response");
        let has_pending_exception = builder
            .build_call(
                self.runtime.has_pending_exception,
                &[],
                "http.handler.has_pending_exception",
            )
            .expect("check HTTP handler exception")
            .try_as_basic_value()
            .basic()
            .expect("pending exception predicate")
            .into_int_value();
        let failed = builder
            .build_or(
                response_is_null,
                has_pending_exception,
                "http.handler.failed",
            )
            .expect("combine HTTP handler failure checks");
        let failure = self.context.append_basic_block(thunk, "handler_failed");
        let succeeded = self.context.append_basic_block(thunk, "handler_succeeded");
        builder
            .build_conditional_branch(failed, failure, succeeded)
            .expect("branch on HTTP handler failure");
        builder.position_at_end(failure);
        builder
            .build_return(Some(&i32_type.const_int(1, false)))
            .expect("return failed HTTP handler status");
        builder.position_at_end(succeeded);

        let response_layout = &self.module.objects[response_class_id as usize];
        let response_type = object_struct(
            self.context,
            response_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let response_out_type = self.context.struct_type(
            &[
                i32_type.into(),
                ptr.into(),
                ptr.into(),
                self.context.i64_type().into(),
                i32_type.into(),
            ],
            false,
        );
        let response_out = thunk
            .get_nth_param(1)
            .expect("HTTP response output")
            .into_pointer_value();
        for (out_index, field_name) in [
            (0u32, "status_code_value"),
            (1, "header_data"),
            (2, "body_value"),
            (3, "server_producer_handle"),
            (4, "server_sse_heartbeat_ms"),
        ] {
            let field_index = response_layout
                .field_index(field_name)
                .expect("HTTPResponse field exists");
            let source = builder
                .build_struct_gep(
                    response_type,
                    response,
                    field_index as u32 + OBJECT_HEADER_FIELDS,
                    "http.response.object.field",
                )
                .expect("address HTTPResponse field");
            let value = builder
                .build_load(
                    self.llvm_type(response_layout.fields[field_index].ty)
                        .expect("HTTPResponse field type"),
                    source,
                    "http.response.field",
                )
                .expect("load HTTPResponse field");
            let destination = builder
                .build_struct_gep(
                    response_out_type,
                    response_out,
                    out_index,
                    "http.response.out.field",
                )
                .expect("address C HTTP response field");
            builder
                .build_store(destination, value)
                .expect("store C HTTP response field");
        }
        builder
            .build_return(Some(&i32_type.const_zero()))
            .expect("return from HTTP thunk");
        thunk
    }

    fn http_websocket_server_handler_thunk(
        &self,
        layout_id: u32,
        websocket_class_id: u32,
    ) -> FunctionValue<'ctx> {
        let name = format!("zirk.http.websocket.server.thunk.{layout_id}.{websocket_class_id}");
        if let Some(existing) = self.llvm.get_function(&name) {
            return existing;
        }
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();
        let thunk = self.llvm.add_function(
            &name,
            i32_type.fn_type(&[i64_type.into(), ptr.into()], false),
            Some(Linkage::Internal),
        );
        set_uwtable(self.context, thunk);
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(thunk, "entry");
        builder.position_at_end(entry);

        let websocket_layout = &self.module.objects[websocket_class_id as usize];
        let websocket_type = object_struct(
            self.context,
            websocket_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let websocket = builder
            .build_call(
                self.runtime.alloc,
                &[
                    websocket_type.size_of().expect("HTTPWebSocket size").into(),
                    websocket_type.get_alignment().into(),
                ],
                "http.websocket.server.object",
            )
            .expect("allocate HTTPWebSocket")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        builder
            .build_store(websocket, self.descriptors[websocket_class_id as usize])
            .expect("write HTTPWebSocket type descriptor");
        let handle_index = websocket_layout
            .fields
            .iter()
            .position(|field| field.name == "handle")
            .expect("HTTPWebSocket exposes compiler-owned handle")
            as u32;
        let handle_field = builder
            .build_struct_gep(
                websocket_type,
                websocket,
                handle_index + OBJECT_HEADER_FIELDS,
                "http.websocket.server.handle",
            )
            .expect("address HTTPWebSocket handle field");
        builder
            .build_store(
                handle_field,
                thunk.get_nth_param(0).expect("WebSocket handle"),
            )
            .expect("store HTTPWebSocket handle");

        let context_type = self
            .context
            .struct_type(&[callable_struct(self.context).into()], false);
        let context_ptr = thunk
            .get_nth_param(1)
            .expect("HTTP WebSocket handler context")
            .into_pointer_value();
        let context_value = builder
            .build_load(context_type, context_ptr, "http.websocket.handler.context")
            .expect("load WebSocket handler context")
            .into_struct_value();
        let callable = builder
            .build_extract_value(context_value, 0, "http.websocket.handler.callable")
            .expect("extract WebSocket handler callable")
            .into_struct_value();
        let dispatch = builder
            .build_extract_value(
                callable,
                CALLABLE_DISPATCH_FIELD,
                "http.websocket.handler.dispatch",
            )
            .expect("extract WebSocket handler dispatch")
            .into_pointer_value();
        let captures = builder
            .build_extract_value(
                callable,
                CALLABLE_CAPTURE_FIELD,
                "http.websocket.handler.captures",
            )
            .expect("extract WebSocket handler captures")
            .into_pointer_value();
        let signature = callable_dispatch_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        builder
            .build_indirect_call(
                signature,
                dispatch,
                &[captures.into(), websocket.into()],
                "http.websocket.handler.call",
            )
            .expect("invoke Zirk HTTPWebSocket handler");
        builder
            .build_return(Some(&i32_type.const_zero()))
            .expect("return from HTTP WebSocket thunk");
        thunk
    }

    fn http_upload_producer_thunk(
        &self,
        layout_id: u32,
        result_callback: bool,
    ) -> FunctionValue<'ctx> {
        let name = format!("zirk.http.upload.producer.thunk.{layout_id}.{result_callback}");
        if let Some(existing) = self.llvm.get_function(&name) {
            return existing;
        }
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let thunk = self.llvm.add_function(
            &name,
            i32_type.fn_type(
                &[ptr.into(), self.context.i64_type().into(), ptr.into()],
                false,
            ),
            Some(Linkage::Internal),
        );
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(thunk, "entry");
        builder.position_at_end(entry);

        let context_type = self
            .context
            .struct_type(&[callable_struct(self.context).into()], false);
        let context_ptr = thunk
            .get_nth_param(0)
            .expect("HTTP producer context")
            .into_pointer_value();
        let context_value = builder
            .build_load(context_type, context_ptr, "http.producer.context")
            .expect("load HTTP producer context")
            .into_struct_value();
        let callable = builder
            .build_extract_value(context_value, 0, "http.producer.callable")
            .expect("extract HTTP producer callable")
            .into_struct_value();
        let dispatch = builder
            .build_extract_value(callable, CALLABLE_DISPATCH_FIELD, "http.producer.dispatch")
            .expect("extract HTTP producer dispatch")
            .into_pointer_value();
        let captures = builder
            .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http.producer.captures")
            .expect("extract HTTP producer captures")
            .into_pointer_value();
        let signature = callable_dispatch_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let produced_value = builder
            .build_indirect_call(
                signature,
                dispatch,
                &[
                    captures.into(),
                    thunk
                        .get_nth_param(1)
                        .expect("HTTP producer chunk index")
                        .into(),
                ],
                "http.producer.call",
            )
            .expect("invoke Zirk HTTP body producer")
            .try_as_basic_value()
            .basic()
            .expect("HTTP producer returns its declared body value");
        let output = thunk
            .get_nth_param(2)
            .expect("HTTP producer output")
            .into_pointer_value();
        if result_callback {
            let ir::IrType::Enum(result_id) = layout.returns else {
                unreachable!("typed HTTP producer returns Result<Array<Byte>, Error>")
            };
            let result_layout = &self.module.enums[result_id as usize];
            let returned = produced_value.into_struct_value();
            let tag = builder
                .build_extract_value(returned, 0, "http.producer.result.tag")
                .expect("extract HTTP producer result tag")
                .into_int_value();
            let is_ok = builder
                .build_int_compare(
                    IntPredicate::EQ,
                    tag,
                    i32_type.const_zero(),
                    "http.producer.result.is_ok",
                )
                .expect("test HTTP producer result tag");
            let ok_block = self.context.append_basic_block(thunk, "producer.ok");
            let error_block = self.context.append_basic_block(thunk, "producer.error");
            builder
                .build_conditional_branch(is_ok, ok_block, error_block)
                .expect("branch on HTTP producer result");

            builder.position_at_end(ok_block);
            let value_field = result_layout.variants[0][0] + ENUM_HEADER_FIELDS;
            let produced_bytes = builder
                .build_extract_value(returned, value_field, "http.producer.result.bytes")
                .expect("extract HTTP producer bytes")
                .into_pointer_value();
            builder
                .build_store(output, produced_bytes)
                .expect("store HTTP producer bytes");
            builder
                .build_return(Some(&i32_type.const_zero()))
                .expect("return successful HTTP producer result");

            builder.position_at_end(error_block);
            let error_field = result_layout.variants[1][0] + ENUM_HEADER_FIELDS;
            let error_tag = builder
                .build_extract_value(returned, error_field, "http.producer.result.error")
                .expect("extract HTTP producer error")
                .into_int_value();
            let status = builder
                .build_int_add(
                    error_tag,
                    i32_type.const_int(1, false),
                    "http.producer.error.status",
                )
                .expect("encode HTTP producer error status");
            builder
                .build_return(Some(&status))
                .expect("return HTTP producer error status");
        } else {
            let produced = produced_value.into_pointer_value();
            builder
                .build_store(output, produced)
                .expect("store HTTP producer chunk");
            builder
                .build_return(Some(&i32_type.const_zero()))
                .expect("return from HTTP producer thunk");
        }
        thunk
    }

    /// Adapts one concrete closure body to the uniform boxed-callable entry
    /// ABI. Concrete target/capture fields stay available to scheduler and C
    /// callback thunks that already know the closure layout.
    fn build_callable_dispatch_adapter(
        &self,
        target_name: &str,
        layout_id: u32,
    ) -> FunctionValue<'ctx> {
        let name = format!("zk.callable.dispatch.{layout_id}.{target_name}");
        if let Some(existing) = self.llvm.get_function(&name) {
            return existing;
        }
        let layout = &self.module.closures[layout_id as usize];
        let signature = callable_dispatch_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let adapter = self
            .llvm
            .add_function(&name, signature, Some(Linkage::Internal));
        adapter.set_call_conventions(0);
        set_uwtable(self.context, adapter);
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(adapter, "entry");
        builder.position_at_end(entry);

        let capture_block = adapter
            .get_first_param()
            .expect("uniform callable capture block")
            .into_pointer_value();
        let mut arguments: Vec<BasicMetadataValueEnum> =
            Vec::with_capacity(layout.captures.len() + layout.params.len());
        for (index, capture_ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let address = unsafe {
                builder.build_gep(
                    self.context.i8_type(),
                    capture_block,
                    &[offset],
                    "dispatch.capture.address",
                )
            }
            .expect("dispatch capture address");
            let ty = self.llvm_type(*capture_ty).expect("capture LLVM type");
            arguments.push(
                builder
                    .build_load(ty, address, "dispatch.capture")
                    .expect("load dispatch capture")
                    .into(),
            );
        }
        for index in 0..layout.params.len() {
            arguments.push(
                adapter
                    .get_nth_param(index as u32 + 1)
                    .expect("callable source parameter")
                    .into(),
            );
        }

        let target = self
            .functions
            .get(target_name)
            .copied()
            .expect("callable target was declared before code generation")
            .as_global_value()
            .as_pointer_value();
        let target_signature = closure_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let call = builder
            .build_indirect_call(target_signature, target, &arguments, "dispatch.target")
            .expect("call concrete closure target");
        if layout.returns == ir::IrType::Void {
            builder
                .build_return(None)
                .expect("return from void adapter");
        } else {
            let value = call
                .try_as_basic_value()
                .basic()
                .expect("callable result value");
            builder
                .build_return(Some(&value))
                .expect("return callable result");
        }
        adapter
    }

    /// Adapts a service callback to the runtime's stable `u64` thunk ABI.
    fn build_service_thunk(
        &self,
        target_name: &str,
        layout_id: u32,
        site: &str,
    ) -> FunctionValue<'ctx> {
        let layout = &self.module.closures[layout_id as usize];
        assert_eq!(layout.params, [ir::IrType::Int(ir::IntWidth::U64)]);
        assert_eq!(layout.returns, ir::IrType::Void);
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        let thunk = self.llvm.add_function(
            &format!("zk.service_thunk.{site}"),
            i64_type.fn_type(&[i64_type.into(), ptr.into()], false),
            Some(Linkage::Private),
        );
        thunk.set_call_conventions(0);
        set_uwtable(self.context, thunk);
        let resume = self
            .builder
            .get_insert_block()
            .expect("service start block");
        let entry = self.context.append_basic_block(thunk, "entry");
        self.builder.position_at_end(entry);
        let service_id = thunk.get_nth_param(0).expect("service id").into_int_value();
        let capture = thunk
            .get_nth_param(1)
            .expect("capture block")
            .into_pointer_value();
        let mut arguments: Vec<BasicMetadataValueEnum> =
            Vec::with_capacity(layout.captures.len() + 1);
        for (index, capture_ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let address = unsafe {
                self.builder.build_gep(
                    self.context.i8_type(),
                    capture,
                    &[offset],
                    "service_capture_addr",
                )
            }
            .expect("service capture address");
            let ty = self.llvm_type(*capture_ty).expect("service capture type");
            arguments.push(
                self.builder
                    .build_load(ty, address, "service_capture_value")
                    .expect("load rooted service capture")
                    .into(),
            );
        }
        arguments.push(service_id.into());
        let target = self
            .functions
            .get(target_name)
            .copied()
            .expect("service target is a lowered function");
        self.builder
            .build_call(target, &arguments, "service_callback")
            .expect("call service callback");
        let pending = self
            .builder
            .build_call(self.runtime.has_pending_exception, &[], "service_threw")
            .expect("zirk_rt_has_pending_exception")
            .try_as_basic_value()
            .basic()
            .expect("pending exception predicate")
            .into_int_value();
        let failed = self.context.append_basic_block(thunk, "failed");
        let returned = self.context.append_basic_block(thunk, "returned");
        self.builder
            .build_conditional_branch(pending, failed, returned)
            .expect("branch on service callback result");
        self.builder.position_at_end(failed);
        self.builder
            .build_call(
                self.runtime.service_failed_current,
                &[],
                "service_failure_status",
            )
            .expect("zirk_rt_service_failed_current");
        self.builder
            .build_return(Some(&i64_type.const_int(1, false)))
            .expect("return failed service status");
        self.builder.position_at_end(returned);
        self.builder
            .build_return(Some(&i64_type.const_zero()))
            .expect("return successful service status");
        self.builder.position_at_end(resume);
        thunk
    }

    /// Builds a C ABI adapter for one verified zero-argument callable. The
    /// runtime invokes this only on a registered managed worker and joins it
    /// before the caller's rooted output slots can leave scope.
    fn thread_run_thunk(&self, layout_id: u32) -> FunctionValue<'ctx> {
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let thunk_type = i32_type.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false);
        let thunk =
            self.llvm
                .add_function("zirk.thread.run.thunk", thunk_type, Some(Linkage::Internal));
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(thunk, "entry");
        builder.position_at_end(entry);

        let target = thunk
            .get_nth_param(0)
            .expect("thread target")
            .into_pointer_value();
        let captures = thunk
            .get_nth_param(1)
            .expect("thread captures")
            .into_pointer_value();
        let result_out = thunk
            .get_nth_param(2)
            .expect("thread result slot")
            .into_pointer_value();
        let exception_out = thunk
            .get_nth_param(3)
            .expect("thread exception slot")
            .into_pointer_value();

        let mut arguments = Vec::with_capacity(layout.captures.len());
        for (index, capture_ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            // SAFETY: the callable's verified layout bounds every capture
            // offset, and `captures` points to its GC-header-prefixed block.
            let address = unsafe {
                builder.build_gep(
                    self.context.i8_type(),
                    captures,
                    &[offset],
                    "capture.address",
                )
            }
            .expect("capture address");
            let llvm_ty = self.llvm_type(*capture_ty).expect("capture LLVM type");
            let value = builder
                .build_load(llvm_ty, address, "capture.value")
                .expect("load capture");
            arguments.push(value.into());
        }
        let signature = closure_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let callback = builder
            .build_indirect_call(signature, target, &arguments, "thread.callback")
            .expect("call managed callback");
        let pending = builder
            .build_call(self.runtime.has_pending_exception, &[], "callback.threw")
            .expect("check callback exception")
            .try_as_basic_value()
            .basic()
            .expect("exception predicate")
            .into_int_value();
        let threw = self.context.append_basic_block(thunk, "threw");
        let succeeded = self.context.append_basic_block(thunk, "succeeded");
        builder
            .build_conditional_branch(pending, threw, succeeded)
            .expect("branch on callback exception");

        builder.position_at_end(threw);
        let exception = builder
            .build_call(
                self.runtime.take_pending_exception,
                &[],
                "callback.exception",
            )
            .expect("take callback exception")
            .try_as_basic_value()
            .basic()
            .expect("exception value");
        builder
            .build_store(exception_out, exception)
            .expect("publish callback exception");
        builder
            .build_return(Some(&i32_type.const_int(1, false)))
            .expect("return thrown status");

        builder.position_at_end(succeeded);
        if let Some(value) = callback.try_as_basic_value().basic() {
            builder
                .build_store(result_out, value)
                .expect("publish callback result");
        }
        builder
            .build_return(Some(&i32_type.const_zero()))
            .expect("return success status");
        thunk
    }

    /// Decodes the descriptor-backed box returned by the runtime for one
    /// non-void task result. Inline records/enums are loaded at their exact
    /// LLVM layout; managed references remain covered by the box descriptor.
    fn narrow_from_branch_word(
        &self,
        raw: inkwell::values::IntValue<'ctx>,
        ty: ir::IrType,
    ) -> Option<BasicValueEnum<'ctx>> {
        match ty {
            ir::IrType::Void | ir::IrType::Never => None,
            _ => {
                let pointer = self
                    .builder
                    .build_int_to_ptr(
                        raw,
                        self.context.ptr_type(AddressSpace::default()),
                        "job_box",
                    )
                    .expect("job result box");
                let box_type = task_result_box_type(self.context, self.module, ty);
                let payload = self
                    .builder
                    .build_struct_gep(box_type, pointer, OBJECT_HEADER_FIELDS, "job_payload")
                    .expect("job payload field");
                Some(
                    self.builder
                        .build_load(
                            self.llvm_type(ty).expect("task result type"),
                            payload,
                            "job_value",
                        )
                        .expect("load task result"),
                )
            }
        }
    }

    /// Adapts a boxed managed callable to the executor's typed background
    /// error callback ABI. The callback is invoked only from a scheduler task,
    /// so suspension and cleanup retain a normal Zirk task context.
    fn build_background_error_thunk(&self, layout_id: u32) -> FunctionValue<'ctx> {
        let layout = &self.module.closures[layout_id as usize];
        let ptr = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let thunk = self.llvm.add_function(
            "zk.background_error_thunk",
            i32_type.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            Some(Linkage::Private),
        );
        thunk.set_call_conventions(0);
        set_uwtable(self.context, thunk);
        let resume = self.builder.get_insert_block().expect("instruction block");
        let entry = self.context.append_basic_block(thunk, "entry");
        self.builder.position_at_end(entry);

        let target = thunk
            .get_nth_param(0)
            .expect("callable target")
            .into_pointer_value();
        let capture = thunk
            .get_nth_param(1)
            .expect("capture block")
            .into_pointer_value();
        let throwable = thunk.get_nth_param(2).expect("Throwable");
        let mut arguments: Vec<BasicMetadataValueEnum> =
            Vec::with_capacity(layout.captures.len() + 1);
        for (index, capture_ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let address = unsafe {
                self.builder.build_gep(
                    self.context.i8_type(),
                    capture,
                    &[offset],
                    "error_handler_capture",
                )
            }
            .expect("capture address");
            let ty = self.llvm_type(*capture_ty).expect("capture LLVM type");
            arguments.push(
                self.builder
                    .build_load(ty, address, "capture")
                    .expect("capture load")
                    .into(),
            );
        }
        arguments.push(throwable.into());
        let signature = closure_signature(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        self.builder
            .build_indirect_call(signature, target, &arguments, "background_error_handler")
            .expect("call managed background error handler");
        let pending = self
            .builder
            .build_call(self.runtime.has_pending_exception, &[], "handler_threw")
            .expect("zirk_rt_has_pending_exception")
            .try_as_basic_value()
            .basic()
            .expect("exception predicate")
            .into_int_value();
        let threw = self.context.append_basic_block(thunk, "handler_threw");
        let returned = self.context.append_basic_block(thunk, "handler_returned");
        self.builder
            .build_conditional_branch(pending, threw, returned)
            .expect("handler exception branch");

        self.builder.position_at_end(threw);
        let exception = self
            .builder
            .build_call(
                self.runtime.take_pending_exception,
                &[],
                "handler_exception",
            )
            .expect("zirk_rt_take_pending_exception")
            .try_as_basic_value()
            .basic()
            .expect("exception value");
        self.builder
            .build_call(
                self.runtime.background_handler_failed,
                &[exception.into()],
                "",
            )
            .expect("zirk_rt_background_handler_failed");
        self.builder
            .build_return(Some(&i32_type.const_int(1, false)))
            .expect("handler failed status");

        self.builder.position_at_end(returned);
        self.builder
            .build_return(Some(&i32_type.const_zero()))
            .expect("handler success status");
        self.builder.position_at_end(resume);
        thunk
    }

    /// Builds the per-site `extern "C" fn(ptr) -> i64` thunk a `BranchStart`
    /// hands to `zirk_rt_spawn` / `zirk_rt_timer_*`: it reloads the captured
    /// values from the boxed capture block, calls through the callable's own
    /// function pointer, turns a pending Zirk exception into a branch failure,
    /// and boxes the normal result in a descriptor-backed GC allocation. The
    /// builder's insert position is saved and restored.
    ///
    fn build_branch_thunk(&self, target: &str, layout_id: u32, site: u32) -> FunctionValue<'ctx> {
        self.build_branch_thunk_named(target, layout_id, site, &format!("zk.branch_thunk.{site}"))
    }

    fn build_branch_thunk_named(
        &self,
        target: &str,
        layout_id: u32,
        site: u32,
        thunk_name: &str,
    ) -> FunctionValue<'ctx> {
        let layout = &self.module.closures[layout_id as usize];
        let returns = self
            .module
            .function(target)
            .expect("branch target is a module function")
            .return_type;

        let thunk = self.llvm.add_function(
            thunk_name,
            self.context.i64_type().fn_type(
                &[self.context.ptr_type(AddressSpace::default()).into()],
                false,
            ),
            Some(Linkage::Private),
        );
        thunk.set_call_conventions(0);
        set_uwtable(self.context, thunk);

        let resume = self.builder.get_insert_block().expect("instruction block");
        let entry = self.context.append_basic_block(thunk, "entry");
        self.builder.position_at_end(entry);

        let capture_arg = thunk
            .get_first_param()
            .expect("capture argument")
            .into_pointer_value();

        // Reserve and root the result box before invoking user code. This
        // avoids a GC gap after the callee has returned an aggregate whose
        // own shadow-stack frame has already been popped.
        let result_box = if let Some(result_type) = self.llvm_type(returns) {
            let box_type = task_result_box_type(self.context, self.module, returns);
            let paths = gc_reference_paths_of(self.module, returns);
            let offsets: Vec<_> = paths
                .iter()
                .map(|path| {
                    let mut full_path = vec![OBJECT_HEADER_FIELDS];
                    full_path.extend(path);
                    const_field_offset(self.context, box_type, &full_path)
                })
                .collect();
            let word = self.context.i64_type();
            let mut descriptor_fields: Vec<BasicValueEnum> = vec![
                self.context
                    .ptr_type(AddressSpace::default())
                    .const_null()
                    .into(),
                word.const_zero().into(), // no ancestors
                word.const_zero().into(), // no contracts
                word.const_int(offsets.len() as u64, false).into(),
            ];
            descriptor_fields.extend(offsets.iter().map(|offset| BasicValueEnum::from(*offset)));
            let descriptor = self.context.const_struct(&descriptor_fields, false);
            let descriptor_global = self.llvm.add_global(
                descriptor.get_type(),
                None,
                &format!("zk.task.result.descriptor.{site}"),
            );
            descriptor_global.set_initializer(&descriptor);
            descriptor_global.set_constant(true);
            descriptor_global.set_linkage(Linkage::Private);

            let ptr = self.context.ptr_type(AddressSpace::default());
            let root_slot = self
                .builder
                .build_alloca(ptr, "task_result_root")
                .expect("result root slot");
            self.builder
                .build_store(root_slot, ptr.const_null())
                .expect("zero result root");
            let root_array_ty = ptr.array_type(1);
            let root_array = self
                .builder
                .build_alloca(root_array_ty, "task_result_roots")
                .expect("result root array");
            let root_entry = unsafe {
                self.builder.build_gep(
                    root_array_ty,
                    root_array,
                    &[
                        self.context.i32_type().const_zero(),
                        self.context.i32_type().const_zero(),
                    ],
                    "task_result_root_entry",
                )
            }
            .expect("result root entry");
            self.builder
                .build_store(root_entry, root_slot)
                .expect("store result root address");
            self.builder
                .build_call(
                    self.runtime.push_frame,
                    &[root_array.into(), word.const_int(1, false).into()],
                    "",
                )
                .expect("push task result root");

            let size = box_type.size_of().expect("sized task result box");
            let align = box_type.get_alignment();
            let box_pointer = self
                .builder
                .build_call(
                    self.runtime.alloc,
                    &[size.into(), align.into()],
                    "task_result_box",
                )
                .expect("allocate task result box")
                .try_as_basic_value()
                .basic()
                .expect("allocator returns pointer")
                .into_pointer_value();
            self.builder
                .build_store(box_pointer, descriptor_global.as_pointer_value())
                .expect("store result descriptor");
            self.builder
                .build_store(root_slot, box_pointer)
                .expect("root result box");
            Some((result_type, box_type, root_slot, box_pointer))
        } else {
            None
        };
        let mut args: Vec<BasicMetadataValueEnum> = Vec::new();
        for (index, ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let slot = unsafe {
                self.builder
                    .build_gep(self.context.i8_type(), capture_arg, &[offset], "capture")
            }
            .expect("branch capture gep");
            args.push(
                self.builder
                    .build_load(self.llvm_type(*ty).expect("capture type"), slot, "capture")
                    .expect("branch capture load")
                    .into(),
            );
        }

        let call = self
            .builder
            .build_call(self.functions[target], &args, "branch_body")
            .expect("branch body call");
        let value = call.try_as_basic_value().basic();

        // A branch that threw returns normally with the pending-exception slot
        // set; hand it to the scope as a branch failure so siblings are
        // cancelled (`concurrent-scopes`, "Structured failure").
        let pending = self
            .builder
            .build_call(self.runtime.has_pending_exception, &[], "branch_pending")
            .expect("has_pending_exception")
            .try_as_basic_value()
            .basic()
            .expect("has_pending_exception returns a bool")
            .into_int_value();
        let failed = self.context.append_basic_block(thunk, "failed");
        let ok = self.context.append_basic_block(thunk, "ok");
        self.builder
            .build_conditional_branch(pending, failed, ok)
            .expect("branch pending check");

        self.builder.position_at_end(failed);
        let exception = self
            .builder
            .build_call(self.runtime.take_pending_exception, &[], "branch_exc")
            .expect("take_pending_exception")
            .try_as_basic_value()
            .basic()
            .expect("take_pending_exception returns a pointer");
        if result_box.is_some() {
            self.builder
                .build_call(self.runtime.pop_frame, &[], "")
                .expect("pop result root on failure");
        }
        self.builder
            .build_call(self.runtime.branch_fail, &[exception.into()], "branch_fail")
            .expect("branch_fail");
        self.builder
            .build_unreachable()
            .expect("branch_fail diverges");

        self.builder.position_at_end(ok);
        let packed = if let Some((_, box_type, _, box_pointer)) = result_box {
            let payload = self
                .builder
                .build_struct_gep(
                    box_type,
                    box_pointer,
                    OBJECT_HEADER_FIELDS,
                    "task_result_payload",
                )
                .expect("result payload field");
            self.builder
                .build_store(payload, value.expect("non-void task result"))
                .expect("store task result payload");
            self.builder
                .build_call(self.runtime.pop_frame, &[], "")
                .expect("pop result root");
            self.builder
                .build_ptr_to_int(box_pointer, self.context.i64_type(), "task_result_word")
                .expect("box pointer to word")
        } else {
            self.context.i64_type().const_zero()
        };
        self.builder
            .build_return(Some(&packed))
            .expect("branch thunk return");

        self.builder.position_at_end(resume);
        thunk
    }

    /// Builds the per-site `extern "C" fn(ptr, i64) -> i64` thunk a
    /// `ParallelForStart` hands to `zirk_rt_parallel_for`
    /// (`parallel-cpu-regions` task 6.1): like [`Self::build_branch_thunk`],
    /// it reloads the captured values from the boxed capture block — the
    /// last of which is the loop's own collection
    /// (`Self::try_lower_parallel_for`'s synthetic capture) — then calls the
    /// lifted per-chunk function with those captures plus this call's own
    /// index (the chunk's element is recovered *inside* that function, via
    /// an ordinary `ArrayListLoad`, not here).
    ///
    /// Unlike a branch thunk, there is no scope to fail: the auto-parallel
    /// lowering only applies to a loop body with no `throw` of its own
    /// (`Self::block_has_loop_incompatible_control_flow`), so a pending
    /// exception here can only come from a *called* function's own throw —
    /// a documented residual gap (`InstKind::ParallelForStart`'s own doc).
    /// It is still taken unconditionally before returning, so it cannot leak
    /// into whatever this pool thread runs next.
    fn build_parallel_for_thunk(
        &self,
        target: &str,
        layout_id: u32,
        site: &str,
    ) -> FunctionValue<'ctx> {
        let layout = &self.module.closures[layout_id as usize];
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();

        let thunk = self.llvm.add_function(
            &format!("zk.parallel_for_thunk.{site}"),
            i64_type.fn_type(&[ptr_type.into(), i64_type.into()], false),
            Some(Linkage::Private),
        );
        thunk.set_call_conventions(0);
        set_uwtable(self.context, thunk);

        let resume = self.builder.get_insert_block().expect("instruction block");
        let entry = self.context.append_basic_block(thunk, "entry");
        self.builder.position_at_end(entry);

        let capture_arg = thunk
            .get_nth_param(0)
            .expect("capture argument")
            .into_pointer_value();
        let index_arg = thunk.get_nth_param(1).expect("index argument");

        let mut args: Vec<BasicMetadataValueEnum> = Vec::new();
        for (index, ty) in layout.captures.iter().enumerate() {
            let offset = self.closure_capture_offset(layout, index);
            let slot = unsafe {
                self.builder
                    .build_gep(self.context.i8_type(), capture_arg, &[offset], "capture")
            }
            .expect("parallel_for capture gep");
            args.push(
                self.builder
                    .build_load(self.llvm_type(*ty).expect("capture type"), slot, "capture")
                    .expect("parallel_for capture load")
                    .into(),
            );
        }
        args.push(index_arg.into());

        self.builder
            .build_call(self.functions[target], &args, "parallel_for_body")
            .expect("parallel_for body call");

        self.builder
            .build_call(
                self.runtime.take_pending_exception,
                &[],
                "parallel_for_drop_exc",
            )
            .expect("take_pending_exception");

        let zero = i64_type.const_zero();
        self.builder
            .build_return(Some(&zero))
            .expect("parallel_for thunk return");

        self.builder.position_at_end(resume);
        thunk
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
            ir::IrType::Enum(id) | ir::IrType::Union(id) => {
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
            ir::IrType::Callable(_) => {
                let struct_type = callable_struct(self.context);
                let next = self
                    .builder
                    .build_struct_gep(struct_type, ptr, index, "gc_callable_step")
                    .expect("gc callable step");
                assert!(
                    rest.is_empty(),
                    "a boxed Callable only exposes its capture pointer to the shadow stack"
                );
                next
            }
            _ => unreachable!("a gc path only steps through Value/Enum/Closure/Nullable/Callable"),
        }
    }

    /// The address of one field inside an object or value struct.
    fn field_pointer(&self, object: ir::Operand, index: u32) -> PointerValue<'ctx> {
        let object_ty = self.value_types[&object.0];
        let ptr = self.operand(object).into_pointer_value();

        match object_ty {
            ir::IrType::Object(id) => {
                let layout = &self.module.objects[id as usize];
                let struct_type = object_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );

                self.builder
                    .build_struct_gep(struct_type, ptr, index + OBJECT_HEADER_FIELDS, "field_ptr")
                    .expect("a verified module addresses a field the layout has")
            }
            ir::IrType::Pointer(pointer_id) => self.field_pointer_of_type(
                ptr,
                self.module.pointer_types[pointer_id as usize],
                index,
                "field_ptr",
                true,
            ),
            ir::IrType::Pin(pin_id) => self.field_pointer_of_type(
                ptr,
                self.module.pin_types[pin_id as usize],
                index,
                "field_ptr",
                false,
            ),
            other => unreachable!(
                "field_pointer called on {}, not an object or pointer",
                other.as_str()
            ),
        }
    }

    /// Helper for `field_pointer` when the layout is reached through an
    /// indirection (`Pointer<T>` or `Pin<T>`): it is the same object/value
    /// layout lookup, but the pointer is already the one to build a GEP on.
    fn field_pointer_of_type(
        &self,
        ptr: PointerValue<'ctx>,
        referent: ir::IrType,
        index: u32,
        name: &str,
        load_object_reference: bool,
    ) -> PointerValue<'ctx> {
        match referent {
            ir::IrType::Object(id) => {
                let layout = &self.module.objects[id as usize];
                let struct_type = object_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                // `Pointer.from(object.field)` starts with the address of
                // the local slot containing the managed object reference.
                // A Pin already carries the pinned object address directly.
                let object_value = if load_object_reference {
                    let object_pointer_type = self
                        .llvm_type(referent)
                        .expect("managed object references have a pointer LLVM type");
                    self.builder
                        .build_load(object_pointer_type, ptr, "pointer_object")
                        .expect("load the managed object reference")
                        .into_pointer_value()
                } else {
                    ptr
                };
                self.builder
                    .build_struct_gep(
                        struct_type,
                        object_value,
                        index + OBJECT_HEADER_FIELDS,
                        name,
                    )
                    .expect("a verified module addresses a field the layout has")
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
                self.builder
                    .build_struct_gep(struct_type, ptr, index, name)
                    .expect("a verified module addresses a field the layout has")
            }
            other => unreachable!(
                "field_pointer reached through pointer/pin refers to {}, not an object or value",
                other.as_str()
            ),
        }
    }

    fn borrowed_value_field_pointer(&self, view: ir::Operand, path: &[u32]) -> PointerValue<'ctx> {
        let ir::IrType::BorrowedView { mutex_type, .. } = self.value_types[&view.0] else {
            unreachable!("verified field projection requires BorrowedView")
        };
        self.payload_path_pointer(
            self.operand(view).into_pointer_value(),
            self.module.mutex_types[mutex_type as usize],
            path,
        )
    }

    /// Applies a statically verified field path to inline aggregate storage.
    fn payload_path_pointer(
        &self,
        mut pointer: PointerValue<'ctx>,
        mut ty: ir::IrType,
        path: &[u32],
    ) -> PointerValue<'ctx> {
        for &index in path {
            let (struct_ty, field_ty, llvm_index) = match ty {
                ir::IrType::Value(id) => {
                    let layout = &self.module.values[id as usize];
                    (
                        value_struct(
                            self.context,
                            layout,
                            &self.module.closures,
                            &self.module.values,
                            &self.module.enums,
                        ),
                        layout.fields[index as usize].ty,
                        index,
                    )
                }
                ir::IrType::Enum(id) | ir::IrType::Union(id) => {
                    let layout = &self.module.enums[id as usize];
                    let field = index
                        .checked_sub(ENUM_HEADER_FIELDS)
                        .expect("verified enum GC path includes header");
                    (
                        enum_struct(
                            self.context,
                            layout,
                            &self.module.closures,
                            &self.module.values,
                            &self.module.enums,
                        ),
                        layout.fields[field as usize].ty,
                        index,
                    )
                }
                ir::IrType::Closure(id) => {
                    let layout = &self.module.closures[id as usize];
                    let field = index
                        .checked_sub(CLOSURE_HEADER_FIELDS)
                        .expect("verified closure GC path includes header");
                    (
                        closure_struct(
                            self.context,
                            layout,
                            &self.module.closures,
                            &self.module.values,
                            &self.module.enums,
                        ),
                        layout.captures[field as usize],
                        index,
                    )
                }
                ir::IrType::Nullable(nullable) => {
                    let inner = nullable.inner();
                    let field_index = index
                        .checked_sub(NULLABLE_HEADER_FIELDS)
                        .expect("verified nullable GC path includes header");
                    (
                        self.llvm_type(ty)
                            .expect("nullable payload type")
                            .into_struct_type(),
                        inner,
                        field_index,
                    )
                }
                ir::IrType::Callable(_) => {
                    let field = index;
                    (
                        callable_struct(self.context),
                        ir::IrType::Pointer(self.module.pointer_types.len() as u32),
                        field,
                    )
                }
                _ => unreachable!("verified GC path only descends into inline aggregate storage"),
            };
            pointer = self
                .builder
                .build_struct_gep(struct_ty, pointer, llvm_index, "mutex.payload.path")
                .expect("verified payload path");
            ty = field_ty;
        }
        pointer
    }

    /// The signature of one contract method, for one concrete call.
    ///
    /// Implementations of a generic contract (`Container<T>`) share their
    /// method index but not their function type — `Container<Int32>.get()`
    /// returns `Int32` while `Container<String>.get()` returns `String`.  The
    /// call site knows the concrete return and argument types, so we find an
    /// implementation whose function type matches them.
    fn contract_signature(
        &self,
        contract: u32,
        index: u32,
        result: ir::IrType,
        args: &[ir::IrType],
    ) -> inkwell::types::FunctionType<'ctx> {
        let expected_return = self.llvm_type(result);
        let expected_args: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = args
            .iter()
            .map(|&ty| {
                self.llvm_type(ty)
                    .expect("a contract method argument has a value type")
                    .into()
            })
            .collect();

        let mut first = None;
        for layout in &self.module.objects {
            let Some(table) = layout.contracts.iter().find(|t| t.contract == contract) else {
                continue;
            };
            let Some(symbol) = table.methods.get(index as usize) else {
                continue;
            };
            let Some(function) = self.functions.get(symbol) else {
                continue;
            };
            let ft = function.get_type();
            if first.is_none() {
                first = Some(ft);
            }

            let return_match = ft.get_return_type() == expected_return;
            let param_types = ft.get_param_types();
            let params_match = param_types.len() == expected_args.len() + 1
                && param_types[1..]
                    .iter()
                    .zip(&expected_args)
                    .all(|(a, b)| a == b);
            if return_match && params_match {
                return ft;
            }
        }

        if let Some(function_type) = first {
            return function_type;
        }

        // A function may be compiled against a contract even when this module
        // contains no concrete implementor (for example, a library function
        // whose caller supplies the value). The call-site IR already carries
        // the substituted argument and result types, so they are sufficient
        // to declare the indirect-call ABI. Runtime dispatch still uses the
        // receiver's descriptor and contract table.
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(expected_args.len() + 1);
        params.push(pointer_type.into());
        params.extend(expected_args);
        match expected_return {
            Some(return_type) => return_type.fn_type(&params, false),
            None => self.context.void_type().fn_type(&params, false),
        }
    }

    /// The layout an object operand belongs to.
    fn object_layout_of(&self, operand: ir::Operand) -> u32 {
        let ir::IrType::Object(id) = self.value_types[&operand.0] else {
            unreachable!("a verified field access reads an object")
        };
        id
    }

    fn emit_instruction(&mut self, instruction: &ir::Instruction) {
        let requester = self
            .module
            .capability_requesters
            .get(&instruction.span)
            .cloned();
        if let Some(requester) = &requester {
            let global = self
                .builder
                .build_global_string_ptr(requester, "zk.permission.requester")
                .expect("create protected call-site requester string");
            self.builder
                .build_call(
                    self.runtime.permission_requester_set,
                    &[
                        global.as_pointer_value().into(),
                        self.context
                            .i64_type()
                            .const_int(requester.len() as u64, false)
                            .into(),
                    ],
                    "",
                )
                .expect("set protected call-site requester");
        }
        let value: Option<BasicValueEnum> = match &instruction.kind {
            ir::InstKind::BorrowViewLoad { view } => {
                let ty = self.llvm_type(instruction.ty).expect("borrow payload type");
                Some(
                    self.builder
                        .build_load(
                            ty,
                            self.operand(*view).into_pointer_value(),
                            "mutex.payload.load",
                        )
                        .expect("load borrowed mutex payload"),
                )
            }
            ir::InstKind::BorrowViewStore { view, value } => {
                self.builder
                    .build_store(
                        self.operand(*view).into_pointer_value(),
                        self.operand(*value),
                    )
                    .expect("store borrowed mutex payload");
                None
            }
            ir::InstKind::BorrowViewFieldLoad { view, path } => {
                let pointer = self.borrowed_value_field_pointer(*view, path);
                let ty = self.llvm_type(instruction.ty).expect("borrowed field type");
                Some(
                    self.builder
                        .build_load(ty, pointer, "mutex.field.load")
                        .expect("load borrowed field"),
                )
            }
            ir::InstKind::BorrowViewFieldStore { view, path, value } => {
                let pointer = self.borrowed_value_field_pointer(*view, path);
                self.builder
                    .build_store(pointer, self.operand(*value))
                    .expect("store borrowed field");
                None
            }
            ir::InstKind::MutexWithCall {
                mutex,
                callback,
                layout,
                result_slot,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let capture_ptr = self
                    .builder
                    .build_extract_value(
                        callable,
                        CALLABLE_CAPTURE_FIELD,
                        "mutex.capture.root.value",
                    )
                    .expect("extract callable capture pointer")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "mutex.capture.root")
                    .expect("allocate temporary capture root");
                self.builder
                    .build_store(capture_root, capture_ptr)
                    .expect("store capture root");
                let mutex_root = self
                    .builder
                    .build_alloca(ptr, "mutex.handle.root")
                    .expect("allocate temporary mutex root");
                self.builder
                    .build_store(mutex_root, self.operand(*mutex))
                    .expect("store mutex handle root");
                let roots_ty = ptr.array_type(2);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "mutex.roots")
                    .expect("allocate temporary root list");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "mutex.root.entry",
                    )
                }
                .expect("root entry");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register capture root address");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_int(1, false),
                        ],
                        "mutex.handle.root.entry",
                    )
                }
                .expect("mutex root entry");
                self.builder
                    .build_store(root_entry, mutex_root)
                    .expect("register mutex root address");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(2, false).into(),
                        ],
                        "mutex.push.frame",
                    )
                    .expect("push mutex adapter roots");

                let result_ptr = result_slot
                    .map(|slot| self.slots[&slot])
                    .unwrap_or_else(|| ptr.const_null());
                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into(), ptr.into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "mutex.callback.context")
                    .expect("allocate mutex callback context");
                let mut context_value = self
                    .builder
                    .build_insert_value(
                        context_ty.get_undef(),
                        callable,
                        0,
                        "mutex.context.callable",
                    )
                    .expect("insert callback callable")
                    .into_struct_value();
                context_value = self
                    .builder
                    .build_insert_value(context_value, result_ptr, 1, "mutex.context.result")
                    .expect("insert result slot")
                    .into_struct_value();
                self.builder
                    .build_store(context, context_value)
                    .expect("store callback context");
                let thunk = self.mutex_with_thunk(*layout);
                let call = self
                    .builder
                    .build_call(
                        self.runtime.gc_mutex_with,
                        &[
                            self.operand(*mutex).into(),
                            thunk.as_global_value().as_pointer_value().into(),
                            context.into(),
                        ],
                        "mutex.with.status",
                    )
                    .expect("call runtime mutex.with ABI");
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "mutex.pop.frame")
                    .expect("pop mutex adapter roots");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("mutex.with returns status"),
                )
            }
            ir::InstKind::ProjectGlobalInit { global, value_slot } => {
                Some(self.emit_project_global_init(*global, *value_slot).into())
            }
            ir::InstKind::ProjectGlobalGet { global, out } => {
                Some(self.emit_project_global_get(*global, *out).into())
            }
            ir::InstKind::ProjectGlobalSet {
                global,
                value,
                event,
            } => Some(self.emit_project_global_set(*global, *value, *event).into()),
            ir::InstKind::ProjectGlobalWatchSubscribe {
                global,
                channel,
                snapshot_out,
                token_out,
                revision_out,
            } => Some(
                self.emit_project_global_watch_subscribe(
                    *global,
                    *channel,
                    *snapshot_out,
                    *token_out,
                    *revision_out,
                )
                .into(),
            ),
            ir::InstKind::ProjectGlobalWatchNext { token, out } => {
                Some(self.emit_project_global_watch_next(*token, *out).into())
            }
            ir::InstKind::ProjectGlobalWatchClose { token } => {
                self.emit_project_global_watch_close(*token);
                None
            }
            ir::InstKind::MutexNew {
                payload,
                mutex_type,
                object_slot,
            } => {
                let payload_ty = self.module.mutex_types[*mutex_type as usize];
                let llvm_ty = self.llvm_type(payload_ty).expect("Mutex payload is sized");
                let staging = self
                    .builder
                    .build_alloca(llvm_ty, "mutex.payload.staging")
                    .expect("allocate mutex payload staging");
                self.builder
                    .build_store(staging, self.operand(*payload))
                    .expect("stage mutex payload");
                let reference_paths = gc_reference_paths_of(self.module, payload_ty);
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let i64_ty = self.context.i64_type();
                let status = self
                    .builder
                    .build_alloca(self.context.i32_type(), "mutex.new.status")
                    .expect("allocate MutexNew status");
                // The destination participates in the function's GC root frame
                // while the allocator may collect. Never expose uninitialized
                // stack contents as a managed pointer; failures retain null.
                self.builder
                    .build_store(self.slots[object_slot], ptr_ty.const_null())
                    .expect("clear Mutex destination root");
                self.builder
                    .build_store(status, self.context.i32_type().const_int(4, false))
                    .expect("initialize MutexNew failure status");
                if reference_paths.is_empty() {
                    let call = self
                        .builder
                        .build_call(
                            self.runtime.gc_mutex_alloc,
                            &[
                                staging.into(),
                                self.byte_size_of(payload_ty).into(),
                                self.byte_align_of(payload_ty).into(),
                                ptr_ty.const_null().into(),
                                i64_ty.const_zero().into(),
                                status.into(),
                            ],
                            "mutex.new.handle",
                        )
                        .expect("allocate GC mutex");
                    let handle = call
                        .try_as_basic_value()
                        .basic()
                        .expect("Mutex allocator returns handle");
                    self.builder
                        .build_store(self.slots[object_slot], handle)
                        .expect("store Mutex handle");
                } else {
                    let roots_ty = ptr_ty.array_type(reference_paths.len() as u32);
                    let roots = self
                        .builder
                        .build_alloca(roots_ty, "mutex.payload.roots")
                        .expect("allocate staging root array");
                    let offsets_ty = i64_ty.array_type(reference_paths.len() as u32);
                    let offsets = self
                        .builder
                        .build_alloca(offsets_ty, "mutex.reference.offsets")
                        .expect("allocate offset array");
                    for (index, path) in reference_paths.iter().enumerate() {
                        let index = index as u32;
                        let root_entry = unsafe {
                            self.builder.build_gep(
                                roots_ty,
                                roots,
                                &[
                                    self.context.i32_type().const_zero(),
                                    self.context.i32_type().const_int(index as u64, false),
                                ],
                                "mutex.payload.root.entry",
                            )
                        }
                        .expect("root entry");
                        let root_address = if path.is_empty() {
                            staging
                        } else {
                            self.payload_path_pointer(staging, payload_ty, path)
                        };
                        self.builder
                            .build_store(root_entry, root_address)
                            .expect("store staged root address");
                        let offset_entry = unsafe {
                            self.builder.build_gep(
                                offsets_ty,
                                offsets,
                                &[
                                    self.context.i32_type().const_zero(),
                                    self.context.i32_type().const_int(index as u64, false),
                                ],
                                "mutex.reference.offset.entry",
                            )
                        }
                        .expect("offset entry");
                        let offset = if path.is_empty() {
                            i64_ty.const_zero()
                        } else {
                            let struct_ty = self
                                .llvm_type(payload_ty)
                                .expect("sized Mutex payload")
                                .into_struct_type();
                            const_field_offset(self.context, struct_ty, path)
                        };
                        self.builder
                            .build_store(offset_entry, offset)
                            .expect("store reference offset");
                    }
                    self.builder
                        .build_call(
                            self.runtime.push_frame,
                            &[
                                roots.into(),
                                i64_ty.const_int(reference_paths.len() as u64, false).into(),
                            ],
                            "mutex.payload.push.frame",
                        )
                        .expect("root staged Mutex payload");
                    let call = self
                        .builder
                        .build_call(
                            self.runtime.gc_mutex_alloc,
                            &[
                                staging.into(),
                                self.byte_size_of(payload_ty).into(),
                                self.byte_align_of(payload_ty).into(),
                                offsets.into(),
                                i64_ty.const_int(reference_paths.len() as u64, false).into(),
                                status.into(),
                            ],
                            "mutex.new.handle",
                        )
                        .expect("allocate GC mutex");
                    let handle = call
                        .try_as_basic_value()
                        .basic()
                        .expect("Mutex allocator returns handle");
                    self.builder
                        .build_call(self.runtime.pop_frame, &[], "mutex.payload.pop.frame")
                        .expect("unroot staged Mutex payload");
                    self.builder
                        .build_store(self.slots[object_slot], handle)
                        .expect("store Mutex handle");
                }
                Some(
                    self.builder
                        .build_load(self.context.i32_type(), status, "mutex.new.status.value")
                        .expect("load MutexNew status"),
                )
            }
            // These verifier markers have no executable lowering yet. The
            // object API rejects them before emission; raw LLVM users must also
            // fail closed instead of treating lock/unlock as no-ops or a
            // borrowed payload as an alias of its mutex handle.
            ir::InstKind::MutexAcquire { .. }
            | ir::InstKind::MutexRelease { .. }
            | ir::InstKind::BorrowViewStart { .. }
            | ir::InstKind::BorrowViewEnd { .. } => {
                let trap = self.llvm.get_function("llvm.trap").unwrap_or_else(|| {
                    self.llvm.add_function(
                        "llvm.trap",
                        self.context.void_type().fn_type(&[], false),
                        None,
                    )
                });
                self.builder
                    .build_call(trap, &[], "mutex.marker.not.lowered")
                    .expect("building the noreturn fail-closed marker call");
                None
            }
            ir::InstKind::AtomicNew {
                kind,
                value,
                object,
            } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.atomic_managed_new,
                        &[
                            self.context
                                .i32_type()
                                .const_int(*kind as u64, false)
                                .into(),
                            self.atomic_pack(*value).into(),
                            self.slots[object].into(),
                        ],
                        "atomic.new.status",
                    )
                    .expect("managed Atomic constructor");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::AtomicLoad { object, order, out } => {
                let temp = self
                    .builder
                    .build_alloca(self.context.i64_type(), "atomic.load.out")
                    .expect("Atomic load output");
                self.builder
                    .build_store(temp, self.context.i64_type().const_zero())
                    .expect("initialize Atomic load output");
                let (function, args) = if let Some(order) = order {
                    (
                        self.runtime.atomic_managed_load_ordered,
                        vec![
                            self.operand(*object).into(),
                            self.operand(*order).into(),
                            temp.into(),
                        ],
                    )
                } else {
                    (
                        self.runtime.atomic_managed_load,
                        vec![self.operand(*object).into(), temp.into()],
                    )
                };
                let call = self
                    .builder
                    .build_call(function, &args, "atomic.load.status")
                    .expect("managed Atomic load");
                let packed = self
                    .builder
                    .build_load(self.context.i64_type(), temp, "atomic.load.value")
                    .expect("load Atomic result")
                    .into_int_value();
                self.atomic_unpack_to_slot(packed, *out);
                call.try_as_basic_value().basic()
            }
            ir::InstKind::AtomicStore {
                object,
                value,
                order,
            } => {
                let args = if let Some(order) = order {
                    vec![
                        self.operand(*object).into(),
                        self.atomic_pack(*value).into(),
                        self.operand(*order).into(),
                    ]
                } else {
                    vec![
                        self.operand(*object).into(),
                        self.atomic_pack(*value).into(),
                    ]
                };
                let function = if order.is_some() {
                    self.runtime.atomic_managed_store_ordered
                } else {
                    self.runtime.atomic_managed_store
                };
                self.builder
                    .build_call(function, &args, "atomic.store.status")
                    .expect("managed Atomic store")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::AtomicExchange {
                object,
                value,
                order,
                out,
            }
            | ir::InstKind::AtomicFetchAdd {
                object,
                value,
                order,
                out,
            }
            | ir::InstKind::AtomicFetchSub {
                object,
                value,
                order,
                out,
            } => {
                let temp = self
                    .builder
                    .build_alloca(self.context.i64_type(), "atomic.rmw.out")
                    .expect("Atomic operation output");
                self.builder
                    .build_store(temp, self.context.i64_type().const_zero())
                    .expect("initialize Atomic output");
                let (plain, ordered) = match &instruction.kind {
                    ir::InstKind::AtomicExchange { .. } => (
                        self.runtime.atomic_managed_exchange,
                        self.runtime.atomic_managed_exchange_ordered,
                    ),
                    ir::InstKind::AtomicFetchAdd { .. } => (
                        self.runtime.atomic_managed_fetch_add,
                        self.runtime.atomic_managed_fetch_add_ordered,
                    ),
                    _ => (
                        self.runtime.atomic_managed_fetch_sub,
                        self.runtime.atomic_managed_fetch_sub_ordered,
                    ),
                };
                let (function, args) = if let Some(order) = order {
                    (
                        ordered,
                        vec![
                            self.operand(*object).into(),
                            self.atomic_pack(*value).into(),
                            self.operand(*order).into(),
                            temp.into(),
                        ],
                    )
                } else {
                    (
                        plain,
                        vec![
                            self.operand(*object).into(),
                            self.atomic_pack(*value).into(),
                            temp.into(),
                        ],
                    )
                };
                let call = self
                    .builder
                    .build_call(function, &args, "atomic.rmw.status")
                    .expect("managed Atomic RMW");
                let packed = self
                    .builder
                    .build_load(self.context.i64_type(), temp, "atomic.rmw.value")
                    .expect("load Atomic RMW result")
                    .into_int_value();
                self.atomic_unpack_to_slot(packed, *out);
                call.try_as_basic_value().basic()
            }
            ir::InstKind::AtomicCompareExchange {
                object,
                current,
                new_value,
                success_order,
                failure_order,
                observed,
                exchanged,
            } => {
                let temp = self
                    .builder
                    .build_alloca(self.context.i64_type(), "atomic.cmpxchg.out")
                    .expect("Atomic compare-exchange output");
                let flag = self
                    .builder
                    .build_alloca(self.context.bool_type(), "atomic.cmpxchg.flag")
                    .expect("Atomic compare-exchange flag");
                self.builder
                    .build_store(temp, self.context.i64_type().const_zero())
                    .expect("initialize compare-exchange output");
                self.builder
                    .build_store(flag, self.context.bool_type().const_zero())
                    .expect("initialize compare-exchange flag");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.atomic_managed_compare_exchange,
                        &[
                            self.operand(*object).into(),
                            self.atomic_pack(*current).into(),
                            self.atomic_pack(*new_value).into(),
                            self.operand(*success_order).into(),
                            self.operand(*failure_order).into(),
                            temp.into(),
                            flag.into(),
                        ],
                        "atomic.cmpxchg.status",
                    )
                    .expect("managed Atomic compare-exchange");
                let packed = self
                    .builder
                    .build_load(self.context.i64_type(), temp, "atomic.cmpxchg.value")
                    .expect("load observed Atomic value")
                    .into_int_value();
                self.atomic_unpack_to_slot(packed, *observed);
                let did_exchange = self
                    .builder
                    .build_load(self.context.bool_type(), flag, "atomic.cmpxchg.exchanged")
                    .expect("load compare-exchange flag");
                self.builder
                    .build_store(self.slots[exchanged], did_exchange)
                    .expect("store compare-exchange flag");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::ConstInt(value) => Some(match instruction.ty {
                ir::IrType::Int(width) => {
                    let int_type = self
                        .context
                        .custom_width_int_type(
                            std::num::NonZeroU32::new(width.bits())
                                .expect("every IntWidth is nonzero"),
                        )
                        .expect("every IntWidth is a valid LLVM integer width");
                    if width.bits() <= 64 {
                        int_type.const_int(*value as u64, true).into()
                    } else {
                        let bits = *value as u128;
                        int_type
                            .const_int_arbitrary_precision(&[bits as u64, (bits >> 64) as u64])
                            .into()
                    }
                }
                _ => unreachable!("ConstInt must have an Int type"),
            }),
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
                specialization,
                args,
            } => {
                let receiver = self.operand(*object).into_pointer_value();
                let ptr = self.context.ptr_type(AddressSpace::default());
                let id = self
                    .context
                    .i64_type()
                    .const_int(u64::from(*contract), false);
                let open = self
                    .builder
                    .build_call(
                        self.runtime.is_open_object,
                        &[receiver.into()],
                        "is_open_object",
                    )
                    .expect("check whether the receiver is an open object")
                    .try_as_basic_value()
                    .basic()
                    .expect("open-object check returns a status")
                    .into_int_value();
                let is_open = self
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        open,
                        self.context.i32_type().const_zero(),
                        "dynamic_contract_receiver",
                    )
                    .expect("compare open-object status");
                let parent = self.builder.get_insert_block().expect("call block");
                let function = parent.get_parent().expect("call function");
                let dynamic_block = self
                    .context
                    .append_basic_block(function, "contract.dynamic");
                let class_block = self.context.append_basic_block(function, "contract.class");
                let merge_block = self.context.append_basic_block(function, "contract.merge");
                self.builder
                    .build_conditional_branch(is_open, dynamic_block, class_block)
                    .expect("branch on receiver representation");

                let result_ty = instruction.ty;
                let arg_tys: Vec<ir::IrType> =
                    args.iter().map(|a| self.value_types[&a.0]).collect();
                let call_signature =
                    self.contract_signature(*contract, *index, result_ty, &arg_tys);

                self.builder.position_at_end(class_block);
                let descriptor = self
                    .builder
                    .build_load(ptr, receiver, "descriptor")
                    .expect("load class descriptor")
                    .into_pointer_value();
                let table = self
                    .builder
                    .build_call(
                        self.runtime.contract_table,
                        &[descriptor.into(), id.into()],
                        "itable",
                    )
                    .expect("find class contract table")
                    .try_as_basic_value()
                    .basic()
                    .expect("contract table lookup returns a pointer")
                    .into_pointer_value();
                let dispatch_index = specialization.map_or(*index, |specialization| {
                    let base_count = self
                        .module
                        .contracts
                        .get(*contract as usize)
                        .map_or(0, |layout| layout.methods.len())
                        as u32;
                    base_count + specialization
                });
                let offset = self
                    .context
                    .i32_type()
                    .const_int(u64::from(dispatch_index), false);
                let slot = unsafe {
                    self.builder
                        .build_in_bounds_gep(ptr, table, &[offset], "method_slot")
                        .expect("verified contract method slot")
                };
                let target = self
                    .builder
                    .build_load(ptr, slot, "method")
                    .expect("load contract method")
                    .into_pointer_value();
                let mut class_arguments: Vec<BasicMetadataValueEnum> = vec![receiver.into()];
                class_arguments.extend(
                    args.iter()
                        .map(|a| BasicMetadataValueEnum::from(self.operand(*a))),
                );
                let class_call = self
                    .builder
                    .build_indirect_call(
                        call_signature,
                        target,
                        &class_arguments,
                        "class_contract_call",
                    )
                    .expect("call through class contract table");
                let class_value = class_call.try_as_basic_value().basic();
                let class_end = self.builder.get_insert_block().expect("class call block");
                self.builder
                    .build_unconditional_branch(merge_block)
                    .expect("join contract dispatch");

                self.builder.position_at_end(dynamic_block);
                let method_name = self
                    .module
                    .contracts
                    .get(*contract as usize)
                    .and_then(|layout| layout.methods.get(*index as usize))
                    .expect("verified contract method has a runtime name");
                let key_bytes = self.context.const_string(method_name.as_bytes(), false);
                let key_global = self.llvm.add_global(
                    key_bytes.get_type(),
                    None,
                    &format!(
                        "zk.contract.{}.{}.dynamic-key",
                        instruction.result.map_or(0, |v| v.0),
                        index
                    ),
                );
                key_global.set_initializer(&key_bytes);
                key_global.set_constant(true);
                key_global.set_linkage(Linkage::Private);
                let property_out = self
                    .builder
                    .build_alloca(ptr, "contract_callable_box_out")
                    .expect("callable property output slot");
                self.builder
                    .build_store(property_out, ptr.const_null())
                    .expect("initialize callable output");
                self.builder
                    .build_call(
                        self.runtime.object_get_bytes,
                        &[
                            receiver.into(),
                            key_global.as_pointer_value().into(),
                            self.context
                                .i64_type()
                                .const_int(method_name.len() as u64, false)
                                .into(),
                            property_out.into(),
                        ],
                        "contract_callable_property_status",
                    )
                    .expect("read callable contract property");
                let box_ptr = self
                    .builder
                    .build_load(ptr, property_out, "contract_callable_box")
                    .expect("load callable property box")
                    .into_pointer_value();
                let callable_id = self
                    .module
                    .closures
                    .iter()
                    .position(|layout| layout.params == arg_tys && layout.returns == result_ty);
                let dynamic_value = if let Some(callable_id) = callable_id {
                    let callable_id = callable_id as u32;
                    let callable_layout = &self.module.closures[callable_id as usize];
                    let callable_ty = callable_struct(self.context);
                    let callable_out = self
                        .builder
                        .build_alloca(callable_ty, "contract_callable_out")
                        .expect("allocate callable payload");
                    let unpack_status = self
                        .builder
                        .build_call(
                            self.runtime.dynamic_value_copy_to,
                            &[
                                box_ptr.into(),
                                self.context
                                    .i32_type()
                                    .const_int(
                                        u64::from(
                                            ir::IrType::Callable(callable_id)
                                                .dynamic_tag()
                                                .unwrap(),
                                        ),
                                        false,
                                    )
                                    .into(),
                                callable_out.into(),
                                callable_ty.size_of().expect("callable payload size").into(),
                            ],
                            "unpack_contract_callable",
                        )
                        .expect("unpack tagged callable property")
                        .try_as_basic_value()
                        .basic()
                        .expect("callable unpack returns status")
                        .into_int_value();
                    let unpacked = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            unpack_status,
                            self.context.i32_type().const_zero(),
                            "contract_callable_layout_matches",
                        )
                        .expect("compare callable layout validation");
                    let callable_ready = self
                        .context
                        .append_basic_block(function, "contract.callable.ready");
                    let callable_invalid = self
                        .context
                        .append_basic_block(function, "contract.callable.invalid");
                    self.builder
                        .build_conditional_branch(unpacked, callable_ready, callable_invalid)
                        .expect("reject a callable whose signature changed after conformance");
                    self.builder.position_at_end(callable_invalid);
                    self.builder
                        .build_call(
                            self.runtime.missing_contract,
                            &[],
                            "invalid_contract_callable",
                        )
                        .expect("invalid callable contract failure helper");
                    self.builder
                        .build_unreachable()
                        .expect("invalid callable dispatch is unreachable");
                    self.builder.position_at_end(callable_ready);
                    let callable = self
                        .builder
                        .build_load(callable_ty, callable_out, "contract_callable")
                        .expect("load callable payload")
                        .into_struct_value();
                    let dispatch = self
                        .builder
                        .build_extract_value(callable, CALLABLE_DISPATCH_FIELD, "contract_dispatch")
                        .expect("callable dispatch entry")
                        .into_pointer_value();
                    let captures = self
                        .builder
                        .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "contract_captures")
                        .expect("callable captures")
                        .into_pointer_value();
                    let mut dynamic_arguments: Vec<BasicMetadataValueEnum> = vec![captures.into()];
                    dynamic_arguments.extend(
                        args.iter()
                            .map(|a| BasicMetadataValueEnum::from(self.operand(*a))),
                    );
                    let dispatch_sig = callable_dispatch_signature(
                        self.context,
                        callable_layout,
                        &self.module.closures,
                        &self.module.values,
                        &self.module.enums,
                    );
                    let dynamic_call = self
                        .builder
                        .build_indirect_call(
                            dispatch_sig,
                            dispatch,
                            &dynamic_arguments,
                            "dynamic_contract_call",
                        )
                        .expect("invoke callable property");
                    let dynamic_value = dynamic_call.try_as_basic_value().basic();
                    let dynamic_end = self.builder.get_insert_block().expect("dynamic call block");
                    self.builder
                        .build_unconditional_branch(merge_block)
                        .expect("join contract dispatch");
                    dynamic_value.map(|value| (value, dynamic_end))
                } else {
                    // A contract method need not be representable as a
                    // first-class callable in this module. Such a module can
                    // still dispatch class implementors; an open object that
                    // reaches this branch is a failed runtime conformance.
                    self.builder
                        .build_call(
                            self.runtime.missing_contract,
                            &[],
                            "missing_dynamic_contract_callable",
                        )
                        .expect("missing-contract failure helper");
                    self.builder
                        .build_unreachable()
                        .expect("invalid dynamic contract dispatch is unreachable");
                    None
                };

                self.builder.position_at_end(merge_block);
                match (class_value, dynamic_value) {
                    (Some(class_value), Some((dynamic_value, dynamic_end))) => {
                        let phi = self
                            .builder
                            .build_phi(class_value.get_type(), "contract_result")
                            .expect("merge dynamic and class contract results");
                        phi.add_incoming(&[
                            (&class_value, class_end),
                            (&dynamic_value, dynamic_end),
                        ]);
                        Some(phi.as_basic_value())
                    }
                    (Some(class_value), None) => {
                        let phi = self
                            .builder
                            .build_phi(class_value.get_type(), "contract_result")
                            .expect("merge class contract result with unreachable dynamic path");
                        phi.add_incoming(&[(&class_value, class_end)]);
                        Some(phi.as_basic_value())
                    }
                    (None, None) => None,
                    _ => panic!("class and dynamic contract calls disagree on return type"),
                }
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

            // `word as <key/element type>` (`universal-to-string-and-multiline-strings`):
            // the inverse of the `sext`/`ptr_to_int` a `MapSet`/`SetAdd`
            // applies — the same conversion `MapGet` performs inline for
            // values. The word is always an `i64` coming in.
            ir::InstKind::WordCast(operand) => {
                let word = self.operand(*operand).into_int_value();
                let converted: BasicValueEnum<'ctx> =
                    match self.llvm_type(instruction.ty).expect("word cast target") {
                        BasicTypeEnum::PointerType(ptr_ty) => self
                            .builder
                            .build_int_to_ptr(word, ptr_ty, "word_ptr")
                            .expect("word to pointer")
                            .into(),
                        BasicTypeEnum::IntType(int_ty) => {
                            if int_ty.get_bit_width() == 64 {
                                word.into()
                            } else {
                                self.builder
                                    .build_int_truncate(word, int_ty, "word")
                                    .expect("word truncation")
                                    .into()
                            }
                        }
                        other => panic!("WordCast target cannot be {other}"),
                    };
                Some(converted)
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
                let ir::IrType::Float(source_width) = self.value_types[&operand.0] else {
                    unreachable!("FloatCast always converts from a Float source")
                };

                // `Float128` never takes LLVM's native `fpext`/`fptrunc`
                // path to or from another width: both depend on the
                // target's quad-precision libcalls, unreliable on
                // Windows/MSVC (`emit_f128_to_f64`'s doc has the specifics).
                // Every other pair is a plain hardware-backed conversion,
                // same as before.
                let converted = if source_width.bits() == target_width.bits() {
                    value
                } else if source_width == ir::FloatWidth::F128 {
                    let as_f64 = self.emit_f128_to_f64(value);
                    if target_width == ir::FloatWidth::F64 {
                        as_f64
                    } else {
                        self.builder
                            .build_float_trunc(as_f64, target_ty, "fptrunc")
                            .expect("float truncation")
                    }
                } else if target_width == ir::FloatWidth::F128 {
                    let as_f64 = if source_width == ir::FloatWidth::F64 {
                        value
                    } else {
                        self.builder
                            .build_float_ext(value, self.context.f64_type(), "fpext")
                            .expect("float extension")
                    };
                    self.emit_f64_to_f128(as_f64)
                } else if source_width.bits() < target_width.bits() {
                    self.builder
                        .build_float_ext(value, target_ty, "fpext")
                        .expect("float extension")
                } else {
                    self.builder
                        .build_float_trunc(value, target_ty, "fptrunc")
                        .expect("float truncation")
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
                let converted = if target_width == ir::FloatWidth::F128 {
                    let value_bits = value.get_type().get_bit_width();
                    let value_i128 = if value_bits == 128 {
                        value
                    } else if source_signed {
                        self.builder
                            .build_int_s_extend(value, self.context.i128_type(), "i128.sext")
                            .expect("sign extend integer for binary128 conversion")
                    } else {
                        self.builder
                            .build_int_z_extend(value, self.context.i128_type(), "i128.zext")
                            .expect("zero extend integer for binary128 conversion")
                    };
                    let input = self
                        .builder
                        .build_alloca(self.context.i128_type(), "f128.from_int.input")
                        .expect("allocate integer conversion input");
                    let output = self
                        .builder
                        .build_alloca(self.context.i128_type(), "f128.from_int.output")
                        .expect("allocate binary128 conversion result");
                    self.builder
                        .build_store(input, value_i128)
                        .expect("store integer conversion input");
                    self.builder
                        .build_call(
                            self.runtime.float128_from_integer,
                            &[
                                output.into(),
                                input.into(),
                                self.context
                                    .i32_type()
                                    .const_int(source_signed as u64, false)
                                    .into(),
                            ],
                            "f128.from_int",
                        )
                        .expect("call portable integer-to-Float128 helper");
                    let bits = self
                        .builder
                        .build_load(self.context.i128_type(), output, "f128.from_int.bits")
                        .expect("load binary128 conversion result");
                    self.builder
                        .build_bit_cast(bits, target_ty, "f128.from_int.value")
                        .expect("restore binary128 integer conversion")
                        .into_float_value()
                } else if source_signed {
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
            // Phase 3b). LLVM's `fptosi`/`fptoui` are only defined for finite,
            // in-range inputs. Check the truncated value against an exact,
            // power-of-two half-open range first; ordered comparisons reject
            // NaN and infinities as well. This matches the interpreter's
            // truncate-then-range-check behavior without exposing LLVM UB.
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
                let converted = if self.value_types[&operand.0]
                    == ir::IrType::Float(ir::FloatWidth::F128)
                {
                    let input = self.float128_bits_slot(value, "f128.to_int.input");
                    let output = self
                        .builder
                        .build_alloca(self.context.i128_type(), "f128.to_int.output")
                        .expect("allocate checked Float128 conversion result");
                    let status = self
                        .builder
                        .build_call(
                            self.runtime.float128_to_integer,
                            &[
                                input.into(),
                                output.into(),
                                self.context
                                    .i32_type()
                                    .const_int(target_width.bits() as u64, false)
                                    .into(),
                                self.context
                                    .i32_type()
                                    .const_int(target_width.signed() as u64, false)
                                    .into(),
                            ],
                            "f128.to_int.status",
                        )
                        .expect("call checked Float128-to-integer helper")
                        .try_as_basic_value()
                        .basic()
                        .expect("Float128 conversion returns a status")
                        .into_int_value();
                    let valid = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            status,
                            self.context.i32_type().const_zero(),
                            "f128.to_int.valid",
                        )
                        .expect("compare Float128 conversion status");
                    let function = self
                        .builder
                        .get_insert_block()
                        .expect("Float128 conversion emitted inside a block")
                        .get_parent()
                        .expect("Float128 conversion block belongs to a function");
                    let success = self
                        .context
                        .append_basic_block(function, "f128.to_int.success");
                    let failure = self
                        .context
                        .append_basic_block(function, "f128.to_int.failure");
                    self.builder
                        .build_conditional_branch(valid, success, failure)
                        .expect("branch on Float128 conversion status");
                    self.builder.position_at_end(failure);
                    self.builder
                        .build_call(self.runtime.invalid_cast, &[], "f128.to_int.failure")
                        .expect("report invalid Float128-to-integer conversion");
                    self.builder
                        .build_unreachable()
                        .expect("invalid Float128 conversion handler does not return");
                    self.builder.position_at_end(success);
                    let bits = self
                        .builder
                        .build_load(self.context.i128_type(), output, "f128.to_int.bits")
                        .expect("load Float128 conversion result bits")
                        .into_int_value();
                    let converted = if target_width.bits() == 128 {
                        bits
                    } else {
                        self.builder
                            .build_int_truncate(bits, target_ty, "f128.to_int")
                            .expect("truncate checked Float128 result to target width")
                    };
                    converted.into()
                } else {
                    let trunc_intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.trunc")
                        .expect("LLVM provides llvm.trunc for every supported float width");
                    let trunc_function = trunc_intrinsic
                        .get_declaration(self.llvm, &[value.get_type().into()])
                        .expect("llvm.trunc accepts each supported float width");
                    let truncated = self
                        .builder
                        .build_call(trunc_function, &[value.into()], "float_to_int_truncated")
                        .expect("truncate float before checked integer conversion")
                        .try_as_basic_value()
                        .basic()
                        .expect("llvm.trunc returns a float")
                        .into_float_value();
                    let float_ty = value.get_type();
                    let exponent = if target_width.signed() {
                        target_width.bits() - 1
                    } else {
                        target_width.bits()
                    };
                    let upper_integer = if exponent == 128 {
                        "340282366920938463463374607431768211456".to_string()
                    } else {
                        (1_u128 << exponent).to_string()
                    };
                    let upper = unsafe { float_ty.const_float_from_string(&upper_integer) };
                    let lower = if target_width.signed() {
                        unsafe { float_ty.const_float_from_string(&format!("-{upper_integer}")) }
                    } else {
                        float_ty.const_zero()
                    };
                    let at_or_above_lower = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OGE,
                            truncated,
                            lower,
                            "float_to_int_lower_ok",
                        )
                        .expect("compare float conversion lower bound");
                    let below_upper = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::OLT,
                            truncated,
                            upper,
                            "float_to_int_upper_ok",
                        )
                        .expect("compare float conversion upper bound");
                    let in_range = self
                        .builder
                        .build_and(at_or_above_lower, below_upper, "float_to_int_in_range")
                        .expect("combine float conversion range checks");
                    let function = self
                        .builder
                        .get_insert_block()
                        .expect("float conversion is emitted inside a block")
                        .get_parent()
                        .expect("float conversion block belongs to a function");
                    let valid = self
                        .context
                        .append_basic_block(function, "float_to_int_valid");
                    let invalid = self
                        .context
                        .append_basic_block(function, "float_to_int_invalid");
                    self.builder
                        .build_conditional_branch(in_range, valid, invalid)
                        .expect("branch on checked float conversion range");
                    self.builder.position_at_end(invalid);
                    self.builder
                        .build_call(self.runtime.invalid_cast, &[], "invalid_float_to_int")
                        .expect("report out-of-range float-to-integer conversion");
                    self.builder
                        .build_unreachable()
                        .expect("invalid cast handler does not return");
                    self.builder.position_at_end(valid);
                    let converted = if target_width.signed() {
                        self.builder
                            .build_float_to_signed_int(truncated, target_ty, "fptosi")
                            .expect("float to signed int")
                    } else {
                        self.builder
                            .build_float_to_unsigned_int(truncated, target_ty, "fptoui")
                            .expect("float to unsigned int")
                    };
                    converted.into()
                };
                Some(converted)
            }

            // An exact base-ten `Float` literal: the digits are parsed once,
            // here, into the `{ i128, i8 }` aggregate — never through a
            // binary float.
            ir::InstKind::ConstDecimal(text) => {
                let (coef, scale) = parse_decimal_literal(text);
                let i128_ty = self.context.i128_type();
                let i8_ty = self.context.i8_type();
                let lo = (coef as u64) as u128;
                let hi = ((coef >> 64) as u64) as u128;
                let coef_const = i128_ty.const_int_arbitrary_precision(&[lo as u64, hi as u64]);
                let scale_const = i8_ty.const_int(scale as u64, false);
                Some(
                    self.context
                        .struct_type(&[i128_ty.into(), i8_ty.into()], false)
                        .const_named_struct(&[coef_const.into(), scale_const.into()])
                        .into(),
                )
            }

            ir::InstKind::IntToDecimal(operand) => {
                let value = self.operand(*operand).into_int_value();
                // Widen to a signed i128 first (the runtime helper takes the
                // integer by pointer).
                let i128_ty = self.context.i128_type();
                let signed = self.is_signed(self.value_types[&operand.0]);
                let wide = if value.get_type().get_bit_width() == 128 {
                    value
                } else if signed {
                    self.builder
                        .build_int_s_extend(value, i128_ty, "sext.i128")
                        .expect("sign-extend to i128")
                } else {
                    self.builder
                        .build_int_z_extend(value, i128_ty, "zext.i128")
                        .expect("zero-extend to i128")
                };
                Some(self.emit_decimal_call(
                    "zirk_rt_decimal_from_i128",
                    &[(wide.into(), ir::IrType::Int(ir::IntWidth::I128))],
                    ir::IrType::Decimal,
                ))
            }

            ir::InstKind::DecimalToInt(operand) => {
                let value = self.operand(*operand);
                let ir::IrType::Int(target) = instruction.ty else {
                    unreachable!("DecimalToInt declares an integer destination")
                };
                let bits = self
                    .context
                    .i32_type()
                    .const_int(target.bits() as u64, false);
                let signed = self
                    .context
                    .bool_type()
                    .const_int(u64::from(self.is_signed(instruction.ty)), false);
                let wide = self.emit_decimal_call(
                    "zirk_rt_decimal_to_i128_checked",
                    &[
                        (value, ir::IrType::Decimal),
                        (bits.into(), ir::IrType::Int(ir::IntWidth::I32)),
                        (signed.into(), ir::IrType::Boolean),
                    ],
                    ir::IrType::Int(ir::IntWidth::I128),
                );
                let target_ty = self
                    .context
                    .custom_width_int_type(
                        std::num::NonZeroU32::new(target.bits()).expect("nonzero width"),
                    )
                    .expect("valid width");
                let narrowed = self
                    .builder
                    .build_int_truncate_or_bit_cast(wide.into_int_value(), target_ty, "trunc")
                    .expect("narrow the checked i128");
                Some(narrowed.into())
            }

            ir::InstKind::DecimalToFloat(operand) => {
                let value = self.operand(*operand);
                let as_f64 = self
                    .emit_decimal_call(
                        "zirk_rt_decimal_to_f64",
                        &[(value, ir::IrType::Decimal)],
                        ir::IrType::Float(ir::FloatWidth::F64),
                    )
                    .into_float_value();
                let ir::IrType::Float(target) = instruction.ty else {
                    unreachable!("DecimalToFloat declares a Float destination")
                };
                let target_ty = self.float_type(target);
                let converted = match target.bits().cmp(&64) {
                    std::cmp::Ordering::Equal => as_f64,
                    std::cmp::Ordering::Less => self
                        .builder
                        .build_float_trunc(as_f64, target_ty, "fptrunc")
                        .expect("narrow from f64"),
                    std::cmp::Ordering::Greater => self
                        .builder
                        .build_float_ext(as_f64, target_ty, "fpext")
                        .expect("widen from f64"),
                };
                Some(converted.into())
            }

            ir::InstKind::FloatToDecimal(operand) => {
                let value = self.operand(*operand).into_float_value();
                let source = self.value_types[&operand.0];
                let ir::IrType::Float(source_width) = source else {
                    unreachable!("FloatToDecimal converts a Float")
                };
                let as_f64 = if source_width.bits() == 64 {
                    value
                } else if source_width.bits() < 64 {
                    self.builder
                        .build_float_ext(value, self.context.f64_type(), "fpext.f64")
                        .expect("widen to f64")
                } else {
                    self.builder
                        .build_float_trunc(value, self.context.f64_type(), "fptrunc.f64")
                        .expect("narrow to f64")
                };
                Some(self.emit_decimal_call(
                    "zirk_rt_decimal_from_f64",
                    &[(as_f64.into(), ir::IrType::Float(ir::FloatWidth::F64))],
                    ir::IrType::Decimal,
                ))
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

            ir::InstKind::ObjectNew => Some(
                self.builder
                    .build_call(self.runtime.object_new, &[], "dynamic_object")
                    .expect("the dynamic-object allocator returns a pointer")
                    .try_as_basic_value()
                    .basic()
                    .expect("the dynamic-object allocator returns a pointer"),
            ),

            ir::InstKind::ObjectSeal { object } => Some(
                self.builder
                    .build_call(
                        self.runtime.object_seal,
                        &[self.operand(*object).into()],
                        "object_seal_status",
                    )
                    .expect("the object key-set sealer is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the object key-set sealer returns a status"),
            ),

            ir::InstKind::DynamicObjectUnbox { value } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let output = self
                    .builder
                    .build_alloca(pointer, "dynamic_object_out")
                    .expect("a dynamic object unbox gets an output slot");
                self.builder
                    .build_store(output, pointer.const_null())
                    .expect("an absent dynamic object starts as null");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.dynamic_object_unbox,
                        &[self.operand(*value).into(), output.into()],
                        "dynamic_object_status",
                    )
                    .expect("the Dynamic object unbox helper is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the Dynamic object unbox helper returns a status")
                    .into_int_value();
                let present = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        status,
                        self.context.i32_type().const_zero(),
                        "dynamic_object_present",
                    )
                    .expect("the Dynamic object status is compared with success");
                let object = self
                    .builder
                    .build_load(pointer, output, "dynamic_object")
                    .expect("the Dynamic object pointer is loaded");
                let nullable = self
                    .llvm_type(instruction.ty)
                    .expect("DynamicObjectUnbox has a nullable Object representation")
                    .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(nullable.get_undef(), present, 0, "dynamic_present")
                    .expect("the nullable Dynamic object stores its presence flag");
                Some(
                    self.builder
                        .build_insert_value(with_flag, object, 1, "dynamic_unboxed_object")
                        .expect("the nullable Dynamic object stores its pointer")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::ObjectConform {
                object,
                contract,
                requirements,
            } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let usize_type = self.context.i64_type();
                let requirement_type = self.context.struct_type(
                    &[
                        pointer.into(),
                        usize_type.into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let requirement_pointer = if requirements.is_empty() {
                    pointer.const_null()
                } else {
                    let mut entries = Vec::with_capacity(requirements.len());
                    for (index, requirement) in requirements.iter().enumerate() {
                        let bytes = self.context.const_string(requirement.key.as_bytes(), false);
                        let key_global = self.llvm.add_global(
                            bytes.get_type(),
                            None,
                            &format!(
                                "zk.contract.{}.{}.key.{index}",
                                instruction
                                    .result
                                    .expect("ObjectConform returns a status")
                                    .0,
                                contract
                            ),
                        );
                        key_global.set_initializer(&bytes);
                        key_global.set_constant(true);
                        key_global.set_linkage(Linkage::Private);
                        entries.push(
                            self.context.const_struct(
                                &[
                                    key_global.as_pointer_value().into(),
                                    usize_type
                                        .const_int(requirement.key.len() as u64, false)
                                        .into(),
                                    self.context
                                        .i32_type()
                                        .const_int(u64::from(requirement.tag), false)
                                        .into(),
                                ],
                                false,
                            ),
                        );
                    }
                    let array = requirement_type.const_array(&entries);
                    let global = self.llvm.add_global(
                        array.get_type(),
                        None,
                        &format!(
                            "zk.contract.{}.{}.requirements",
                            instruction
                                .result
                                .expect("ObjectConform returns a status")
                                .0,
                            contract
                        ),
                    );
                    global.set_initializer(&array);
                    global.set_constant(true);
                    global.set_linkage(Linkage::Private);
                    global.as_pointer_value()
                };
                let status = self
                    .builder
                    .build_call(
                        self.runtime.object_conform,
                        &[
                            self.operand(*object).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*contract), false)
                                .into(),
                            requirement_pointer.into(),
                            usize_type
                                .const_int(requirements.len() as u64, false)
                                .into(),
                            pointer.const_null().into(),
                        ],
                        "object_conformance_status",
                    )
                    .expect("the runtime contract validator is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the runtime contract validator returns a status");
                Some(status)
            }

            ir::InstKind::DynamicPack { value } => {
                let source_ty = self.value_types[&value.0];
                if source_ty == ir::IrType::Dynamic {
                    Some(self.operand(*value))
                } else {
                    let tag = source_ty
                        .dynamic_tag()
                        .expect("the verifier rejects non-boxable DynamicPack types");
                    let source_type = self
                        .llvm_type(source_ty)
                        .expect("a DynamicPack source has an LLVM layout");
                    let source_slot = self
                        .builder
                        .build_alloca(source_type, "dynamic_payload")
                        .expect("a Dynamic payload gets a stack slot");
                    self.builder
                        .build_store(source_slot, self.operand(*value))
                        .expect("the Dynamic payload is stored before copying");
                    let (offsets, count) = self.gc_offsets_for(
                        source_ty,
                        &format!(
                            "zk.dynamic.offsets.{}",
                            instruction.result.expect("DynamicPack produces a value").0
                        ),
                    );
                    let bytes = self.byte_size_of(source_ty);
                    Some(
                        self.builder
                            .build_call(
                                self.runtime.dynamic_value_new,
                                &[
                                    self.context
                                        .i32_type()
                                        .const_int(u64::from(tag), false)
                                        .into(),
                                    source_slot.into(),
                                    bytes.into(),
                                    offsets.into(),
                                    count.into(),
                                ],
                                "dynamic_value",
                            )
                            .expect("the Dynamic boxing helper is declared")
                            .try_as_basic_value()
                            .basic()
                            .expect("Dynamic boxing returns a pointer"),
                    )
                }
            }

            ir::InstKind::JsonStringifyDynamic { value } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let output = self.aligned_alloca(pointer.into(), "json_stringify_out");
                self.builder
                    .build_store(output, pointer.const_null())
                    .expect("JSON stringify output starts absent");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.json_stringify_dynamic,
                        &[self.operand(*value).into(), output.into()],
                        "json_stringify_status",
                    )
                    .expect("the dynamic JSON stringifier is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the JSON stringifier returns a status")
                    .into_int_value();
                let present = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        status,
                        self.context.i32_type().const_zero(),
                        "json_stringify_present",
                    )
                    .expect("JSON stringify status is compared with success");
                let string = self
                    .builder
                    .build_load(pointer, output, "json_stringify_value")
                    .expect("the JSON output pointer is loaded");
                let nullable = self
                    .llvm_type(instruction.ty)
                    .expect("JSON stringify returns a nullable String")
                    .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(nullable.get_undef(), present, 0, "json_present")
                    .expect("the JSON result stores its presence flag");
                Some(
                    self.builder
                        .build_insert_value(with_flag, string, 1, "json_string")
                        .expect("the JSON result stores its text")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::JsonParseDynamic {
                document,
                list_dynamic_id,
            } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let output = self.aligned_alloca(pointer.into(), "json_parse_out");
                self.builder
                    .build_store(output, pointer.const_null())
                    .expect("JSON parse output starts absent");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.json_parse_dynamic,
                        &[
                            self.operand(*document).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*list_dynamic_id), false)
                                .into(),
                            output.into(),
                        ],
                        "json_parse_status",
                    )
                    .expect("the dynamic JSON parser is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the JSON parser returns a status")
                    .into_int_value();
                let present = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        status,
                        self.context.i32_type().const_zero(),
                        "json_parse_present",
                    )
                    .expect("JSON parse status is compared with success");
                let value = self
                    .builder
                    .build_load(pointer, output, "json_parse_value")
                    .expect("the parsed Dynamic pointer is loaded");
                let nullable = self
                    .llvm_type(instruction.ty)
                    .expect("JSON parse returns a nullable Dynamic")
                    .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(nullable.get_undef(), present, 0, "json_present")
                    .expect("the JSON result stores its presence flag");
                Some(
                    self.builder
                        .build_insert_value(with_flag, value, 1, "json_dynamic")
                        .expect("the JSON result stores its Dynamic tree")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::DynamicIsType { value, tag } => {
                let actual = self
                    .builder
                    .build_call(
                        self.runtime.dynamic_value_tag,
                        &[self.operand(*value).into()],
                        "dynamic_tag",
                    )
                    .expect("the Dynamic tag reader is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the Dynamic tag reader returns a tag")
                    .into_int_value();
                Some(
                    self.builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            actual,
                            self.context.i32_type().const_int(u64::from(*tag), false),
                            "dynamic_type_matches",
                        )
                        .expect("the Dynamic type tag is compared")
                        .into(),
                )
            }

            ir::InstKind::DynamicUnbox { value, tag } => {
                let result_type = self
                    .llvm_type(instruction.ty)
                    .expect("a checked Dynamic target has an LLVM layout");
                let output = self
                    .builder
                    .build_alloca(result_type, "dynamic_unbox_out")
                    .expect("a Dynamic payload gets a typed output slot");
                let size = self.byte_size_of(instruction.ty);
                self.builder
                    .build_call(
                        self.runtime.dynamic_value_copy_to,
                        &[
                            self.operand(*value).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*tag), false)
                                .into(),
                            output.into(),
                            size.into(),
                        ],
                        "dynamic_unbox_status",
                    )
                    .expect("the checked Dynamic payload copier is declared");
                Some(
                    self.builder
                        .build_load(result_type, output, "dynamic_unboxed")
                        .expect("the checked Dynamic payload is loaded"),
                )
            }

            ir::InstKind::ObjectSet { object, key, value } => Some(
                self.builder
                    .build_call(
                        self.runtime.object_set,
                        &[
                            self.operand(*object).into(),
                            self.operand(*key).into(),
                            self.operand(*value).into(),
                        ],
                        "object_set_status",
                    )
                    .expect("the dynamic-object property setter is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the dynamic property setter returns a status"),
            ),

            ir::InstKind::ObjectGet { object, key } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let output = self
                    .builder
                    .build_alloca(pointer, "dynamic_property_out")
                    .expect("a dynamic property result gets an output slot");
                self.builder
                    .build_store(output, pointer.const_null())
                    .expect("a missing dynamic property starts as null");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.object_get,
                        &[
                            self.operand(*object).into(),
                            self.operand(*key).into(),
                            output.into(),
                        ],
                        "dynamic_property_status",
                    )
                    .expect("the dynamic-object property getter is declared")
                    .try_as_basic_value()
                    .basic()
                    .expect("the dynamic property getter returns a status")
                    .into_int_value();
                let present = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        status,
                        self.context.i32_type().const_int(1, false),
                        "dynamic_property_present",
                    )
                    .expect("the property status is compared with present");
                let value = self
                    .builder
                    .build_load(pointer, output, "dynamic_property")
                    .expect("the dynamic property result is loaded");
                let ty = self
                    .llvm_type(instruction.ty)
                    .expect("ObjectGet has its nullable Dynamic representation")
                    .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(ty.get_undef(), present, 0, "dynamic_present")
                    .expect("the nullable presence flag is inserted");
                Some(
                    self.builder
                        .build_insert_value(with_flag, value, 1, "dynamic_wrapped")
                        .expect("the nullable dynamic payload is inserted")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::ObjectMerge {
                destination,
                source,
            } => {
                self.builder
                    .build_call(
                        self.runtime.object_merge,
                        &[
                            self.operand(*destination).into(),
                            self.operand(*source).into(),
                        ],
                        "object_merge_status",
                    )
                    .expect("the dynamic-object merge helper is declared");
                None
            }

            ir::InstKind::ObjectRest {
                object,
                excluded_keys,
            } => {
                let pointer = self.context.ptr_type(AddressSpace::default());
                let count = excluded_keys.len();
                let excluded_pointer = if count == 0 {
                    pointer.const_null()
                } else {
                    let array_type = pointer.array_type(count as u32);
                    let array = self
                        .builder
                        .build_alloca(array_type, "object_rest_exclusions")
                        .expect("object-rest exclusion array allocation");
                    for (index, key) in excluded_keys.iter().enumerate() {
                        let entry = unsafe {
                            self.builder.build_gep(
                                array_type,
                                array,
                                &[
                                    self.context.i32_type().const_zero(),
                                    self.context.i32_type().const_int(index as u64, false),
                                ],
                                "object_rest_exclusion",
                            )
                        }
                        .expect("object-rest exclusion array index");
                        self.builder
                            .build_store(entry, self.operand(*key))
                            .expect("store object-rest exclusion key");
                    }
                    array
                };
                Some(
                    self.builder
                        .build_call(
                            self.runtime.object_rest,
                            &[
                                self.operand(*object).into(),
                                excluded_pointer.into(),
                                self.context
                                    .i64_type()
                                    .const_int(count as u64, false)
                                    .into(),
                            ],
                            "object_rest",
                        )
                        .expect("the object-rest helper is declared")
                        .try_as_basic_value()
                        .basic()
                        .expect("object rest returns a managed object pointer"),
                )
            }

            // A `Void` field (roadmap Phase 4c, `Result<Void,E>`) has no
            // value to extract — the same reason a `Void` call's own result
            // is never inserted into `self.values` below.
            ir::InstKind::LoadField { .. } if instruction.ty == ir::IrType::Void => None,

            ir::InstKind::LoadField { object, index } => {
                // A record or enum payload field comes straight
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
                // Start from a fully initialized value so any nested
                // managed references in fields are null until written.
                let mut built = struct_type.const_zero();
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
                // Inactive flattened payload slots are scanned by GC field
                // descriptors without consulting the enum discriminant.
                // Start from a fully initialized null/zero representation.
                let mut built = struct_type.const_zero();
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
                    // A `Void` field has no operand to insert; its zero-sized
                    // slot remains initialized and is never read by codegen.
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

            ir::InstKind::UnionInject {
                union_id,
                alternative,
                payload,
            } => {
                let layout = &self.module.union_types[*union_id as usize];
                let enum_id = layout
                    .representation_layout
                    .expect("verified executable union");
                let enum_layout = &self.module.enums[enum_id as usize];
                let descriptor = enum_layout
                    .union_layout
                    .as_ref()
                    .expect("verified union has physical layout metadata");
                let struct_type = union_struct(self.context, descriptor);
                let storage = self
                    .builder
                    .build_alloca(struct_type, "union.storage")
                    .expect("allocate inline union construction storage");
                self.builder
                    .build_store(storage, struct_type.const_zero())
                    .expect("zero-initialize union payload storage");
                let payload_slot = self
                    .builder
                    .build_struct_gep(struct_type, storage, 3, "union.payload.slot")
                    .expect("union payload buffer");
                self.builder
                    .build_store(payload_slot, self.operand(*payload))
                    .expect("store union payload into aligned buffer");
                let tag_slot = self
                    .builder
                    .build_struct_gep(struct_type, storage, 0, "union.tag.slot")
                    .expect("union tag field");
                self.builder
                    .build_store(
                        tag_slot,
                        self.context
                            .i32_type()
                            .const_int(u64::from(*alternative), false),
                    )
                    .expect("publish union tag after payload");
                Some(
                    self.builder
                        .build_load(struct_type, storage, "union.value")
                        .expect("load constructed inline union"),
                )
            }
            ir::InstKind::UnionTag(value) => Some(
                self.builder
                    .build_extract_value(self.operand(*value).into_struct_value(), 0, "union.tag")
                    .expect("extract union tag"),
            ),
            ir::InstKind::UnionProject {
                value,
                union_id,
                alternative,
            } => {
                let enum_id = self.module.union_types[*union_id as usize]
                    .representation_layout
                    .expect("verified executable union");
                let enum_layout = &self.module.enums[enum_id as usize];
                let descriptor = enum_layout
                    .union_layout
                    .as_ref()
                    .expect("verified union has physical layout metadata");
                let struct_type = union_struct(self.context, descriptor);
                let aggregate = self.operand(*value).into_struct_value();
                let tag = self
                    .builder
                    .build_extract_value(aggregate, 0, "union.project.tag")
                    .expect("extract union projection tag")
                    .into_int_value();
                let expected = self
                    .context
                    .i32_type()
                    .const_int(u64::from(*alternative), false);
                let matches = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        tag,
                        expected,
                        "union.project.valid",
                    )
                    .expect("compare union projection tag");
                let function = self
                    .builder
                    .get_insert_block()
                    .expect("union projection block")
                    .get_parent()
                    .expect("union projection function");
                let valid_block = self
                    .context
                    .append_basic_block(function, "union.project.valid");
                let invalid_block = self
                    .context
                    .append_basic_block(function, "union.project.invalid");
                self.builder
                    .build_conditional_branch(matches, valid_block, invalid_block)
                    .expect("guard union projection payload");
                self.builder.position_at_end(invalid_block);
                self.builder
                    .build_call(self.runtime.invalid_union_tag, &[], "union.invalid.tag")
                    .expect("report an invalid union tag");
                self.builder
                    .build_unreachable()
                    .expect("invalid union tag handler does not return");
                self.builder.position_at_end(valid_block);
                let storage = self
                    .builder
                    .build_alloca(struct_type, "union.project.storage")
                    .expect("allocate union projection storage");
                self.builder
                    .build_store(storage, aggregate)
                    .expect("spill checked union for payload projection");
                let payload_slot = self
                    .builder
                    .build_struct_gep(struct_type, storage, 3, "union.project.payload")
                    .expect("union projection payload buffer");
                let payload_type = llvm_type_in(
                    self.context,
                    self.module.union_types[*union_id as usize].alternatives[*alternative as usize],
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("union alternative has a value representation");
                Some(
                    self.builder
                        .build_load(payload_type, payload_slot, "union.payload")
                        .expect("load union payload"),
                )
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

            // Byte offset of the `index`-th grapheme, or `-1` past the end
            // (roadmap Phase 4e, `String[index]` read-only access).
            ir::InstKind::StringGraphemeOffset { string, index } => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.str_grapheme_offset,
                        &[self.operand(*string).into(), self.operand(*index).into()],
                        "grapheme_offset",
                    )
                    .expect("call to the grapheme offset lookup");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the lookup returns a value"),
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

            ir::InstKind::SetSuppressed {
                exception,
                suppressed,
            } => {
                self.builder
                    .build_call(
                        self.runtime.set_suppressed,
                        &[
                            self.operand(*exception).into(),
                            self.operand(*suppressed).into(),
                        ],
                        "",
                    )
                    .expect("set the suppressed exception");
                None
            }

            ir::InstKind::StackTrace(exception) => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.stack_trace,
                        &[self.operand(*exception).into()],
                        "stack_trace",
                    )
                    .expect("get the stack trace");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::Suppressed(exception) => {
                let call = self
                    .builder
                    .build_call(
                        self.runtime.suppressed,
                        &[self.operand(*exception).into()],
                        "suppressed",
                    )
                    .expect("get the suppressed exception");
                let ptr = call
                    .try_as_basic_value()
                    .basic()
                    .expect("a pointer result")
                    .into_pointer_value();
                let is_null = self
                    .builder
                    .build_is_null(ptr, "suppressed_is_null")
                    .expect("null check");
                let present = self
                    .builder
                    .build_not(is_null, "suppressed_present")
                    .expect("present flag");

                let ty = llvm_type_in(
                    self.context,
                    instruction.ty,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a nullable type has a representation")
                .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(ty.get_undef(), present, 0, "present")
                    .expect("present flag");
                Some(
                    self.builder
                        .build_insert_value(with_flag.into_struct_value(), ptr, 1, "wrapped")
                        .expect("payload")
                        .as_basic_value_enum(),
                )
            }

            ir::InstKind::HasPendingException => {
                let call = self
                    .builder
                    .build_call(self.runtime.has_pending_exception, &[], "has_pending")
                    .expect("query the pending exception");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::IsCancelled => {
                let call = self
                    .builder
                    .build_call(self.runtime.is_cancelled, &[], "is_cancelled")
                    .expect("query the cancellation token");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::CancelShieldEnter => {
                self.builder
                    .build_call(self.runtime.cancel_shield_enter, &[], "")
                    .expect("enter cancellation cleanup shield");
                None
            }
            ir::InstKind::CancelShieldExit => {
                self.builder
                    .build_call(self.runtime.cancel_shield_exit, &[], "")
                    .expect("exit cancellation cleanup shield");
                None
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
                let store = self
                    .builder
                    .build_store(self.slots[slot], self.operand(*operand))
                    .expect("slot store");
                self.declare_debug_slot(*slot, instruction.span, store);
                None
            }

            ir::InstKind::Unary { op, operand } => {
                let signed = self.is_signed(self.value_types[&operand.0]);
                Some(self.emit_unary(*op, self.operand(*operand), signed))
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
                    left_ty,
                ))
            }

            ir::InstKind::CheckedArithmetic {
                op,
                left,
                right,
                signed,
            } => {
                let l = self.operand(*left).into_int_value();
                let r = self.operand(*right).into_int_value();
                Some(self.emit_overflowed(*op, l, r, *signed))
            }

            ir::InstKind::Call { callee, args } => {
                // The exact-`Float` helpers, the `DateTime` civil-timestamp
                // ones, and the `zirk_int_*` scalar-member ones cross the
                // boundary by pointer (`declare_extern_fn`'s own doc has the
                // full "why"); `emit_decimal_call` (generic despite its
                // name) handles the spill/out-parameter shape. This must
                // stay in exact sync with `declare_extern_fn`'s own
                // `needs_pointer_abi` gate — a caller/callee mismatch there
                // produces garbage values, not necessarily a crash.
                if callee.starts_with("zirk_rt_decimal_")
                    || callee == "zirk_str_from_decimal"
                    || callee.starts_with("zirk_rt_datetime_")
                    || callee.starts_with("zirk_int_")
                {
                    let typed: Vec<_> = args
                        .iter()
                        .map(|a| (self.operand(*a), self.value_types[&a.0]))
                        .collect();
                    Some(self.emit_decimal_call(callee, &typed, instruction.ty))
                } else if callee == "zirk_regex_find" {
                    // The `Nullable<T>` return crosses by a leading
                    // out-pointer instead — `declare_extern_fn`'s doc has
                    // the ABI "why"; this must stay in sync with that gate.
                    let result_ty = self
                        .llvm_type(instruction.ty)
                        .expect("zirk_regex_find's Nullable return is never Void");
                    let slot = self.aligned_alloca(result_ty, "regex_find.out");
                    let mut arguments: Vec<BasicMetadataValueEnum> = vec![slot.into()];
                    arguments.extend(
                        args.iter()
                            .map(|a| BasicMetadataValueEnum::from(self.operand(*a))),
                    );
                    self.builder
                        .build_call(self.functions[callee], &arguments, "call")
                        .expect("call to zirk_regex_find");
                    Some(
                        self.builder
                            .build_load(result_ty, slot, "regex_find.result")
                            .expect("load zirk_regex_find's result"),
                    )
                } else {
                    let arguments: Vec<_> = args.iter().map(|a| self.operand(*a).into()).collect();
                    let call = self
                        .builder
                        .build_call(self.functions[callee], &arguments, "call")
                        .expect("function call");
                    call.try_as_basic_value().basic()
                }
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
                } else if self.value_types[&operand.0] == ir::IrType::Decimal {
                    // The exact `Float` renders its coefficient and scale
                    // directly through the runtime, by pointer.
                    let value = self.operand(*operand);
                    Some(self.emit_decimal_call(
                        "zirk_str_from_decimal",
                        &[(value, ir::IrType::Decimal)],
                        ir::IrType::String,
                    ))
                } else if self.value_types[&operand.0] == ir::IrType::Float(ir::FloatWidth::F128) {
                    let value = self.operand(*operand).into_float_value();
                    let bits = self
                        .builder
                        .build_bit_cast(value, self.context.i128_type(), "to_string.f128_bits")
                        .expect("bitcast Float128 to its exact bit representation");
                    let slot = self
                        .builder
                        .build_alloca(self.context.i128_type(), "to_string.f128")
                        .expect("stack slot for a Float128 conversion");
                    self.builder
                        .build_store(slot, bits)
                        .expect("store Float128 bits for formatting");
                    let call = self
                        .builder
                        .build_call(self.runtime.float128_to_string, &[slot.into()], "str")
                        .expect("call to the binary128 formatter");
                    Some(
                        call.try_as_basic_value()
                            .basic()
                            .expect("the binary128 formatter returns a String"),
                    )
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
                            // `F128` is handled by the direct binary128
                            // formatter above; it must never reach this path.
                            ir::FloatWidth::F128 => {
                                unreachable!("Float128 ToString uses its direct formatter")
                            }
                            ir::FloatWidth::F16 => {
                                unreachable!("ToString over Float16 reaches the F16 arm")
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

            ir::InstKind::Float128Format { value, spec } => {
                let value = self.operand(*value).into_float_value();
                let bits = self
                    .builder
                    .build_bit_cast(value, self.context.i128_type(), "format.f128_bits")
                    .expect("bitcast Float128 to its exact bit representation");
                let slot = self
                    .builder
                    .build_alloca(self.context.i128_type(), "format.f128")
                    .expect("stack slot for Float128 formatting");
                self.builder
                    .build_store(slot, bits)
                    .expect("store Float128 bits for formatting");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.float128_format,
                        &[slot.into(), self.operand(*spec).into()],
                        "formatted.f128",
                    )
                    .expect("call to the direct binary128 format helper");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the binary128 format helper returns a String"),
                )
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

            ir::InstKind::Print(operand) => {
                self.builder
                    .build_call(self.runtime.io_print, &[self.operand(*operand).into()], "")
                    .expect("call to print");
                None
            }

            ir::InstKind::PrintlnStderr(operand) => {
                self.builder
                    .build_call(
                        self.runtime.io_eprintln,
                        &[self.operand(*operand).into()],
                        "",
                    )
                    .expect("call to eprintln");
                None
            }

            ir::InstKind::PrintStderr(operand) => {
                self.builder
                    .build_call(self.runtime.io_eprint, &[self.operand(*operand).into()], "")
                    .expect("call to eprint");
                None
            }

            ir::InstKind::FlushStdout { enum_id } => {
                let call = self
                    .builder
                    .build_call(self.runtime.io_stdout_flush, &[], "flush")
                    .expect("call to zirk_io_stdout_flush");
                let status = call
                    .try_as_basic_value()
                    .basic()
                    .expect("flush returns a status")
                    .into_int_value();
                Some(self.build_flush_result(*enum_id, status))
            }

            ir::InstKind::FlushStderr { enum_id } => {
                let call = self
                    .builder
                    .build_call(self.runtime.io_stderr_flush, &[], "flush")
                    .expect("call to zirk_io_stderr_flush");
                let status = call
                    .try_as_basic_value()
                    .basic()
                    .expect("flush returns a status")
                    .into_int_value();
                Some(self.build_flush_result(*enum_id, status))
            }

            ir::InstKind::StdinReadLineResult { enum_id } => {
                let (kind, error, payload) =
                    self.call_stdin_read(self.runtime.io_stdin_read_line, &[]);
                // `ReadResult<T>`: Value = 0, End = 1, Error = 2
                // (`stdlib/std/io/package.zrk`'s declaration order).
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::StdinReadChar { enum_id } => {
                let (kind, error, payload) =
                    self.call_stdin_read(self.runtime.io_stdin_read_char, &[]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::StdinReadBytes { max, enum_id } => {
                let max_value = self.operand(*max);
                let (kind, error, payload) =
                    self.call_stdin_read(self.runtime.io_stdin_read_bytes, &[max_value]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::StdinReadExact { count, enum_id } => {
                let count_value = self.operand(*count);
                let (kind, error, payload) =
                    self.call_stdin_read(self.runtime.io_stdin_read_exact, &[count_value]);
                // `Result<T, IoError>`: Ok = 0, Error = 1 (`register_native_result_enum`).
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::EnvironmentGet {
                name,
                enum_id,
                secret,
            } => {
                let name_value = self.operand(*name);
                let runtime = if *secret {
                    self.runtime.environment_get_secret
                } else {
                    self.runtime.environment_get
                };
                let (kind, error, payload) = self.call_stdin_read(runtime, &[name_value]);
                // `Result<String, EnvironmentError>` uses the same native
                // out-parameter layout and declaration order as the runtime.
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::EnvironmentContains { name, enum_id } => {
                let name_value = self.operand(*name);
                let (kind, detail, _) =
                    self.call_stdin_read(self.runtime.environment_contains, &[name_value]);
                Some(self.build_boolean_tagged_outcome(*enum_id, kind, detail))
            }

            ir::InstKind::EnvironmentListNames { enum_id } => {
                let (kind, error, payload) =
                    self.call_stdin_read(self.runtime.environment_list_names, &[]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::FilesystemCall { binding, args } => {
                let operation = filesystem_operation(binding).unwrap_or_else(|| {
                    panic!("unregistered filesystem compiler binding: {binding}")
                });
                let words = args
                    .iter()
                    .map(|argument| self.filesystem_argument_word(*argument))
                    .collect::<Vec<_>>();
                let (kind, error, scalar, payload) = self.call_filesystem(operation, &words);
                Some(self.build_filesystem_outcome(instruction.ty, kind, error, scalar, payload))
            }

            ir::InstKind::NetworkTcpConnect { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, scalar, _) =
                    self.call_network_out(self.runtime.network_tcp_connect, &[endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, scalar))
            }

            ir::InstKind::NetworkDnsResolve { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, _, payload) =
                    self.call_network_out(self.runtime.network_dns_resolve, &[endpoint.into()]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::NetworkInterfacesSnapshot { enum_id } => {
                let (kind, error, _, payload) =
                    self.call_network_out(self.runtime.network_interfaces_snapshot, &[]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::NetworkIpAddressParse { value, enum_id } => {
                let value = self.operand(*value);
                let (kind, error, _, payload) =
                    self.call_network_out(self.runtime.network_ip_address_parse, &[value.into()]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::NetworkTcpOriginNormalize {
                host,
                port,
                enum_id,
            } => {
                let host = self.operand(*host);
                let port = self.operand(*port);
                let (kind, error, _, payload) = self.call_network_out(
                    self.runtime.network_tcp_origin_normalize,
                    &[host.into(), port.into()],
                );
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 1))
            }

            ir::InstKind::NetworkTlsConnect { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, handle, _) =
                    self.call_network_out(self.runtime.network_tls_connect, &[endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsConnectWithRoots {
                endpoint,
                roots_der,
                allow_tls12,
                enum_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let roots_der = self.operand(*roots_der);
                let allow_tls12 = self.operand(*allow_tls12);
                let out_ty = self.network_out_struct();
                let out = self.aligned_alloca(out_ty.into(), "network.tls.roots.out");
                self.builder
                    .build_call(
                        self.runtime.network_tls_connect_with_roots,
                        &[
                            endpoint.into(),
                            roots_der.into(),
                            allow_tls12.into(),
                            out.into(),
                        ],
                        "network.tls.roots",
                    )
                    .expect("call configured TLS connect");
                let loaded = self
                    .builder
                    .build_load(out_ty, out, "network.tls.roots.load")
                    .expect("load configured TLS connect result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(loaded, 0, "network.tls.roots.kind")
                    .expect("extract configured TLS result kind")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(loaded, 1, "network.tls.roots.error")
                    .expect("extract configured TLS error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(loaded, 2, "network.tls.roots.handle")
                    .expect("extract configured TLS handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsConnectWithAlpn {
                endpoint,
                roots_der,
                allow_tls12,
                protocols,
                enum_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let roots_der = self.operand(*roots_der);
                let allow_tls12 = self.operand(*allow_tls12);
                let protocols = self.operand(*protocols);
                let out_ty = self.network_out_struct();
                let out = self.aligned_alloca(out_ty.into(), "network.tls.alpn.out");
                self.builder
                    .build_call(
                        self.runtime.network_tls_connect_with_alpn,
                        &[
                            endpoint.into(),
                            roots_der.into(),
                            allow_tls12.into(),
                            protocols.into(),
                            out.into(),
                        ],
                        "network.tls.alpn",
                    )
                    .expect("call configured TLS ALPN connect");
                let loaded = self
                    .builder
                    .build_load(out_ty, out, "network.tls.alpn.load")
                    .expect("load configured TLS ALPN result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(loaded, 0, "network.tls.alpn.kind")
                    .expect("extract configured TLS ALPN result kind")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(loaded, 1, "network.tls.alpn.error")
                    .expect("extract configured TLS ALPN result error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(loaded, 2, "network.tls.alpn.handle")
                    .expect("extract configured TLS ALPN handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsUpgradeTcp {
                tcp_handle,
                endpoint,
                roots_der,
                allow_tls12,
                protocols,
                enum_id,
                stream_class_id,
            } => {
                let (kind, error, stream, _) = self.call_network_out(
                    self.runtime.network_tls_upgrade_tcp,
                    &[
                        self.operand(*tcp_handle).into(),
                        self.operand(*endpoint).into(),
                        self.operand(*roots_der).into(),
                        self.operand(*allow_tls12).into(),
                        self.operand(*protocols).into(),
                    ],
                );
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *stream_class_id,
                    kind,
                    error,
                    stream,
                ))
            }

            ir::InstKind::NetworkTlsConnectWithClientIdentity {
                endpoint,
                roots_der,
                certificate_chain_der,
                private_key_der,
                allow_tls12,
                protocols,
                enum_id,
            } => {
                let out_ty = self.network_out_struct();
                let out = self.aligned_alloca(out_ty.into(), "network.tls.identity.out");
                self.builder
                    .build_call(
                        self.runtime.network_tls_connect_with_client_identity,
                        &[
                            self.operand(*endpoint).into(),
                            self.operand(*roots_der).into(),
                            self.operand(*certificate_chain_der).into(),
                            self.operand(*private_key_der).into(),
                            self.operand(*allow_tls12).into(),
                            self.operand(*protocols).into(),
                            out.into(),
                        ],
                        "network.tls.identity",
                    )
                    .expect("call mutual TLS client connect");
                let loaded = self
                    .builder
                    .build_load(out_ty, out, "network.tls.identity.load")
                    .expect("load mutual TLS result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(loaded, 0, "network.tls.identity.kind")
                    .expect("extract mutual TLS result kind")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(loaded, 1, "network.tls.identity.error")
                    .expect("extract mutual TLS result error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(loaded, 2, "network.tls.identity.handle")
                    .expect("extract mutual TLS stream handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsListen {
                endpoint,
                certificate_chain_der,
                private_key_der,
                allow_tls12,
                http_alpn,
                enum_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let certificate_chain_der = self.operand(*certificate_chain_der);
                let private_key_der = self.operand(*private_key_der);
                let allow_tls12 = self.operand(*allow_tls12);
                let bind = if *http_alpn {
                    self.runtime.network_tls_listen_http
                } else {
                    self.runtime.network_tls_listen
                };
                let (kind, error, handle, _) = self.call_network_out(
                    bind,
                    &[
                        endpoint.into(),
                        certificate_chain_der.into(),
                        private_key_der.into(),
                        allow_tls12.into(),
                    ],
                );
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsListenWithClientAuth {
                endpoint,
                certificate_chain_der,
                private_key_der,
                client_roots_der,
                allow_tls12,
                enum_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let certificate_chain_der = self.operand(*certificate_chain_der);
                let private_key_der = self.operand(*private_key_der);
                let client_roots_der = self.operand(*client_roots_der);
                let allow_tls12 = self.operand(*allow_tls12);
                let (kind, error, handle, _) = self.call_network_out(
                    self.runtime.network_tls_listen_with_client_auth,
                    &[
                        endpoint.into(),
                        certificate_chain_der.into(),
                        private_key_der.into(),
                        client_roots_der.into(),
                        allow_tls12.into(),
                    ],
                );
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkTlsAccept {
                handle,
                shutdown_control,
                enum_id,
                stream_class_id,
            } => {
                let handle = self.operand(*handle);
                let (kind, error, stream, _) = if let Some(control) = shutdown_control {
                    let control = self.operand(*control);
                    self.call_network_out(
                        self.runtime.network_tls_accept_until_shutdown,
                        &[handle.into(), control.into()],
                    )
                } else {
                    self.call_network_out(self.runtime.network_tls_accept, &[handle.into()])
                };
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *stream_class_id,
                    kind,
                    error,
                    stream,
                ))
            }

            ir::InstKind::NetworkTlsCloseListener { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) = self
                    .call_network_out(self.runtime.network_tls_close_listener, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTlsListenerIsClosed { handle } => {
                let handle = self.operand(*handle);
                let value = self
                    .builder
                    .build_call(
                        self.runtime.network_tls_listener_is_closed,
                        &[handle.into()],
                        "tls.listener_is_closed",
                    )
                    .expect("call TLS listener is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("TLS listener is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::NetworkTlsPeerAddress { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, address) =
                    self.call_network_out(self.runtime.network_tls_peer_address, &[handle.into()]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, address, 0, 1))
            }

            ir::InstKind::NetworkTlsNegotiatedAlpn { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, protocol) = self
                    .call_network_out(self.runtime.network_tls_negotiated_alpn, &[handle.into()]);
                Some(
                    self.build_nullable_string_tagged_outcome(
                        *enum_id, kind, error, protocol, 0, 1,
                    ),
                )
            }

            ir::InstKind::NetworkTlsPeerCertificateSha256 { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, fingerprint) = self.call_network_out(
                    self.runtime.network_tls_peer_certificate_sha256,
                    &[handle.into()],
                );
                Some(self.build_nullable_string_tagged_outcome(
                    *enum_id,
                    kind,
                    error,
                    fingerprint,
                    0,
                    1,
                ))
            }

            ir::InstKind::NetworkTlsRead {
                handle,
                max_bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let max_bytes = self.operand(*max_bytes);
                let (kind, error, _, payload) = self.call_network_out(
                    self.runtime.network_tls_read,
                    &[handle.into(), max_bytes.into()],
                );
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::NetworkTlsWriteAll {
                handle,
                bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let (kind, error, _, _) = self.call_network_out(
                    self.runtime.network_tls_write_all,
                    &[handle.into(), bytes.into()],
                );
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTlsShutdown { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_tls_shutdown, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTlsClose { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_tls_close, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTlsIsClosed { handle } => {
                let handle = self.operand(*handle);
                let value = self
                    .builder
                    .build_call(
                        self.runtime.network_tls_is_closed,
                        &[handle.into()],
                        "tls.is_closed",
                    )
                    .expect("call TLS is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("TLS is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::NetworkTcpRead {
                handle,
                max_bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let max_bytes = self.operand(*max_bytes);
                let (kind, error, _, payload) = self.call_network_out(
                    self.runtime.network_tcp_read,
                    &[handle.into(), max_bytes.into()],
                );
                // `TCPReadResult<T>` is Value(0), End(1), Error(2).
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::NetworkTcpWrite {
                handle,
                bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let (kind, error, scalar, _) = self.call_network_out(
                    self.runtime.network_tcp_write,
                    &[handle.into(), bytes.into()],
                );
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, scalar))
            }

            ir::InstKind::NetworkTcpWriteAll {
                handle,
                bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let (kind, error, _, _) = self.call_network_out(
                    self.runtime.network_tcp_write_all,
                    &[handle.into(), bytes.into()],
                );
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTcpShutdown { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_tcp_shutdown, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTcpClose { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_tcp_close, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTcpIsClosed { handle } => {
                let handle = self.operand(*handle);
                let value = self
                    .builder
                    .build_call(
                        self.runtime.network_tcp_is_closed,
                        &[handle.into()],
                        "tcp.is_closed",
                    )
                    .expect("call TCP is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::NetworkTcpPeerAddress { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, address) =
                    self.call_network_out(self.runtime.network_tcp_peer_address, &[handle.into()]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, address, 0, 1))
            }

            ir::InstKind::NetworkTcpListen { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, scalar, _) =
                    self.call_network_out(self.runtime.network_tcp_listen, &[endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, scalar))
            }

            ir::InstKind::NetworkTcpAccept {
                handle,
                shutdown_control,
                enum_id,
                stream_class_id,
            } => {
                let handle = self.operand(*handle);
                let (kind, error, scalar, _) = if let Some(control) = shutdown_control {
                    let control = self.operand(*control);
                    self.call_network_out(
                        self.runtime.network_tcp_accept_until_shutdown,
                        &[handle.into(), control.into()],
                    )
                } else {
                    self.call_network_out(self.runtime.network_tcp_accept, &[handle.into()])
                };
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *stream_class_id,
                    kind,
                    error,
                    scalar,
                ))
            }

            ir::InstKind::NetworkTcpCloseListener { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) = self
                    .call_network_out(self.runtime.network_tcp_close_listener, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkTcpListenerIsClosed { handle } => {
                let handle = self.operand(*handle);
                let value = self
                    .builder
                    .build_call(
                        self.runtime.network_tcp_listener_is_closed,
                        &[handle.into()],
                        "tcp.listener_is_closed",
                    )
                    .expect("call TCP listener is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("listener is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::NetworkUdpConnect { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, handle, _) =
                    self.call_network_out(self.runtime.network_udp_connect, &[endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkUdpBind { endpoint, enum_id } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, handle, _) =
                    self.call_network_out(self.runtime.network_udp_bind, &[endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }

            ir::InstKind::NetworkUdpSend {
                handle,
                bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let (kind, error, written, _) = self.call_network_out(
                    self.runtime.network_udp_send,
                    &[handle.into(), bytes.into()],
                );
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, written))
            }

            ir::InstKind::NetworkUdpSendTo {
                handle,
                bytes,
                endpoint,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let endpoint = self.operand(*endpoint);
                let (kind, error, written, _) = self.call_network_out(
                    self.runtime.network_udp_send_to,
                    &[handle.into(), bytes.into(), endpoint.into()],
                );
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, written))
            }

            ir::InstKind::NetworkUdpSendBroadcast {
                handle,
                bytes,
                endpoint,
                enum_id,
            }
            | ir::InstKind::NetworkUdpSendMulticast {
                handle,
                bytes,
                endpoint,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let endpoint = self.operand(*endpoint);
                let function = if matches!(
                    instruction.kind,
                    ir::InstKind::NetworkUdpSendBroadcast { .. }
                ) {
                    self.runtime.network_udp_send_broadcast
                } else {
                    self.runtime.network_udp_send_multicast
                };
                let (kind, error, written, _) = self
                    .call_network_out(function, &[handle.into(), bytes.into(), endpoint.into()]);
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, written))
            }

            ir::InstKind::NetworkUdpJoinMulticast {
                handle,
                group,
                enum_id,
            }
            | ir::InstKind::NetworkUdpLeaveMulticast {
                handle,
                group,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let group = self.operand(*group);
                let function = if matches!(
                    instruction.kind,
                    ir::InstKind::NetworkUdpJoinMulticast { .. }
                ) {
                    self.runtime.network_udp_join_multicast
                } else {
                    self.runtime.network_udp_leave_multicast
                };
                let (kind, error, _, _) =
                    self.call_network_out(function, &[handle.into(), group.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkUdpReceive {
                handle,
                max_bytes,
                enum_id,
                datagram_class_id,
            } => {
                let handle = self.operand(*handle);
                let max_bytes = self.operand(*max_bytes);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "network.udp.receive.out");
                self.builder
                    .build_call(
                        self.runtime.network_udp_receive,
                        &[handle.into(), max_bytes.into(), out.into()],
                        "network.udp.receive",
                    )
                    .expect("call UDP receive runtime operation");
                let loaded = self
                    .builder
                    .build_load(out_ty, out, "network.udp.receive.load")
                    .expect("load UDP datagram result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(loaded, 0, "network.udp.receive.kind")
                    .expect("extract UDP result kind")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(loaded, 1, "network.udp.receive.error")
                    .expect("extract UDP result error")
                    .into_int_value();
                let bytes = self
                    .builder
                    .build_extract_value(loaded, 2, "network.udp.receive.bytes")
                    .expect("extract UDP payload")
                    .into_pointer_value();
                let peer = self
                    .builder
                    .build_extract_value(loaded, 3, "network.udp.receive.peer")
                    .expect("extract UDP peer endpoint")
                    .into_pointer_value();
                Some(self.build_udp_datagram_tagged_outcome(
                    *enum_id,
                    *datagram_class_id,
                    kind,
                    error,
                    bytes,
                    peer,
                ))
            }

            ir::InstKind::NetworkUdpClose { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_udp_close, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkUdpLocalAddress { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, address) =
                    self.call_network_out(self.runtime.network_udp_local_address, &[handle.into()]);
                Some(self.build_tagged_outcome(*enum_id, kind, error, address, 0, 1))
            }

            ir::InstKind::NetworkUdpIsClosed { handle } => {
                let handle = self.operand(*handle);
                let value = self
                    .builder
                    .build_call(
                        self.runtime.network_udp_is_closed,
                        &[handle.into()],
                        "udp.is_closed",
                    )
                    .expect("call UDP is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("UDP is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::NetworkLocalConnect {
                endpoint,
                enum_id,
                stream_class_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let (kind, error, handle, _) =
                    self.call_network_out(self.runtime.network_local_connect, &[endpoint.into()]);
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *stream_class_id,
                    kind,
                    error,
                    handle,
                ))
            }

            ir::InstKind::NetworkLocalBind {
                endpoint,
                remove_stale,
                enum_id,
                listener_class_id,
            } => {
                let endpoint = self.operand(*endpoint);
                let remove_stale = self.operand(*remove_stale);
                let (kind, error, handle, _) = self.call_network_out(
                    self.runtime.network_local_bind,
                    &[endpoint.into(), remove_stale.into()],
                );
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *listener_class_id,
                    kind,
                    error,
                    handle,
                ))
            }

            ir::InstKind::NetworkLocalAccept {
                handle,
                enum_id,
                stream_class_id,
            } => {
                let handle = self.operand(*handle);
                let (kind, error, stream_handle, _) =
                    self.call_network_out(self.runtime.network_local_accept, &[handle.into()]);
                Some(self.build_u64_object_tagged_outcome(
                    *enum_id,
                    *stream_class_id,
                    kind,
                    error,
                    stream_handle,
                ))
            }

            ir::InstKind::NetworkLocalRead {
                handle,
                max_bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let max_bytes = self.operand(*max_bytes);
                let (kind, error, _, payload) = self.call_network_out(
                    self.runtime.network_local_read,
                    &[handle.into(), max_bytes.into()],
                );
                Some(self.build_tagged_outcome(*enum_id, kind, error, payload, 0, 2))
            }

            ir::InstKind::NetworkLocalWrite {
                handle,
                bytes,
                enum_id,
                write_all,
            } => {
                let handle = self.operand(*handle);
                let bytes = self.operand(*bytes);
                let function = if *write_all {
                    self.runtime.network_local_write_all
                } else {
                    self.runtime.network_local_write
                };
                let (kind, error, written, _) =
                    self.call_network_out(function, &[handle.into(), bytes.into()]);
                if *write_all {
                    Some(self.build_void_tagged_outcome(*enum_id, kind, error))
                } else {
                    Some(self.build_u64_tagged_outcome(*enum_id, kind, error, written))
                }
            }

            ir::InstKind::NetworkLocalShutdown { handle, enum_id } => {
                let handle = self.operand(*handle);
                let (kind, error, _, _) =
                    self.call_network_out(self.runtime.network_local_shutdown, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkLocalClose {
                handle,
                enum_id,
                listener,
            } => {
                let handle = self.operand(*handle);
                let function = if *listener {
                    self.runtime.network_local_close_listener
                } else {
                    self.runtime.network_local_close_stream
                };
                let (kind, error, _, _) = self.call_network_out(function, &[handle.into()]);
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::NetworkLocalIsClosed { handle, listener } => {
                let handle = self.operand(*handle);
                let function = if *listener {
                    self.runtime.network_local_listener_is_closed
                } else {
                    self.runtime.network_local_stream_is_closed
                };
                let value = self
                    .builder
                    .build_call(function, &[handle.into()], "local.is_closed")
                    .expect("call local resource is_closed")
                    .try_as_basic_value()
                    .basic()
                    .expect("local resource is_closed returns a Boolean")
                    .into_int_value();
                Some(value.into())
            }

            ir::InstKind::HttpGet {
                url,
                cached,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.get.out");
                self.builder
                    .build_call(
                        if *cached {
                            self.runtime.http_get_cached
                        } else {
                            self.runtime.http_get
                        },
                        &[url.into(), out.into()],
                        "http.get",
                    )
                    .expect("call the HTTP GET runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.get.out.load")
                    .expect("load HTTP GET outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.get.kind")
                    .expect("extract HTTP result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.get.error")
                    .expect("extract HTTP error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.get.status")
                    .expect("extract HTTP status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.get.body")
                    .expect("extract HTTP body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.get.headers")
                    .expect("extract HTTP response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpRequest {
                method,
                url,
                body,
                headers,
                validate_json,
                cached,
                enum_id,
                response_class_id,
            } => {
                let method = self.operand(*method);
                let url = self.operand(*url);
                let body = self.operand(*body);
                let headers = self.operand(*headers);
                let validate_json = self.operand(*validate_json);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.request.out");
                let call_args = if *cached {
                    vec![
                        method.into(),
                        url.into(),
                        body.into(),
                        headers.into(),
                        out.into(),
                    ]
                } else {
                    vec![
                        method.into(),
                        url.into(),
                        body.into(),
                        headers.into(),
                        validate_json.into(),
                        out.into(),
                    ]
                };
                self.builder
                    .build_call(
                        if *cached {
                            self.runtime.http_request_cached
                        } else {
                            self.runtime.http_request
                        },
                        &call_args,
                        "http.request",
                    )
                    .expect("call the generic HTTP request runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.request.out.load")
                    .expect("load generic HTTP request outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.request.kind")
                    .expect("extract HTTP request tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.request.error")
                    .expect("extract HTTP request error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.request.status")
                    .expect("extract HTTP request status")
                    .into_int_value();
                let response_body = self
                    .builder
                    .build_extract_value(value, 3, "http.request.body")
                    .expect("extract HTTP request response body")
                    .into_pointer_value();
                let response_headers = self
                    .builder
                    .build_extract_value(value, 4, "http.request.headers")
                    .expect("extract HTTP request response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (response_body, response_headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpClientCreate {
                max_origins,
                enum_id,
            } => {
                let max_origins = self.operand(*max_origins).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.create.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_create,
                        &[max_origins.into(), out.into()],
                        "http.client.create",
                    )
                    .expect("call reusable HTTP client create adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.create.value")
                    .expect("load HTTP client create result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.create.kind")
                    .expect("extract HTTP client create tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.create.error")
                    .expect("extract HTTP client create error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.client.create.handle")
                    .expect("extract HTTP client handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpClientCreateWithOptions {
                max_origins,
                connect_timeout_ms,
                response_headers_timeout_ms,
                response_idle_timeout_ms,
                protocol_code,
                enum_id,
            } => {
                let max_origins = self.operand(*max_origins).into_int_value();
                let connect_timeout_ms = self.operand(*connect_timeout_ms).into_int_value();
                let response_headers_timeout_ms =
                    self.operand(*response_headers_timeout_ms).into_int_value();
                let response_idle_timeout_ms =
                    self.operand(*response_idle_timeout_ms).into_int_value();
                let protocol_code = self.operand(*protocol_code).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.options.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_create_with_protocol_options,
                        &[
                            max_origins.into(),
                            connect_timeout_ms.into(),
                            response_headers_timeout_ms.into(),
                            response_idle_timeout_ms.into(),
                            protocol_code.into(),
                            out.into(),
                        ],
                        "http.client.options.create",
                    )
                    .expect("call HTTP client create with options adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.options.value")
                    .expect("load HTTP client options result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.options.kind")
                    .expect("extract HTTP client options tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.options.error")
                    .expect("extract HTTP client options error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.client.options.handle")
                    .expect("extract HTTP client options handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpClientCreateWithTrustRoots {
                roots,
                max_origins,
                connect_timeout_ms,
                response_headers_timeout_ms,
                response_idle_timeout_ms,
                protocol_code,
                enum_id,
            } => {
                let roots = self.operand(*roots);
                let max_origins = self.operand(*max_origins).into_int_value();
                let connect_timeout_ms = self.operand(*connect_timeout_ms).into_int_value();
                let response_headers_timeout_ms =
                    self.operand(*response_headers_timeout_ms).into_int_value();
                let response_idle_timeout_ms =
                    self.operand(*response_idle_timeout_ms).into_int_value();
                let protocol_code = self.operand(*protocol_code).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.create.roots.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_create_with_trust_roots,
                        &[
                            max_origins.into(),
                            roots.into(),
                            connect_timeout_ms.into(),
                            response_headers_timeout_ms.into(),
                            response_idle_timeout_ms.into(),
                            protocol_code.into(),
                            out.into(),
                        ],
                        "http.client.create.roots",
                    )
                    .expect("call HTTP client create with trust roots adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.create.roots.value")
                    .expect("load HTTP client create with roots result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.create.roots.kind")
                    .expect("extract HTTP client create with roots tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.create.roots.error")
                    .expect("extract HTTP client create with roots error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.client.create.roots.handle")
                    .expect("extract HTTP client handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpClientCreateWithProxy {
                proxy_url,
                proxy_credentials,
                max_origins,
                connect_timeout_ms,
                response_headers_timeout_ms,
                response_idle_timeout_ms,
                protocol_code,
                enum_id,
            } => {
                let proxy_url = self.operand(*proxy_url);
                let proxy_credentials = self.operand(*proxy_credentials);
                let max_origins = self.operand(*max_origins).into_int_value();
                let connect_timeout_ms = self.operand(*connect_timeout_ms).into_int_value();
                let response_headers_timeout_ms =
                    self.operand(*response_headers_timeout_ms).into_int_value();
                let response_idle_timeout_ms =
                    self.operand(*response_idle_timeout_ms).into_int_value();
                let protocol_code = self.operand(*protocol_code).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.create.proxy.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_create_with_proxy,
                        &[
                            proxy_url.into(),
                            proxy_credentials.into(),
                            max_origins.into(),
                            connect_timeout_ms.into(),
                            response_headers_timeout_ms.into(),
                            response_idle_timeout_ms.into(),
                            protocol_code.into(),
                            out.into(),
                        ],
                        "http.client.create.proxy",
                    )
                    .expect("call HTTP client create with proxy adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.create.proxy.value")
                    .expect("load HTTP client create with proxy result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.create.proxy.kind")
                    .expect("extract HTTP client create with proxy tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.create.proxy.error")
                    .expect("extract HTTP client create with proxy error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.client.create.proxy.handle")
                    .expect("extract HTTP client handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpClientCreateWithProxyAndTrustRoots {
                proxy_url,
                proxy_credentials,
                roots,
                max_origins,
                connect_timeout_ms,
                response_headers_timeout_ms,
                response_idle_timeout_ms,
                protocol_code,
                enum_id,
            } => {
                let proxy_url = self.operand(*proxy_url);
                let proxy_credentials = self.operand(*proxy_credentials);
                let roots = self.operand(*roots);
                let max_origins = self.operand(*max_origins).into_int_value();
                let connect_timeout_ms = self.operand(*connect_timeout_ms).into_int_value();
                let response_headers_timeout_ms =
                    self.operand(*response_headers_timeout_ms).into_int_value();
                let response_idle_timeout_ms =
                    self.operand(*response_idle_timeout_ms).into_int_value();
                let protocol_code = self.operand(*protocol_code).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.create.proxy.roots.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_create_with_proxy_and_trust_roots,
                        &[
                            proxy_url.into(),
                            proxy_credentials.into(),
                            max_origins.into(),
                            roots.into(),
                            connect_timeout_ms.into(),
                            response_headers_timeout_ms.into(),
                            response_idle_timeout_ms.into(),
                            protocol_code.into(),
                            out.into(),
                        ],
                        "http.client.create.proxy.roots",
                    )
                    .expect("call HTTP client create with proxy and trust roots adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.create.proxy.roots.value")
                    .expect("load HTTP client create with proxy and roots result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.create.proxy.roots.kind")
                    .expect("extract HTTP client create result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.create.proxy.roots.error")
                    .expect("extract HTTP client create error")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.client.create.proxy.roots.handle")
                    .expect("extract HTTP client handle")
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpClientRequest {
                handle,
                method,
                url,
                body,
                headers,
                validate_json,
                enum_id,
                response_class_id,
            } => {
                let handle = self.operand(*handle).into_int_value();
                let method = self.operand(*method);
                let url = self.operand(*url);
                let body = self.operand(*body);
                let headers = self.operand(*headers);
                let validate_json = self.operand(*validate_json);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.request.out");
                self.builder
                    .build_call(
                        self.runtime.http_client_request,
                        &[
                            handle.into(),
                            method.into(),
                            url.into(),
                            body.into(),
                            headers.into(),
                            validate_json.into(),
                            out.into(),
                        ],
                        "http.client.request",
                    )
                    .expect("call reusable HTTP client request adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.request.value")
                    .expect("load HTTP client response outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.request.kind")
                    .expect("extract HTTP client response tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.request.error")
                    .expect("extract HTTP client response error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.client.request.status")
                    .expect("extract HTTP client response status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.client.request.body")
                    .expect("extract HTTP client response body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.client.request.headers")
                    .expect("extract HTTP client response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.client.request.protocol")
                    .expect("extract HTTP client negotiated protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpClientRequestStream {
                handle,
                method,
                url,
                headers,
                callback,
                callback_layout,
                enum_id,
                response_class_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let captures = self
                    .builder
                    .build_extract_value(
                        callable,
                        CALLABLE_CAPTURE_FIELD,
                        "http.client.upload.captures",
                    )
                    .expect("extract reusable HTTP upload captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http.client.upload.capture.root")
                    .expect("allocate reusable HTTP upload capture root");
                self.builder
                    .build_store(capture_root, captures)
                    .expect("store reusable HTTP upload captures");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http.client.upload.roots")
                    .expect("allocate reusable HTTP upload roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http.client.upload.root.entry",
                    )
                }
                .expect("index reusable HTTP upload root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register reusable HTTP upload root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http.client.upload.push.roots",
                    )
                    .expect("root reusable HTTP upload captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http.client.upload.context")
                    .expect("allocate reusable HTTP upload context");
                self.builder
                    .build_store(context, callable)
                    .expect("store reusable HTTP upload callback");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                        ptr.into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.upload.out");
                let thunk = self.http_upload_producer_thunk(*callback_layout, false);
                self.builder
                    .build_call(
                        self.runtime.http_client_request_stream,
                        &[
                            self.operand(*handle).into(),
                            self.operand(*method).into(),
                            self.operand(*url).into(),
                            self.operand(*headers).into(),
                            thunk.as_global_value().as_pointer_value().into(),
                            context.into(),
                            self.context.i32_type().const_zero().into(),
                            out.into(),
                        ],
                        "http.client.upload.send",
                    )
                    .expect("send reusable HTTP streaming request");
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http.client.upload.pop.roots")
                    .expect("release reusable HTTP upload captures");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.upload.value")
                    .expect("load reusable HTTP upload result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.upload.kind")
                    .expect("extract reusable HTTP result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.upload.error")
                    .expect("extract reusable HTTP error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.client.upload.status")
                    .expect("extract reusable HTTP status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.client.upload.body")
                    .expect("extract reusable HTTP body")
                    .into_pointer_value();
                let response_headers = self
                    .builder
                    .build_extract_value(value, 4, "http.client.upload.headers")
                    .expect("extract reusable HTTP headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.client.upload.protocol")
                    .expect("extract reusable HTTP negotiated protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, response_headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpClientAction {
                handle,
                close,
                clear_cookies,
                enum_id,
            } => {
                let handle = self.operand(*handle).into_int_value();
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.client.action.out");
                let function = if *close {
                    self.runtime.http_client_close
                } else if *clear_cookies {
                    self.runtime.http_client_clear_cookies
                } else {
                    self.runtime.http_client_enable_cookies
                };
                self.builder
                    .build_call(function, &[handle.into(), out.into()], "http.client.action")
                    .expect("call HTTP client action adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.client.action.value")
                    .expect("load HTTP client action outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.client.action.kind")
                    .expect("extract HTTP client action tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.client.action.error")
                    .expect("extract HTTP client action error")
                    .into_int_value();
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }
            ir::InstKind::HttpWebSocketConnect {
                url,
                origin,
                subprotocols,
                http3,
                enum_id,
            } => {
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.connect.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_connect,
                        &[
                            self.operand(*url).into(),
                            self.operand(*origin).into(),
                            self.operand(*subprotocols).into(),
                            self.context
                                .bool_type()
                                .const_int(u64::from(*http3), false)
                                .into(),
                            out.into(),
                        ],
                        "http.websocket.connect",
                    )
                    .expect("connect a bounded HTTP WebSocket client");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.connect.load")
                    .expect("load WebSocket client handle")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.kind")
                    .unwrap()
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.error")
                    .unwrap()
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.websocket.handle")
                    .unwrap()
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpWebSocketConnectWithTrustRoots {
                url,
                origin,
                subprotocols,
                roots_der,
                allow_tls12,
                http3,
                enum_id,
            } => {
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.roots.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_connect_with_trust_roots,
                        &[
                            self.operand(*url).into(),
                            self.operand(*origin).into(),
                            self.operand(*subprotocols).into(),
                            self.operand(*roots_der).into(),
                            self.operand(*allow_tls12).into(),
                            self.context
                                .bool_type()
                                .const_int(u64::from(*http3), false)
                                .into(),
                            out.into(),
                        ],
                        "http.websocket.roots.connect",
                    )
                    .expect("connect WebSocket client with explicit trust roots");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.roots.load")
                    .expect("load WebSocket client with roots result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.roots.kind")
                    .unwrap()
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.roots.error")
                    .unwrap()
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 2, "http.websocket.roots.handle")
                    .unwrap()
                    .into_int_value();
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, handle))
            }
            ir::InstKind::HttpWebSocketSend {
                handle,
                record,
                enum_id,
            } => {
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.send.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_send,
                        &[
                            self.operand(*handle).into(),
                            self.operand(*record).into(),
                            out.into(),
                        ],
                        "http.websocket.send",
                    )
                    .expect("send one bounded WebSocket message");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.send.load")
                    .unwrap()
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.send.kind")
                    .unwrap()
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.send.error")
                    .unwrap()
                    .into_int_value();
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }
            ir::InstKind::HttpWebSocketReceive { handle, enum_id } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.receive.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_receive,
                        &[self.operand(*handle).into(), out.into()],
                        "http.websocket.receive",
                    )
                    .expect("receive one bounded WebSocket message");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.receive.load")
                    .unwrap()
                    .into_struct_value();
                let raw_kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.receive.kind")
                    .unwrap()
                    .into_int_value();
                let is_error = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        raw_kind,
                        self.context.i32_type().const_int(2, false),
                        "http.websocket.receive.is_error",
                    )
                    .unwrap();
                let kind = self
                    .builder
                    .build_int_z_extend(
                        is_error,
                        self.context.i32_type(),
                        "http.websocket.receive.result_kind",
                    )
                    .unwrap();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.receive.error")
                    .unwrap()
                    .into_int_value();
                let record = self
                    .builder
                    .build_extract_value(value, 2, "http.websocket.receive.record")
                    .unwrap()
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, record, 0, 1))
            }
            ir::InstKind::HttpWebSocketClose { handle, enum_id } => {
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.close.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_close,
                        &[self.operand(*handle).into(), out.into()],
                        "http.websocket.close",
                    )
                    .expect("close a task-owned WebSocket");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.close.load")
                    .unwrap()
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.close.kind")
                    .unwrap()
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.close.error")
                    .unwrap()
                    .into_int_value();
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }
            ir::InstKind::HttpWebSocketCloseWith {
                handle,
                code,
                reason,
                enum_id,
            } => {
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.close_with.out");
                self.builder
                    .build_call(
                        self.runtime.http_websocket_close_with,
                        &[
                            self.operand(*handle).into(),
                            self.operand(*code).into(),
                            self.operand(*reason).into(),
                            out.into(),
                        ],
                        "http.websocket.close_with",
                    )
                    .expect("close a task-owned WebSocket with a status");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.close_with.load")
                    .unwrap()
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.close_with.kind")
                    .unwrap()
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.close_with.error")
                    .unwrap()
                    .into_int_value();
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }
            ir::InstKind::HttpRequestSecret {
                method,
                url,
                body,
                headers,
                header_name,
                secret_value,
                enum_id,
                response_class_id,
            } => {
                let method = self.operand(*method);
                let url = self.operand(*url);
                let body = self.operand(*body);
                let headers = self.operand(*headers);
                let header_name = self.operand(*header_name);
                let secret_value = self.operand(*secret_value);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.secret_request.out");
                self.builder
                    .build_call(
                        self.runtime.http_request_secret,
                        &[
                            method.into(),
                            url.into(),
                            body.into(),
                            headers.into(),
                            header_name.into(),
                            secret_value.into(),
                            out.into(),
                        ],
                        "http.secret_request",
                    )
                    .expect("call the explicit HTTP secret sink adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.secret_request.out.load")
                    .expect("load HTTP secret sink outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.secret_request.kind")
                    .expect("extract HTTP secret sink tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.secret_request.error")
                    .expect("extract HTTP secret sink error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.secret_request.status")
                    .expect("extract HTTP secret sink status")
                    .into_int_value();
                let response_body = self
                    .builder
                    .build_extract_value(value, 3, "http.secret_request.body")
                    .expect("extract HTTP secret sink response body")
                    .into_pointer_value();
                let response_headers = self
                    .builder
                    .build_extract_value(value, 4, "http.secret_request.headers")
                    .expect("extract HTTP secret sink response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (response_body, response_headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpHead {
                url,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.head.out");
                self.builder
                    .build_call(
                        self.runtime.http_head,
                        &[url.into(), out.into()],
                        "http.head",
                    )
                    .expect("call the HTTP HEAD runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.head.out.load")
                    .expect("load HTTP HEAD outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.head.kind")
                    .expect("extract HTTP result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.head.error")
                    .expect("extract HTTP error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.head.status")
                    .expect("extract HTTP status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.head.body")
                    .expect("extract HTTP body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.head.headers")
                    .expect("extract HTTP response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpQuery {
                url,
                query,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let query = self.operand(*query);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.query.out");
                self.builder
                    .build_call(
                        self.runtime.http_query,
                        &[url.into(), query.into(), out.into()],
                        "http.query",
                    )
                    .expect("call the HTTP QUERY runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.query.out.load")
                    .expect("load HTTP QUERY outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.query.kind")
                    .expect("extract HTTP QUERY result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.query.error")
                    .expect("extract HTTP QUERY error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.query.status")
                    .expect("extract HTTP QUERY status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.query.body")
                    .expect("extract HTTP QUERY body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.query.headers")
                    .expect("extract HTTP response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpQueryWithContentType {
                url,
                query,
                content_type,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let query = self.operand(*query);
                let content_type = self.operand(*content_type);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.query.custom.out");
                self.builder
                    .build_call(
                        self.runtime.http_query_with_content_type,
                        &[url.into(), query.into(), content_type.into(), out.into()],
                        "http.query.custom",
                    )
                    .expect("call the HTTP QUERY media-type runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.query.custom.out.load")
                    .expect("load custom HTTP QUERY outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.query.custom.kind")
                    .expect("extract custom HTTP QUERY result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.query.custom.error")
                    .expect("extract custom HTTP QUERY error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.query.custom.status")
                    .expect("extract custom HTTP QUERY status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.query.custom.body")
                    .expect("extract custom HTTP QUERY body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.query.custom.headers")
                    .expect("extract custom HTTP QUERY response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpPost {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let body = self.operand(*body);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.post.out");
                self.builder
                    .build_call(
                        self.runtime.http_post,
                        &[url.into(), body.into(), out.into()],
                        "http.post",
                    )
                    .expect("call the HTTP POST runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.post.out.load")
                    .expect("load HTTP POST outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.post.kind")
                    .expect("extract HTTP POST result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.post.error")
                    .expect("extract HTTP POST error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.post.status")
                    .expect("extract HTTP POST status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.post.body")
                    .expect("extract HTTP POST body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.post.headers")
                    .expect("extract HTTP response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpPostJson {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let body = self.operand(*body);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.post.out");
                self.builder
                    .build_call(
                        self.runtime.http_post_json,
                        &[url.into(), body.into(), out.into()],
                        "http.post.json",
                    )
                    .expect("call the HTTP POST runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.post.out.load")
                    .expect("load HTTP POST outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.post.kind")
                    .expect("extract HTTP POST result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.post.error")
                    .expect("extract HTTP POST error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.post.status")
                    .expect("extract HTTP POST status")
                    .into_int_value();
                let body = self
                    .builder
                    .build_extract_value(value, 3, "http.post.body")
                    .expect("extract HTTP POST body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.post.headers")
                    .expect("extract HTTP response headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpPostBytes {
                url,
                body,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let body = self.operand(*body);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.post.bytes.out");
                self.builder
                    .build_call(
                        self.runtime.http_post_bytes,
                        &[url.into(), body.into(), out.into()],
                        "http.post.bytes",
                    )
                    .expect("call the HTTP byte POST runtime adapter");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.post.bytes.out.load")
                    .expect("load HTTP byte POST outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.post.bytes.kind")
                    .expect("extract HTTP byte POST result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.post.bytes.error")
                    .expect("extract HTTP byte POST error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.post.bytes.status")
                    .expect("extract HTTP byte POST status")
                    .into_int_value();
                let response_body = self
                    .builder
                    .build_extract_value(value, 3, "http.post.bytes.body")
                    .expect("extract HTTP byte POST body")
                    .into_pointer_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.post.bytes.headers")
                    .expect("extract HTTP byte POST headers")
                    .into_pointer_value();
                let protocol = self
                    .builder
                    .build_extract_value(value, 5, "http.response.protocol")
                    .expect("extract HTTP response protocol")
                    .into_int_value();
                Some(self.build_http_get_result_with_protocol(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    status,
                    (response_body, headers),
                    protocol,
                ))
            }
            ir::InstKind::HttpGetStream {
                url,
                headers,
                enum_id,
                response_class_id,
            } => {
                let url = self.operand(*url);
                let request_headers = self.operand(*headers);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.stream.open.out");
                self.builder
                    .build_call(
                        self.runtime.http_get_stream,
                        &[url.into(), request_headers.into(), out.into()],
                        "http.stream.open",
                    )
                    .expect("open a streaming HTTP GET");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.stream.open.load")
                    .expect("load streaming HTTP GET result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.stream.open.kind")
                    .expect("extract stream-open result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.stream.open.error")
                    .expect("extract stream-open error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.stream.open.status")
                    .expect("extract stream status")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 3, "http.stream.open.handle")
                    .expect("extract stream handle")
                    .into_int_value();
                let headers = self
                    .builder
                    .build_extract_value(value, 4, "http.stream.open.headers")
                    .expect("extract stream headers")
                    .into_pointer_value();
                Some(self.build_http_stream_open_result(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    (status, handle),
                    headers,
                ))
            }
            ir::InstKind::HttpResponseToStream {
                status,
                body,
                headers,
                enum_id,
                response_class_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.response.stream.out");
                self.builder
                    .build_call(
                        self.runtime.http_response_to_stream,
                        &[
                            self.operand(*status).into(),
                            self.operand(*body).into(),
                            self.operand(*headers).into(),
                            out.into(),
                        ],
                        "http.response.stream.open",
                    )
                    .expect("transfer buffered HTTP response body to stream");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.response.stream.load")
                    .expect("load transferred HTTP response stream")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.response.stream.kind")
                    .expect("extract response stream result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.response.stream.error")
                    .expect("extract response stream error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.response.stream.status")
                    .expect("extract response stream status")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 3, "http.response.stream.handle")
                    .expect("extract response stream handle")
                    .into_int_value();
                let response_headers = self
                    .builder
                    .build_extract_value(value, 4, "http.response.stream.headers")
                    .expect("extract response stream headers")
                    .into_pointer_value();
                Some(self.build_http_stream_open_result(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    (status, handle),
                    response_headers,
                ))
            }
            ir::InstKind::HttpRequestStream {
                method,
                url,
                headers,
                callback,
                callback_layout,
                result_callback,
                enum_id,
                response_class_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let captures = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http.upload.captures")
                    .expect("extract HTTP upload captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http.upload.capture.root")
                    .expect("allocate HTTP upload capture root");
                self.builder
                    .build_store(capture_root, captures)
                    .expect("store HTTP upload capture root");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http.upload.roots")
                    .expect("allocate HTTP upload roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http.upload.root.entry",
                    )
                }
                .expect("index HTTP upload root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register HTTP upload capture root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http.upload.push.roots",
                    )
                    .expect("root HTTP upload captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http.upload.context")
                    .expect("allocate HTTP upload callback context");
                self.builder
                    .build_store(context, callable)
                    .expect("store HTTP upload callback context");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.upload.out");
                let thunk = self.http_upload_producer_thunk(*callback_layout, *result_callback);
                self.builder
                    .build_call(
                        self.runtime.http_request_stream,
                        &[
                            self.operand(*method).into(),
                            self.operand(*url).into(),
                            self.operand(*headers).into(),
                            thunk.as_global_value().as_pointer_value().into(),
                            context.into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*result_callback), false)
                                .into(),
                            out.into(),
                        ],
                        "http.upload.open",
                    )
                    .expect("stream an HTTP request body");
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http.upload.pop.roots")
                    .expect("release HTTP upload captures");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.upload.out.load")
                    .expect("load HTTP upload result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.upload.kind")
                    .expect("extract HTTP upload result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.upload.error")
                    .expect("extract HTTP upload error")
                    .into_int_value();
                let status = self
                    .builder
                    .build_extract_value(value, 2, "http.upload.status")
                    .expect("extract HTTP upload response status")
                    .into_int_value();
                let handle = self
                    .builder
                    .build_extract_value(value, 3, "http.upload.handle")
                    .expect("extract HTTP upload response handle")
                    .into_int_value();
                let response_headers = self
                    .builder
                    .build_extract_value(value, 4, "http.upload.headers")
                    .expect("extract HTTP upload response headers")
                    .into_pointer_value();
                Some(self.build_http_stream_open_result(
                    *enum_id,
                    *response_class_id,
                    kind,
                    error,
                    (status, handle),
                    response_headers,
                ))
            }
            ir::InstKind::HttpRegisterServerResponseProducer {
                callback,
                callback_layout,
            } => {
                let callable = self.operand(*callback).into_struct_value();
                let captures = self
                    .builder
                    .build_extract_value(
                        callable,
                        CALLABLE_CAPTURE_FIELD,
                        "http.server.producer.captures",
                    )
                    .expect("extract HTTP server producer captures")
                    .into_pointer_value();
                let context_type = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_type, "http.server.producer.context")
                    .expect("allocate HTTP server producer context");
                self.builder
                    .build_store(context, callable)
                    .expect("store HTTP server producer callable");
                let thunk = self.http_upload_producer_thunk(*callback_layout, false);
                Some(
                    self.builder
                        .build_call(
                            self.runtime.http_server_producer_register,
                            &[
                                thunk.as_global_value().as_pointer_value().into(),
                                context.into(),
                                captures.into(),
                            ],
                            "http.server.producer.register",
                        )
                        .expect("register HTTP server response producer")
                        .try_as_basic_value()
                        .basic()
                        .expect("producer registration returns a handle"),
                )
            }
            ir::InstKind::HttpResponseReadChunk {
                handle,
                max_bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle);
                let max_bytes = self.operand(*max_bytes);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.stream.read.out");
                self.builder
                    .build_call(
                        self.runtime.http_stream_read_chunk,
                        &[handle.into(), max_bytes.into(), out.into()],
                        "http.stream.read",
                    )
                    .expect("read an HTTP response chunk");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.stream.read.load")
                    .expect("load HTTP response chunk")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.stream.read.kind")
                    .expect("extract stream-read tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.stream.read.error")
                    .expect("extract stream-read error")
                    .into_int_value();
                let bytes = self
                    .builder
                    .build_extract_value(value, 2, "http.stream.read.bytes")
                    .expect("extract response chunk")
                    .into_pointer_value();
                Some(self.build_http_stream_chunk(*enum_id, kind, error, bytes))
            }
            ir::InstKind::HttpResponseReadAll { handle, enum_id } => {
                let handle = self.operand(*handle);
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.stream.read_all.out");
                self.builder
                    .build_call(
                        self.runtime.http_stream_read_all,
                        &[handle.into(), out.into()],
                        "http.stream.read_all",
                    )
                    .expect("consume an HTTP response stream");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.stream.read_all.load")
                    .expect("load consumed HTTP response body")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.stream.read_all.kind")
                    .expect("extract stream read-all tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.stream.read_all.error")
                    .expect("extract stream read-all error")
                    .into_int_value();
                let bytes = self
                    .builder
                    .build_extract_value(value, 2, "http.stream.read_all.bytes")
                    .expect("extract consumed response bytes")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, bytes, 0, 1))
            }
            ir::InstKind::HttpResponseReadSseEvent { handle, enum_id } => {
                let handle = self.operand(*handle);
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.sse.next.out");
                self.builder
                    .build_call(
                        self.runtime.http_stream_sse_next,
                        &[handle.into(), out.into()],
                        "http.sse.next",
                    )
                    .expect("read the next HTTP server-sent event");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.sse.next.load")
                    .expect("load HTTP SSE event fields")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.sse.next.kind")
                    .expect("extract HTTP SSE result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.sse.next.error")
                    .expect("extract HTTP SSE error")
                    .into_int_value();
                let fields = self
                    .builder
                    .build_extract_value(value, 2, "http.sse.next.fields")
                    .expect("extract HTTP SSE event fields")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, fields, 0, 1))
            }
            ir::InstKind::HttpResponseSseHeartbeatCount { handle } => {
                let handle = self.operand(*handle);
                Some(
                    self.builder
                        .build_call(
                            self.runtime.http_stream_sse_heartbeat_count,
                            &[handle.into()],
                            "http.sse.heartbeat_count",
                        )
                        .expect("read the SSE heartbeat count")
                        .try_as_basic_value()
                        .basic()
                        .expect("SSE heartbeat count returns UInt64"),
                )
            }
            ir::InstKind::HttpResponseStreamClose { handle } => {
                let handle = self.operand(*handle);
                let closed = self
                    .builder
                    .build_call(
                        self.runtime.http_stream_close,
                        &[handle.into()],
                        "http.stream.close",
                    )
                    .expect("close an HTTP response stream")
                    .try_as_basic_value()
                    .basic()
                    .expect("HTTP response close returns Boolean");
                Some(closed)
            }
            ir::InstKind::HttpUrlNormalize { value, enum_id } => {
                let value = self.operand(*value);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.url.out");
                self.builder
                    .build_call(
                        self.runtime.http_url_normalize,
                        &[value.into(), out.into()],
                        "http.url.normalize",
                    )
                    .expect("call the HTTP URL normalizer");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.url.out.load")
                    .expect("load normalized HTTP URL outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.url.kind")
                    .expect("extract URL result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.url.error")
                    .expect("extract URL error")
                    .into_int_value();
                let normalized = self
                    .builder
                    .build_extract_value(value, 2, "http.url.normalized")
                    .expect("extract normalized URL")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, normalized, 0, 1))
            }
            ir::InstKind::HttpUrlResolve {
                base,
                reference,
                enum_id,
            } => {
                let base = self.operand(*base);
                let reference = self.operand(*reference);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.url.resolve.out");
                self.builder
                    .build_call(
                        self.runtime.http_url_resolve,
                        &[base.into(), reference.into(), out.into()],
                        "http.url.resolve",
                    )
                    .expect("call the HTTP URL resolver");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.url.resolve.load")
                    .expect("load resolved HTTP URL outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.url.resolve.kind")
                    .expect("extract URL resolution tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.url.resolve.error")
                    .expect("extract URL resolution error")
                    .into_int_value();
                let normalized = self
                    .builder
                    .build_extract_value(value, 2, "http.url.resolve.value")
                    .expect("extract resolved URL")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, normalized, 0, 1))
            }
            ir::InstKind::HttpResponseText { body, enum_id } => {
                let body = self.operand(*body);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.text.out");
                self.builder
                    .build_call(
                        self.runtime.http_response_text,
                        &[body.into(), out.into()],
                        "http.response.text",
                    )
                    .expect("call the HTTP response text decoder");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.text.out.load")
                    .expect("load HTTP response text result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.text.kind")
                    .expect("extract HTTP text result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.text.error")
                    .expect("extract HTTP text error")
                    .into_int_value();
                let text = self
                    .builder
                    .build_extract_value(value, 2, "http.text.value")
                    .expect("extract HTTP response text")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, text, 0, 1))
            }
            ir::InstKind::HttpResponseHeaderValues {
                headers,
                name,
                enum_id,
            } => {
                let headers = self.operand(*headers);
                let name = self.operand(*name);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.header.values.out");
                self.builder
                    .build_call(
                        self.runtime.http_response_header_values,
                        &[headers.into(), name.into(), out.into()],
                        "http.response.header_values",
                    )
                    .expect("call the HTTP response header lookup");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.header.values.out.load")
                    .expect("load HTTP header lookup result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.header.values.kind")
                    .expect("extract HTTP header result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.header.values.error")
                    .expect("extract HTTP header error")
                    .into_int_value();
                let values = self
                    .builder
                    .build_extract_value(value, 2, "http.header.values.list")
                    .expect("extract HTTP header values list")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, values, 0, 1))
            }
            ir::InstKind::HttpFormParse {
                body,
                content_type,
                enum_id,
            } => {
                let body = self.operand(*body);
                let content_type = self.operand(*content_type);
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.ptr_type(AddressSpace::default()).into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.form.parse.out");
                self.builder
                    .build_call(
                        self.runtime.http_form_parse,
                        &[body.into(), content_type.into(), out.into()],
                        "http.form.parse",
                    )
                    .expect("parse HTTP form data");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.form.parse.out.load")
                    .expect("load HTTP form parse outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.form.parse.kind")
                    .expect("extract HTTP form result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.form.parse.error")
                    .expect("extract HTTP form error")
                    .into_int_value();
                let fields = self
                    .builder
                    .build_extract_value(value, 2, "http.form.parse.fields")
                    .expect("extract parsed HTTP form fields")
                    .into_pointer_value();
                Some(self.build_tagged_outcome(*enum_id, kind, error, fields, 0, 1))
            }
            ir::InstKind::HttpMultipartParse {
                body,
                content_type,
                enum_id,
                data_class_id,
            } => {
                let body = self.operand(*body);
                let content_type = self.operand(*content_type);
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                        ptr.into(),
                        ptr.into(),
                        ptr.into(),
                        ptr.into(),
                        ptr.into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.multipart.parse.out");
                self.builder
                    .build_call(
                        self.runtime.http_multipart_parse,
                        &[body.into(), content_type.into(), out.into()],
                        "http.multipart.parse",
                    )
                    .expect("parse HTTP multipart data");
                let result = self
                    .builder
                    .build_load(out_ty, out, "http.multipart.parse.out.load")
                    .expect("load HTTP multipart outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(result, 0, "http.multipart.parse.kind")
                    .expect("extract HTTP multipart result tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(result, 1, "http.multipart.parse.error")
                    .expect("extract HTTP multipart error")
                    .into_int_value();
                let names = self
                    .builder
                    .build_extract_value(result, 2, "http.multipart.parse.names")
                    .expect("extract multipart names")
                    .into_pointer_value();
                let filenames = self
                    .builder
                    .build_extract_value(result, 3, "http.multipart.parse.filenames")
                    .expect("extract multipart filenames")
                    .into_pointer_value();
                let filename_present = self
                    .builder
                    .build_extract_value(result, 4, "http.multipart.parse.filename_present")
                    .expect("extract multipart filename-presence list")
                    .into_pointer_value();
                let content_types = self
                    .builder
                    .build_extract_value(result, 5, "http.multipart.parse.content_types")
                    .expect("extract multipart content types")
                    .into_pointer_value();
                let content_type_present = self
                    .builder
                    .build_extract_value(result, 6, "http.multipart.parse.content_type_present")
                    .expect("extract multipart content-type-presence list")
                    .into_pointer_value();
                let bodies = self
                    .builder
                    .build_extract_value(result, 7, "http.multipart.parse.bodies")
                    .expect("extract multipart bodies")
                    .into_pointer_value();
                let upload_handles = self
                    .builder
                    .build_extract_value(result, 8, "http.multipart.parse.upload_handles")
                    .expect("extract managed upload handles")
                    .into_pointer_value();

                let roots_type = self.context.struct_type(&[ptr.into(); 7], false);
                let roots = self.aligned_alloca(roots_type.into(), "http.multipart.roots");
                for (index, value) in [
                    names,
                    filenames,
                    filename_present,
                    content_types,
                    content_type_present,
                    bodies,
                    upload_handles,
                ]
                .into_iter()
                .enumerate()
                {
                    let slot = self
                        .builder
                        .build_struct_gep(
                            roots_type,
                            roots,
                            index as u32,
                            "http.multipart.root.slot",
                        )
                        .expect("address multipart managed root slot");
                    self.builder
                        .build_store(slot, value)
                        .expect("root multipart managed list");
                }
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(7, false).into(),
                        ],
                        "http.multipart.push.roots",
                    )
                    .expect("root multipart result lists");

                let layout = &self.module.objects[*data_class_id as usize];
                debug_assert_eq!(layout.fields.len(), 7);
                let object_type = object_struct(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let object = self
                    .builder
                    .build_call(
                        self.runtime.alloc,
                        &[
                            object_type
                                .size_of()
                                .expect("multipart data object size")
                                .into(),
                            object_type.get_alignment().into(),
                        ],
                        "http.multipart.object",
                    )
                    .expect("allocate HTTPMultipartData")
                    .try_as_basic_value()
                    .basic()
                    .expect("runtime allocator returns a pointer")
                    .into_pointer_value();
                self.builder
                    .build_store(object, self.descriptors[*data_class_id as usize])
                    .expect("write HTTPMultipartData descriptor");
                for (index, value) in [
                    names,
                    filenames,
                    filename_present,
                    content_types,
                    content_type_present,
                    bodies,
                    upload_handles,
                ]
                .into_iter()
                .enumerate()
                {
                    let field = self
                        .builder
                        .build_struct_gep(
                            object_type,
                            object,
                            OBJECT_HEADER_FIELDS + index as u32,
                            "http.multipart.field",
                        )
                        .expect("address multipart data field");
                    self.builder
                        .build_store(field, value)
                        .expect("store multipart data field");
                }
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http.multipart.pop.roots")
                    .expect("pop multipart managed roots");
                Some(self.build_tagged_outcome(*enum_id, kind, error, object, 0, 1))
            }
            ir::InstKind::HttpUploadRead {
                handle,
                offset,
                max_bytes,
                enum_id,
            } => {
                let handle = self.operand(*handle).into_int_value();
                let offset = self.operand(*offset).into_int_value();
                let max_bytes = self.operand(*max_bytes).into_int_value();
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.upload.read.out");
                self.builder
                    .build_call(
                        self.runtime.http_upload_read,
                        &[handle.into(), offset.into(), max_bytes.into(), out.into()],
                        "http.upload.read",
                    )
                    .expect("read a managed multipart upload range");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.upload.read.value")
                    .expect("load multipart upload read result")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.upload.read.kind")
                    .expect("extract upload read tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.upload.read.error")
                    .expect("extract upload read error")
                    .into_int_value();
                let bytes = self
                    .builder
                    .build_extract_value(value, 2, "http.upload.read.bytes")
                    .expect("extract upload bytes")
                    .into_pointer_value();
                Some(self.build_http_stream_chunk(*enum_id, kind, error, bytes))
            }
            ir::InstKind::HttpUploadClose { handle } => {
                let handle = self.operand(*handle).into_int_value();
                Some(
                    self.builder
                        .build_call(
                            self.runtime.http_upload_close,
                            &[handle.into()],
                            "http.upload.close",
                        )
                        .expect("close a managed multipart upload")
                        .try_as_basic_value()
                        .basic()
                        .expect("HTTP upload close returns Boolean"),
                )
            }
            ir::InstKind::HttpServerControlCreate => Some(
                self.builder
                    .build_call(
                        self.runtime.http_server_control_create,
                        &[],
                        "http.server.control.create",
                    )
                    .expect("create HTTP server shutdown control")
                    .try_as_basic_value()
                    .basic()
                    .expect("HTTP server control creation returns UInt64"),
            ),
            ir::InstKind::HttpServerControlRequestShutdown { handle } => {
                let result = self
                    .builder
                    .build_call(
                        self.runtime.http_server_control_request_shutdown,
                        &[self.operand(*handle).into()],
                        "http.server.control.shutdown",
                    )
                    .expect("request HTTP server shutdown")
                    .try_as_basic_value()
                    .basic()
                    .expect("HTTP server shutdown request returns i32")
                    .into_int_value();
                Some(
                    self.builder
                        .build_int_compare(
                            IntPredicate::NE,
                            result,
                            self.context.i32_type().const_zero(),
                            "http.server.control.shutdown.accepted",
                        )
                        .expect("convert HTTP server shutdown result to Boolean")
                        .into(),
                )
            }
            ir::InstKind::HttpRouteMatch { pattern, path } => {
                let pattern = self.operand(*pattern);
                let path = self.operand(*path);
                let ptr = self.context.ptr_type(AddressSpace::default());
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        ptr.into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.route.match.out");
                self.builder
                    .build_call(
                        self.runtime.http_route_match,
                        &[pattern.into(), path.into(), out.into()],
                        "http.route.match",
                    )
                    .expect("match HTTP route pattern");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.route.match.out.load")
                    .expect("load HTTP route match outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.route.match.kind")
                    .expect("extract HTTP route match tag")
                    .into_int_value();
                let parameters = self
                    .builder
                    .build_extract_value(value, 2, "http.route.match.parameters")
                    .expect("extract HTTP route parameters")
                    .into_pointer_value();
                let is_success = self
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        kind,
                        self.context.i32_type().const_zero(),
                        "http.route.match.succeeded",
                    )
                    .expect("compare HTTP route match outcome");
                let empty = self
                    .builder
                    .build_call(
                        self.runtime.list_new,
                        &[
                            self.context.i64_type().const_int(8, false).into(),
                            self.context.i64_type().const_int(8, false).into(),
                            self.context.bool_type().const_int(1, false).into(),
                        ],
                        "http.route.match.empty",
                    )
                    .expect("allocate empty HTTP route match list")
                    .try_as_basic_value()
                    .basic()
                    .expect("list_new returns a list")
                    .into_pointer_value();
                Some(
                    self.builder
                        .build_select(is_success, parameters, empty, "http.route.match.result")
                        .expect("select HTTP route match list"),
                )
            }
            ir::InstKind::HttpServerServeOnce {
                stream_handle,
                secure,
                max_requests,
                require_http2,
                callback,
                callback_layout,
                request_class_id,
                response_class_id,
                enum_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let capture_value = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http.server.capture")
                    .expect("extract HTTP handler captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http.server.capture.root")
                    .expect("allocate capture root");
                self.builder
                    .build_store(capture_root, capture_value)
                    .expect("store capture root");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http.server.roots")
                    .expect("allocate server roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http.server.root.entry",
                    )
                }
                .expect("index capture root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register capture root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http.server.push.roots",
                    )
                    .expect("root HTTP handler captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http.server.context")
                    .expect("allocate HTTP callback context");
                self.builder
                    .build_store(context, callable)
                    .expect("store HTTP callback context");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.server.out");
                let thunk = self.http_server_handler_thunk(
                    *callback_layout,
                    *request_class_id,
                    *response_class_id,
                );
                let thunk_ptr = thunk.as_global_value().as_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.http_server_serve_once,
                        &[
                            self.operand(*stream_handle).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*secure), false)
                                .into(),
                            self.operand(*max_requests).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*require_http2), false)
                                .into(),
                            thunk_ptr.into(),
                            context.into(),
                            out.into(),
                        ],
                        "http.server.serve",
                    )
                    .expect("serve HTTP request");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.server.out.value")
                    .expect("load HTTP server outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.server.kind")
                    .expect("extract HTTP server tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.server.error")
                    .expect("extract HTTP server error")
                    .into_int_value();
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http.server.pop.roots")
                    .expect("pop HTTP handler roots");
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }
            ir::InstKind::HttpServerServeHttp3 {
                endpoint,
                certificate_chain_der,
                private_key_der,
                max_requests,
                max_connections,
                control_handle,
                drain_timeout_ms,
                max_handshakes,
                max_handler_tasks,
                max_streams_per_connection,
                max_queued_response_bytes,
                callback,
                callback_layout,
                request_class_id,
                response_class_id,
                enum_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let capture_value = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http3.server.capture")
                    .expect("extract HTTP/3 handler captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http3.server.capture.root")
                    .expect("allocate HTTP/3 handler root");
                self.builder
                    .build_store(capture_root, capture_value)
                    .expect("store HTTP/3 handler root");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http3.server.roots")
                    .expect("allocate HTTP/3 server roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http3.server.root.entry",
                    )
                }
                .expect("index HTTP/3 handler root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register HTTP/3 handler root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http3.server.push.roots",
                    )
                    .expect("root HTTP/3 handler captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http3.server.context")
                    .expect("allocate HTTP/3 callback context");
                self.builder
                    .build_store(context, callable)
                    .expect("store HTTP/3 callback context");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http3.server.out");
                let thunk = self.http_server_handler_thunk(
                    *callback_layout,
                    *request_class_id,
                    *response_class_id,
                );
                self.builder
                    .build_call(
                        self.runtime.http_server_serve_http3,
                        &[
                            self.operand(*endpoint).into(),
                            self.operand(*certificate_chain_der).into(),
                            self.operand(*private_key_der).into(),
                            self.operand(*max_requests).into(),
                            self.operand(*max_connections).into(),
                            self.operand(*control_handle).into(),
                            self.operand(*drain_timeout_ms).into(),
                            self.operand(*max_handshakes).into(),
                            self.operand(*max_handler_tasks).into(),
                            self.operand(*max_streams_per_connection).into(),
                            self.operand(*max_queued_response_bytes).into(),
                            thunk.as_global_value().as_pointer_value().into(),
                            context.into(),
                            out.into(),
                        ],
                        "http3.server.serve",
                    )
                    .expect("serve HTTP/3 requests");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http3.server.out.value")
                    .expect("load HTTP/3 server outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http3.server.kind")
                    .expect("extract HTTP/3 server tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http3.server.error")
                    .expect("extract HTTP/3 server error")
                    .into_int_value();
                let count = self
                    .builder
                    .build_extract_value(value, 2, "http3.server.count")
                    .expect("extract HTTP/3 request count")
                    .into_int_value();
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http3.server.pop.roots")
                    .expect("pop HTTP/3 handler roots");
                Some(self.build_u64_tagged_outcome(*enum_id, kind, error, count))
            }
            ir::InstKind::HttpServerWebSocketServeOnce {
                stream_handle,
                secure,
                path,
                allowed_origins,
                allow_missing_origin,
                protocols,
                max_messages,
                callback,
                callback_layout,
                websocket_class_id,
                enum_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let capture_value = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http.websocket.capture")
                    .expect("extract WebSocket captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http.websocket.capture.root")
                    .expect("allocate WebSocket capture root");
                self.builder
                    .build_store(capture_root, capture_value)
                    .expect("store WebSocket capture root");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http.websocket.roots")
                    .expect("allocate WebSocket roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http.websocket.root.entry",
                    )
                }
                .expect("index WebSocket capture root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register WebSocket capture root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http.websocket.push.roots",
                    )
                    .expect("root WebSocket handler captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http.websocket.context")
                    .expect("allocate WebSocket callback context");
                self.builder
                    .build_store(context, callable)
                    .expect("store WebSocket callback context");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http.websocket.server.out");
                let thunk =
                    self.http_websocket_server_handler_thunk(*callback_layout, *websocket_class_id);
                let thunk_ptr = thunk.as_global_value().as_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.http_server_websocket_serve_once,
                        &[
                            self.operand(*stream_handle).into(),
                            self.context
                                .i32_type()
                                .const_int(u64::from(*secure), false)
                                .into(),
                            self.operand(*path).into(),
                            self.operand(*allowed_origins).into(),
                            self.builder
                                .build_int_z_extend(
                                    self.operand(*allow_missing_origin).into_int_value(),
                                    self.context.i32_type(),
                                    "http.websocket.allow.missing",
                                )
                                .expect("extend origin flag")
                                .into(),
                            self.operand(*protocols).into(),
                            self.operand(*max_messages).into(),
                            thunk_ptr.into(),
                            context.into(),
                            out.into(),
                        ],
                        "http.websocket.server.serve",
                    )
                    .expect("serve HTTP WebSocket session");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http.websocket.server.out.value")
                    .expect("load WebSocket server outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http.websocket.server.kind")
                    .expect("extract WebSocket server tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http.websocket.server.error")
                    .expect("extract WebSocket server error")
                    .into_int_value();
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http.websocket.pop.roots")
                    .expect("pop WebSocket handler roots");
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            ir::InstKind::HttpServerWebSocketServeHttp3Once {
                endpoint,
                certificate_chain_der,
                private_key_der,
                path,
                allowed_origins,
                allow_missing_origin,
                protocols,
                max_messages,
                idle_timeout_ms,
                callback,
                callback_layout,
                websocket_class_id,
                enum_id,
            } => {
                let ptr = self.context.ptr_type(AddressSpace::default());
                let callable = self.operand(*callback).into_struct_value();
                let capture_value = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "http3.ws.capture")
                    .expect("extract HTTP/3 WebSocket captures")
                    .into_pointer_value();
                let capture_root = self
                    .builder
                    .build_alloca(ptr, "http3.ws.capture.root")
                    .expect("allocate HTTP/3 WebSocket capture root");
                self.builder
                    .build_store(capture_root, capture_value)
                    .expect("store HTTP/3 WebSocket capture root");
                let roots_ty = ptr.array_type(1);
                let roots = self
                    .builder
                    .build_alloca(roots_ty, "http3.ws.roots")
                    .expect("allocate HTTP/3 WebSocket roots");
                let root_entry = unsafe {
                    self.builder.build_gep(
                        roots_ty,
                        roots,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "http3.ws.root.entry",
                    )
                }
                .expect("index HTTP/3 WebSocket capture root");
                self.builder
                    .build_store(root_entry, capture_root)
                    .expect("register HTTP/3 WebSocket capture root");
                self.builder
                    .build_call(
                        self.runtime.push_frame,
                        &[
                            roots.into(),
                            self.context.i64_type().const_int(1, false).into(),
                        ],
                        "http3.ws.push.roots",
                    )
                    .expect("root HTTP/3 WebSocket handler captures");

                let context_ty = self
                    .context
                    .struct_type(&[callable.get_type().into()], false);
                let context = self
                    .builder
                    .build_alloca(context_ty, "http3.ws.context")
                    .expect("allocate HTTP/3 WebSocket callback context");
                self.builder
                    .build_store(context, callable)
                    .expect("store HTTP/3 WebSocket callback context");
                let out_ty = self.context.struct_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i32_type().into(),
                    ],
                    false,
                );
                let out = self.aligned_alloca(out_ty.into(), "http3.ws.server.out");
                let thunk =
                    self.http_websocket_server_handler_thunk(*callback_layout, *websocket_class_id);
                self.builder
                    .build_call(
                        self.runtime.http_server_websocket_serve_http3_once,
                        &[
                            self.operand(*endpoint).into(),
                            self.operand(*certificate_chain_der).into(),
                            self.operand(*private_key_der).into(),
                            self.operand(*path).into(),
                            self.operand(*allowed_origins).into(),
                            self.builder
                                .build_int_z_extend(
                                    self.operand(*allow_missing_origin).into_int_value(),
                                    self.context.i32_type(),
                                    "http3.ws.allow.missing",
                                )
                                .expect("extend origin flag")
                                .into(),
                            self.operand(*protocols).into(),
                            self.operand(*max_messages).into(),
                            self.operand(*idle_timeout_ms).into(),
                            thunk.as_global_value().as_pointer_value().into(),
                            context.into(),
                            out.into(),
                        ],
                        "http3.ws.server.serve",
                    )
                    .expect("serve HTTP/3 WebSocket session");
                let value = self
                    .builder
                    .build_load(out_ty, out, "http3.ws.server.out.value")
                    .expect("load HTTP/3 WebSocket server outcome")
                    .into_struct_value();
                let kind = self
                    .builder
                    .build_extract_value(value, 0, "http3.ws.server.kind")
                    .expect("extract HTTP/3 WebSocket server tag")
                    .into_int_value();
                let error = self
                    .builder
                    .build_extract_value(value, 1, "http3.ws.server.error")
                    .expect("extract HTTP/3 WebSocket server error")
                    .into_int_value();
                self.builder
                    .build_call(self.runtime.pop_frame, &[], "http3.ws.pop.roots")
                    .expect("pop HTTP/3 WebSocket handler roots");
                Some(self.build_void_tagged_outcome(*enum_id, kind, error))
            }

            // A nullable value is `{ i1 present, T value }`. The absent form's
            // payload slot is zeroed, not left undefined: ordinary Zirk-level
            // code never reads it without checking the flag first (the
            // verifier enforces that), but the collector's own `mark_object`
            // and `Clone`'s deep-clone traversal (roadmap Phase 4e,
            // `fase-4e-clone`) both read a nullable-*reference*-typed field's
            // payload directly off the object's own byte layout, with no flag
            // check at all — `gc_field_offsets`' table carries only byte
            // offsets, not "and here is where its own present flag lives" —
            // relying on exactly this invariant: an absent reference's
            // payload pointer is reliably null, the same way a `Base::Class`/
            // `Base::Contract` field that was simply never written is (every
            // `zirk_rt_alloc` allocation is zeroed). An undefined payload
            // (this arm's previous behavior) breaks that silently: LLVM is
            // free to lower `undef` to any bit pattern, including a
            // non-null-looking one, which crashes `Clone`'s own recursion the
            // moment it treats that garbage as a pointer to dereference —
            // found while building `fase-4e-clone`'s own end-to-end tests,
            // not a `Clone`-specific requirement: `mark_object` has carried
            // this exact same assumption, and therefore this exact same
            // latent bug, since `fase-4e-colector-mark-sweep`.
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
                Some(ty.const_zero().into()).map(|value: BasicValueEnum| {
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
            // receiver is supported by this slice — a record's
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

            // `pointer.as_slice(length)`/`.as_slice_mut(length)`'s own
            // runtime validation (roadmap Phase 4e, `fase-4e-native-slice`,
            // design D3/D5): `element`'s size/alignment come from LLVM's
            // own data layout via its LLVM type, the same way
            // `InstKind::Alloc`/`WeakFrom` already derive an object's own
            // size from its LLVM struct type rather than a hand-written
            // table. `known_length` becomes `-1` (opaque provenance,
            // trusted) when `None`.
            ir::InstKind::NativeSliceValidate {
                pointer,
                length,
                element,
                known_length,
            } => {
                let elem_ty = self
                    .llvm_type(*element)
                    .expect("a NativeSlice/NativeSliceMut element is never Void");
                let elem_size = elem_ty.size_of().expect("a sized element type");
                let elem_align_bits = elem_ty.get_alignment();
                let elem_align = self
                    .builder
                    .build_int_z_extend(elem_align_bits, self.context.i64_type(), "elem_align")
                    .expect("widen the element's own alignment to i64");
                let known_length_value = self
                    .context
                    .i64_type()
                    .const_int(known_length.map(|n| n as i64).unwrap_or(-1) as u64, true);
                let call = self
                    .builder
                    .build_call(
                        self.runtime.native_slice_validate,
                        &[
                            self.operand(*pointer).into(),
                            self.operand(*length).into(),
                            elem_size.into(),
                            elem_align.into(),
                            known_length_value.into(),
                        ],
                        "native_slice_validate",
                    )
                    .expect("call the native slice validation runtime helper")
                    .try_as_basic_value()
                    .basic()
                    .expect("zirk_rt_native_slice_validate returns a bool");
                Some(call)
            }

            // Packs an already-validated `(pointer, length)` pair (design
            // D2) — a plain struct build, no allocation.
            ir::InstKind::NativeSliceValue { pointer, length } => {
                let struct_type = native_slice_struct(self.context);
                let mut built = struct_type.get_undef();
                built = self
                    .builder
                    .build_insert_value(
                        built,
                        self.operand(*pointer),
                        NATIVE_SLICE_POINTER_FIELD,
                        "native_slice_pointer",
                    )
                    .expect("insert the native slice's pointer")
                    .into_struct_value();
                built = self
                    .builder
                    .build_insert_value(
                        built,
                        self.operand(*length),
                        NATIVE_SLICE_LENGTH_FIELD,
                        "native_slice_length",
                    )
                    .expect("insert the native slice's length")
                    .into_struct_value();
                Some(built.into())
            }

            // `.length` (roadmap Phase 4e, `fase-4e-native-slice`): the
            // length word out of the `(pointer, length)` representation.
            ir::InstKind::NativeSliceLength(receiver) => {
                let receiver = self.operand(*receiver).into_struct_value();
                Some(
                    self.builder
                        .build_extract_value(
                            receiver,
                            NATIVE_SLICE_LENGTH_FIELD,
                            "native_slice_length",
                        )
                        .expect("extract the native slice's length"),
                )
            }

            // `view[i]` read (design D5): a plain load at `pointer + i *
            // sizeof(T)` — the bounds check already ran in `zirk-ir`
            // (`Self::lower_native_slice_bounds_check`), so this
            // instruction performs none of its own.
            ir::InstKind::NativeSliceLoad { receiver, index } => {
                let element_ty = self
                    .llvm_type(instruction.ty)
                    .expect("a NativeSlice/NativeSliceMut element is never Void");
                let receiver_struct = self.operand(*receiver).into_struct_value();
                let base = self
                    .builder
                    .build_extract_value(
                        receiver_struct,
                        NATIVE_SLICE_POINTER_FIELD,
                        "native_slice_ptr",
                    )
                    .expect("extract the native slice's pointer")
                    .into_pointer_value();
                let element = unsafe {
                    self.builder
                        .build_gep(
                            element_ty,
                            base,
                            &[self.operand(*index).into_int_value()],
                            "native_slice_index",
                        )
                        .expect("index a native slice")
                };
                Some(
                    self.builder
                        .build_load(element_ty, element, "native_slice_load")
                        .expect("native slice load"),
                )
            }

            // `view[i] = value` write (design D5): the write counterpart of
            // the load just above — only ever emitted against a
            // `NativeSliceMut<T>` receiver.
            ir::InstKind::NativeSliceStore {
                receiver,
                index,
                value,
            } => {
                let value_operand = self.operand(*value);
                let element_ty = value_operand.get_type();
                let receiver_struct = self.operand(*receiver).into_struct_value();
                let base = self
                    .builder
                    .build_extract_value(
                        receiver_struct,
                        NATIVE_SLICE_POINTER_FIELD,
                        "native_slice_ptr",
                    )
                    .expect("extract the native slice's pointer")
                    .into_pointer_value();
                let element = unsafe {
                    self.builder
                        .build_gep(
                            element_ty,
                            base,
                            &[self.operand(*index).into_int_value()],
                            "native_slice_index",
                        )
                        .expect("index a native slice")
                };
                self.builder
                    .build_store(element, value_operand)
                    .expect("native slice store");
                None
            }

            // `Array<T>(capacity)` / `List<T>()` allocation.
            ir::InstKind::ArrayNew {
                element_id,
                capacity,
            } => {
                let element = self.module.array_types[*element_id as usize];
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let result_id = instruction.result.expect("ArrayNew produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.array.offsets.{result_id}"));
                let capacity = self.operand(*capacity).into_int_value();
                let call = self
                    .builder
                    .build_call(
                        self.runtime.array_new_with_layout,
                        &[
                            capacity.into(),
                            elem_size.into(),
                            elem_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "array",
                    )
                    .expect("array new");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::ListNew { element_id } => {
                let element = self.module.list_types[*element_id as usize];
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let result_id = instruction.result.expect("ListNew produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.list.offsets.{result_id}"));
                let call = self
                    .builder
                    .build_call(
                        self.runtime.list_new_with_layout,
                        &[
                            elem_size.into(),
                            elem_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "list",
                    )
                    .expect("list new");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::DequeNew { element_id } => {
                let element = self.module.list_types[*element_id as usize];
                let elem_size = self.byte_size_of(element);
                let result_id = instruction.result.expect("DequeNew produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.deque.offsets.{result_id}"));
                let call = self
                    .builder
                    .build_call(
                        self.runtime.deque_new_with_layout,
                        &[elem_size.into(), offsets.into(), count.into()],
                        "deque",
                    )
                    .expect("deque new");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::PriorityQueueNew { element_id, order } => {
                let element = self.module.list_types[*element_id as usize];
                let elem_size = self.byte_size_of(element);
                let result_id = instruction
                    .result
                    .expect("PriorityQueueNew produces a value")
                    .0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.priority_queue.offsets.{result_id}"));
                let call = self
                    .builder
                    .build_call(
                        self.runtime.priority_queue_new_with_layout,
                        &[
                            elem_size.into(),
                            offsets.into(),
                            count.into(),
                            self.context
                                .i32_type()
                                .const_int(*order as u64, false)
                                .into(),
                        ],
                        "priority_queue",
                    )
                    .expect("priority queue new");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::MapNew { map_id } => {
                let (key_ty, _) = self.module.map_types[*map_id as usize];
                let text_key = matches!(
                    key_ty,
                    ir::IrType::String
                        | ir::IrType::Char
                        | ir::IrType::Nullable(ir::Nullable::String | ir::Nullable::Char)
                );
                let string_content_keys = self.bool_const(text_key);
                let (_, value_ty) = self.module.map_types[*map_id as usize];
                let offsets_for = |ty: ir::IrType, suffix: &str| {
                    let paths = gc_reference_paths_of(self.module, ty);
                    let offsets: Vec<_> = paths
                        .iter()
                        .map(|path| {
                            if path.is_empty() {
                                self.context.i64_type().const_zero()
                            } else {
                                const_field_offset(
                                    self.context,
                                    self.llvm_type(ty)
                                        .expect("map entry type has an LLVM layout")
                                        .into_struct_type(),
                                    path,
                                )
                            }
                        })
                        .collect();
                    if offsets.is_empty() {
                        (
                            self.context
                                .ptr_type(inkwell::AddressSpace::default())
                                .const_null(),
                            self.context.i64_type().const_zero(),
                        )
                    } else {
                        let values = self.context.i64_type().const_array(&offsets);
                        let result_id = instruction.result.expect("MapNew produces a value").0;
                        let global = self.llvm.add_global(
                            values.get_type(),
                            None,
                            &format!("zk.map.{suffix}.{result_id}"),
                        );
                        global.set_initializer(&values);
                        global.set_constant(true);
                        (
                            global.as_pointer_value(),
                            self.context
                                .i64_type()
                                .const_int(offsets.len() as u64, false),
                        )
                    }
                };
                let (key_offsets, key_count) = if text_key {
                    // Text keys are snapshotted by the runtime into immutable
                    // Rust-owned content; the mutable source handle is not a
                    // GC edge of the map entry.
                    (
                        self.context
                            .ptr_type(inkwell::AddressSpace::default())
                            .const_null(),
                        self.context.i64_type().const_zero(),
                    )
                } else {
                    offsets_for(key_ty, "key_offsets")
                };
                let (value_offsets, value_count) = offsets_for(value_ty, "value_offsets");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_new,
                        &[
                            key_offsets.into(),
                            key_count.into(),
                            self.byte_size_of(key_ty).into(),
                            value_offsets.into(),
                            value_count.into(),
                            string_content_keys.into(),
                            self.byte_size_of(value_ty).into(),
                        ],
                        "map",
                    )
                    .expect("map new");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::SetNew { set_id } => {
                let element_ty = self.module.set_types[*set_id as usize];
                let has_text_equality = matches!(
                    element_ty,
                    ir::IrType::String
                        | ir::IrType::Char
                        | ir::IrType::Nullable(ir::Nullable::String | ir::Nullable::Char)
                );
                let string_content_elements = self.bool_const(has_text_equality);
                let paths = if has_text_equality {
                    Vec::new()
                } else {
                    gc_reference_paths_of(self.module, element_ty)
                };
                let offsets: Vec<_> = paths
                    .iter()
                    .map(|path| {
                        if path.is_empty() {
                            self.context.i64_type().const_zero()
                        } else {
                            const_field_offset(
                                self.context,
                                self.llvm_type(element_ty)
                                    .expect("set element type has an LLVM layout")
                                    .into_struct_type(),
                                path,
                            )
                        }
                    })
                    .collect();
                let (offsets, count) = if offsets.is_empty() {
                    (
                        self.context
                            .ptr_type(inkwell::AddressSpace::default())
                            .const_null(),
                        self.context.i64_type().const_zero(),
                    )
                } else {
                    let values = self.context.i64_type().const_array(&offsets);
                    let result_id = instruction.result.expect("SetNew produces a value").0;
                    let global = self.llvm.add_global(
                        values.get_type(),
                        None,
                        &format!("zk.set.element_offsets.{result_id}"),
                    );
                    global.set_initializer(&values);
                    global.set_constant(true);
                    (
                        global.as_pointer_value(),
                        self.context.i64_type().const_int(paths.len() as u64, false),
                    )
                };
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_new,
                        &[
                            offsets.into(),
                            count.into(),
                            self.byte_size_of(element_ty).into(),
                            string_content_elements.into(),
                        ],
                        "set",
                    )
                    .expect("set new");
                call.try_as_basic_value().basic()
            }

            // `array.length` / `list.length`.
            ir::InstKind::ArrayLength(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let call = self
                    .builder
                    .build_call(
                        self.runtime.array_length,
                        &[receiver.into()],
                        "array_length",
                    )
                    .expect("array length");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::ListLength(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let call = self
                    .builder
                    .build_call(self.runtime.list_length, &[receiver.into()], "list_length")
                    .expect("list length");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::DequeLength(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.deque_length,
                        &[receiver.into()],
                        "deque_length",
                    )
                    .expect("deque length")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::PriorityQueueLength(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.priority_queue_length,
                        &[receiver.into()],
                        "priority_queue_length",
                    )
                    .expect("priority queue length")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::PriorityQueueIsEmpty(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let raw = self
                    .builder
                    .build_call(
                        self.runtime.priority_queue_is_empty,
                        &[receiver.into()],
                        "priority_queue_is_empty.raw",
                    )
                    .expect("priority queue is_empty")
                    .try_as_basic_value()
                    .basic()
                    .expect("i32 return")
                    .into_int_value();
                Some(
                    self.builder
                        .build_int_compare(
                            inkwell::IntPredicate::NE,
                            raw,
                            self.context.i32_type().const_zero(),
                            "priority_queue_is_empty",
                        )
                        .expect("priority queue empty comparison")
                        .into(),
                )
            }
            ir::InstKind::PriorityQueueVersion(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.priority_queue_version,
                        &[receiver.into()],
                        "priority_queue_version",
                    )
                    .expect("priority queue version")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::DequeIsEmpty(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let length = self
                    .builder
                    .build_call(
                        self.runtime.deque_length,
                        &[receiver.into()],
                        "deque_length",
                    )
                    .expect("deque length")
                    .try_as_basic_value()
                    .basic()
                    .expect("deque length result")
                    .into_int_value();
                let zero = length.get_type().const_zero();
                Some(
                    self.builder
                        .build_int_compare(IntPredicate::EQ, length, zero, "deque_is_empty")
                        .expect("deque empty comparison")
                        .into(),
                )
            }
            ir::InstKind::DequeVersion(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                self.builder
                    .build_call(
                        self.runtime.deque_version,
                        &[receiver.into()],
                        "deque_version",
                    )
                    .expect("deque version")
                    .try_as_basic_value()
                    .basic()
            }

            // `array[i]` / `list[i]` read.
            ir::InstKind::ArrayListLoad { receiver, index } => {
                let element = instruction.ty;
                let receiver_value = self.operand(*receiver);
                let function = if matches!(self.value_types[&receiver.0], ir::IrType::Array(_)) {
                    self.runtime.array_element
                } else {
                    self.runtime.list_element
                };
                let elem_size = self.byte_size_of(element);
                let ptr = self
                    .builder
                    .build_call(
                        function,
                        &[
                            receiver_value.into_pointer_value().into(),
                            self.operand(*index).into_int_value().into(),
                            elem_size.into(),
                        ],
                        "array_list_elem_ptr",
                    )
                    .expect("array/list element pointer")
                    .try_as_basic_value()
                    .basic()
                    .expect("element pointer")
                    .into_pointer_value();
                let llvm_element = self.llvm_type(element).expect("an element is never Void");
                Some(
                    self.builder
                        .build_load(llvm_element, ptr, "array_list_load")
                        .expect("array/list load"),
                )
            }

            // `array[i] = value` / `list[i] = value`.
            ir::InstKind::ArrayListStore {
                receiver,
                index,
                value,
            } => {
                let value_operand = self.operand(*value);
                let element_ty = self.value_types[&value.0];
                let receiver_value = self.operand(*receiver);
                let function = if matches!(self.value_types[&receiver.0], ir::IrType::Array(_)) {
                    self.runtime.array_element
                } else {
                    self.runtime.list_element
                };
                let elem_size = self.byte_size_of(element_ty);
                let ptr = self
                    .builder
                    .build_call(
                        function,
                        &[
                            receiver_value.into_pointer_value().into(),
                            self.operand(*index).into_int_value().into(),
                            elem_size.into(),
                        ],
                        "array_list_elem_ptr",
                    )
                    .expect("array/list element pointer")
                    .try_as_basic_value()
                    .basic()
                    .expect("element pointer")
                    .into_pointer_value();
                self.builder
                    .build_store(ptr, value_operand)
                    .expect("array/list store");
                None
            }
            ir::InstKind::DequePushFront { receiver, value }
            | ir::InstKind::DequePushBack { receiver, value } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (value_slot, _) = self.collection_value_slot(*value, "deque.value.slot");
                let function = if matches!(&instruction.kind, ir::InstKind::DequePushFront { .. }) {
                    self.runtime.deque_push_front
                } else {
                    self.runtime.deque_push_back
                };
                self.builder
                    .build_call(
                        function,
                        &[receiver.into(), value_slot.into()],
                        "deque_push",
                    )
                    .expect("deque push")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::DequeFront(receiver)
            | ir::InstKind::DequeBack(receiver)
            | ir::InstKind::DequePopFront(receiver)
            | ir::InstKind::DequePopBack(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let value_type = instruction.ty;
                let llvm_type = self.llvm_type(value_type).expect("deque element type");
                let output = self
                    .builder
                    .build_alloca(llvm_type, "deque.value.output")
                    .expect("deque output slot");
                let function = match &instruction.kind {
                    ir::InstKind::DequeFront(_) => self.runtime.deque_front,
                    ir::InstKind::DequeBack(_) => self.runtime.deque_back,
                    ir::InstKind::DequePopFront(_) => self.runtime.deque_pop_front,
                    _ => self.runtime.deque_pop_back,
                };
                self.builder
                    .build_call(function, &[receiver.into(), output.into()], "deque_access")
                    .expect("deque access");
                Some(
                    self.builder
                        .build_load(llvm_type, output, "deque.value")
                        .expect("load deque value"),
                )
            }
            ir::InstKind::DequeClear(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                self.builder
                    .build_call(self.runtime.deque_clear, &[receiver.into()], "deque_clear")
                    .expect("deque clear");
                None
            }
            ir::InstKind::PriorityQueueEnqueue { receiver, value } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let value_ty = self.value_types[&value.0];
                let (value_slot, _) =
                    self.collection_value_slot(*value, "priority_queue.value.slot");
                let _ = value_ty;
                self.builder
                    .build_call(
                        self.runtime.priority_queue_enqueue,
                        &[receiver.into(), value_slot.into()],
                        "priority_queue_enqueue",
                    )
                    .expect("priority queue enqueue")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::PriorityQueuePeek(receiver)
            | ir::InstKind::PriorityQueueDequeue(receiver) => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let value_type = instruction.ty;
                let llvm_type = self
                    .llvm_type(value_type)
                    .expect("priority queue element type");
                let output = self
                    .builder
                    .build_alloca(llvm_type, "priority_queue.value.output")
                    .expect("priority queue output slot");
                let function = if matches!(instruction.kind, ir::InstKind::PriorityQueuePeek(_)) {
                    self.runtime.priority_queue_peek
                } else {
                    self.runtime.priority_queue_dequeue
                };
                self.builder
                    .build_call(
                        function,
                        &[receiver.into(), output.into()],
                        "priority_queue_access",
                    )
                    .expect("priority queue access");
                Some(
                    self.builder
                        .build_load(llvm_type, output, "priority_queue.value")
                        .expect("load priority queue value"),
                )
            }

            // `list.add(value)`.
            ir::InstKind::ListAdd { receiver, value } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, _) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let is_ref = self.bool_const(self.is_gc_reference(element));
                let receiver = self.operand(*receiver).into_pointer_value();
                let value = self.operand(*value);
                let value_ptr = self.element_value_ptr(value, element);
                self.builder
                    .build_call(
                        self.runtime.list_add,
                        &[
                            receiver.into(),
                            value_ptr.into(),
                            elem_size.into(),
                            elem_align.into(),
                            is_ref.into(),
                        ],
                        "",
                    )
                    .expect("list add");
                None
            }
            // `list.insert(index, value)`.
            ir::InstKind::ListInsert {
                receiver,
                index,
                value,
            } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, _) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let is_ref = self.bool_const(self.is_gc_reference(element));
                let receiver = self.operand(*receiver).into_pointer_value();
                let index = self.operand(*index).into_int_value();
                let value = self.operand(*value);
                let value_ptr = self.element_value_ptr(value, element);
                self.builder
                    .build_call(
                        self.runtime.list_insert,
                        &[
                            receiver.into(),
                            index.into(),
                            value_ptr.into(),
                            elem_size.into(),
                            elem_align.into(),
                            is_ref.into(),
                        ],
                        "",
                    )
                    .expect("list insert");
                None
            }
            // `list.remove(index)`.
            ir::InstKind::ListRemove { receiver, index } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, _) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let is_ref = self.bool_const(self.is_gc_reference(element));
                let receiver = self.operand(*receiver).into_pointer_value();
                let index = self.operand(*index).into_int_value();
                self.builder
                    .build_call(
                        self.runtime.list_remove,
                        &[
                            receiver.into(),
                            index.into(),
                            elem_size.into(),
                            elem_align.into(),
                            is_ref.into(),
                        ],
                        "",
                    )
                    .expect("list remove");
                None
            }
            // `list.remove(value)` — removes the first matching element and
            // returns whether one was removed.
            ir::InstKind::ListRemoveValue { receiver, value } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, _) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let is_ref = self.bool_const(self.is_gc_reference(element));
                let receiver = self.operand(*receiver).into_pointer_value();
                let value = self.operand(*value);
                let value_ptr = self.element_value_ptr(value, element);
                self.builder
                    .build_call(
                        self.runtime.list_remove_value,
                        &[
                            receiver.into(),
                            value_ptr.into(),
                            elem_size.into(),
                            elem_align.into(),
                            is_ref.into(),
                        ],
                        "",
                    )
                    .expect("list remove value")
                    .try_as_basic_value()
                    .basic()
            }
            // `map.length()` / `set.length()`.
            ir::InstKind::MapLength(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let call = self
                    .builder
                    .build_call(self.runtime.map_length, &[receiver.into()], "map_len")
                    .expect("map length");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::MapVersion(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let call = self
                    .builder
                    .build_call(self.runtime.map_version, &[receiver.into()], "map_version")
                    .expect("map version");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::MapSortedKeys(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let ir::IrType::Map(map_id) = self.value_types[&operand.0] else {
                    panic!("MapSortedKeys receiver must be a Map")
                };
                let (key_type, _) = self.module.map_types[map_id as usize];
                let string_keys = self.context.i64_type().const_int(
                    matches!(key_type, ir::IrType::String | ir::IrType::Char) as u64,
                    false,
                );
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_sorted_keys,
                        &[receiver.into(), string_keys.into()],
                        "map.sorted.keys",
                    )
                    .expect("sorted Map key snapshot");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::MapKeys(operand)
            | ir::InstKind::MapValues(operand)
            | ir::InstKind::MapEntries(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let ir::IrType::Array(element_id) = instruction.ty else {
                    panic!("MapKeys/MapValues result must be an Array")
                };
                let element = self.module.array_types[element_id as usize];
                let size = self.byte_size_of(element);
                let align = self.byte_align_of(element);
                let result_id = instruction.result.expect("map snapshot produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.map.snapshot.offsets.{result_id}"));
                if matches!(&instruction.kind, ir::InstKind::MapEntries(_)) {
                    let ir::IrType::Value(_) = element else {
                        panic!("MapEntries array element must be a tuple value")
                    };
                    let (key_ty, value_ty) = match self.value_types[&operand.0] {
                        ir::IrType::Map(map_id) => self.module.map_types[map_id as usize],
                        _ => panic!("MapEntries receiver must be a Map"),
                    };
                    let tuple = self
                        .llvm_type(element)
                        .expect("MapEntries tuple has an LLVM layout")
                        .into_struct_type();
                    let key_offset = const_field_offset(self.context, tuple, &[0]);
                    let value_offset = const_field_offset(self.context, tuple, &[1]);
                    let call = self
                        .builder
                        .build_call(
                            self.runtime.map_entries,
                            &[
                                receiver.into(),
                                size.into(),
                                align.into(),
                                offsets.into(),
                                count.into(),
                                key_offset.into(),
                                self.byte_size_of(key_ty).into(),
                                value_offset.into(),
                                self.byte_size_of(value_ty).into(),
                            ],
                            "map.entries",
                        )
                        .expect("map entry snapshot");
                    call.try_as_basic_value().basic()
                } else {
                    let runtime = if matches!(&instruction.kind, ir::InstKind::MapKeys(_)) {
                        self.runtime.map_keys
                    } else {
                        self.runtime.map_values
                    };
                    let call = self
                        .builder
                        .build_call(
                            runtime,
                            &[
                                receiver.into(),
                                size.into(),
                                align.into(),
                                offsets.into(),
                                count.into(),
                            ],
                            "map.snapshot",
                        )
                        .expect("map snapshot");
                    call.try_as_basic_value().basic()
                }
            }
            ir::InstKind::SetLength(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let call = self
                    .builder
                    .build_call(self.runtime.set_length, &[receiver.into()], "set_len")
                    .expect("set length");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::SetValues(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let ir::IrType::Array(element_id) = instruction.ty else {
                    panic!("SetValues result must be an Array")
                };
                let element = self.module.array_types[element_id as usize];
                let size = self.byte_size_of(element);
                let align = self.byte_align_of(element);
                let result_id = instruction.result.expect("SetValues produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.set.snapshot.offsets.{result_id}"));
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_values,
                        &[
                            receiver.into(),
                            size.into(),
                            align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "set.snapshot",
                    )
                    .expect("set values snapshot");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::SetSortedElements(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let ir::IrType::Set(set_id) = self.value_types[&operand.0] else {
                    panic!("SetSortedElements receiver must be a Set")
                };
                let element_type = self.module.set_types[set_id as usize];
                let string_elements = self.context.i64_type().const_int(
                    matches!(element_type, ir::IrType::String | ir::IrType::Char) as u64,
                    false,
                );
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_sorted_elements,
                        &[receiver.into(), string_elements.into()],
                        "set.sorted.elements",
                    )
                    .expect("sorted Set element snapshot");
                call.try_as_basic_value().basic()
            }
            // `map.is_empty()` / `set.is_empty()`.
            ir::InstKind::MapIsEmpty(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_is_empty,
                        &[receiver.into()],
                        "map_is_empty",
                    )
                    .expect("map is empty");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::SetIsEmpty(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_is_empty,
                        &[receiver.into()],
                        "set_is_empty",
                    )
                    .expect("set is empty");
                call.try_as_basic_value().basic()
            }
            ir::InstKind::MapClear(operand) | ir::InstKind::SetClear(operand) => {
                let receiver = self.operand(*operand).into_pointer_value();
                let function = if matches!(&instruction.kind, ir::InstKind::MapClear(_)) {
                    self.runtime.map_clear
                } else {
                    self.runtime.set_clear
                };
                self.builder
                    .build_call(function, &[receiver.into()], "")
                    .expect("clear map or set");
                None
            }
            ir::InstKind::SetOperation {
                left,
                right,
                operation,
            } => {
                use ir::SetOperation as Op;
                let left = self.operand(*left).into_pointer_value();
                let right = self.operand(*right).into_pointer_value();
                let (callee, tag, returns_set) = match operation {
                    Op::Union => (self.runtime.set_algebra, 0, true),
                    Op::Intersection => (self.runtime.set_algebra, 1, true),
                    Op::Difference => (self.runtime.set_algebra, 2, true),
                    Op::SymmetricDifference => (self.runtime.set_algebra, 3, true),
                    Op::IsSubsetOf => (self.runtime.set_relation, 0, false),
                    Op::IsSupersetOf => (self.runtime.set_relation, 1, false),
                    Op::IsDisjoint => (self.runtime.set_relation, 2, false),
                };
                let call = self
                    .builder
                    .build_call(
                        callee,
                        &[
                            left.into(),
                            right.into(),
                            self.context.i32_type().const_int(tag, false).into(),
                        ],
                        "set.operation",
                    )
                    .expect("set operation");
                if returns_set {
                    Some(
                        call.try_as_basic_value()
                            .basic()
                            .expect("set algebra returns a set handle"),
                    )
                } else {
                    let result = call
                        .try_as_basic_value()
                        .basic()
                        .expect("set relation returns an integer status")
                        .into_int_value();
                    Some(
                        self.builder
                            .build_int_compare(
                                IntPredicate::NE,
                                result,
                                self.context.i32_type().const_zero(),
                                "set.relation",
                            )
                            .expect("set relation result")
                            .into(),
                    )
                }
            }
            // `map.set(key, value)`.
            ir::InstKind::MapSet {
                receiver,
                key,
                value,
            } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (key_slot, key_size) = self.collection_value_slot(*key, "map.key.slot");
                let value_ty = self.value_types[&value.0];
                let llvm_value_ty = self
                    .llvm_type(value_ty)
                    .expect("Map value has an LLVM type");
                let value_slot = self
                    .builder
                    .build_alloca(llvm_value_ty, "map.value.slot")
                    .expect("Map value slot");
                self.builder
                    .build_store(value_slot, self.operand(*value))
                    .expect("store Map value");
                let value_size = self.byte_size_of(value_ty);
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_set,
                        &[
                            receiver.into(),
                            key_slot.into(),
                            key_size.into(),
                            value_slot.into(),
                            value_size.into(),
                        ],
                        "",
                    )
                    .expect("map set");
                call.try_as_basic_value().basic()
            }
            // `map.contains_key(key)`.
            ir::InstKind::MapContainsKey { receiver, key } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (key_slot, key_size) = self.collection_value_slot(*key, "map.key.slot");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_contains_key,
                        &[receiver.into(), key_slot.into(), key_size.into()],
                        "map_contains",
                    )
                    .expect("map contains key");
                call.try_as_basic_value().basic()
            }
            // `set.add(value)`.
            ir::InstKind::SetAdd { receiver, value } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (value_slot, value_size) =
                    self.collection_value_slot(*value, "set.element.slot");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.set_add,
                        &[receiver.into(), value_slot.into(), value_size.into()],
                        "set_add",
                    )
                    .expect("set add")
                    .try_as_basic_value()
                    .basic()
                    .expect("set add returns an insertion status")
                    .into_int_value();
                Some(status.into())
            }
            // `set.contains(value)`.
            ir::InstKind::SetContains { receiver, value } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (value_slot, value_size) =
                    self.collection_value_slot(*value, "set.element.slot");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_contains,
                        &[receiver.into(), value_slot.into(), value_size.into()],
                        "set_contains",
                    )
                    .expect("set contains");
                call.try_as_basic_value().basic()
            }
            // Internal map value load; public lookup lowering guards presence.
            ir::InstKind::MapGet { receiver, key } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (key_slot, key_size) = self.collection_value_slot(*key, "map.key.slot");
                let value_type = instruction.ty;
                let inner_type = self.llvm_type(value_type).expect("map get value type");
                let output = self
                    .builder
                    .build_alloca(inner_type, "map.value.output")
                    .expect("Map output slot");
                let found = self
                    .builder
                    .build_call(
                        self.runtime.map_get,
                        &[
                            receiver.into(),
                            key_slot.into(),
                            key_size.into(),
                            output.into(),
                            self.byte_size_of(value_type).into(),
                        ],
                        "map_get_typed",
                    )
                    .expect("map get value")
                    .try_as_basic_value()
                    .basic()
                    .expect("map get presence result");
                let payload = self
                    .builder
                    .build_load(inner_type, output, "map.value")
                    .expect("load Map value");
                let _ = found; // The caller's Result branch already proved presence.
                Some(payload)
            }
            // `map.remove(key)`.
            ir::InstKind::MapRemove { receiver, key } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (key_slot, key_size) = self.collection_value_slot(*key, "map.key.slot");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.map_remove,
                        &[receiver.into(), key_slot.into(), key_size.into()],
                        "map_remove",
                    )
                    .expect("map remove");
                call.try_as_basic_value().basic()
            }
            // `set.remove(value)`.
            ir::InstKind::SetRemove { receiver, value } => {
                let receiver = self.operand(*receiver).into_pointer_value();
                let (value_slot, value_size) =
                    self.collection_value_slot(*value, "set.element.slot");
                let call = self
                    .builder
                    .build_call(
                        self.runtime.set_remove,
                        &[receiver.into(), value_slot.into(), value_size.into()],
                        "set_remove",
                    )
                    .expect("set remove");
                call.try_as_basic_value().basic()
            }

            // `array.clone()` / `list.clone()` — a shallow copy of the
            // element buffer; typed collection layouts preserve every nested
            // managed reference path.
            ir::InstKind::ArrayClone { receiver } | ir::InstKind::ListClone { receiver } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, is_ref) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let receiver = self.operand(*receiver).into_pointer_value();
                let call = if matches!(receiver_ty, ir::IrType::Array(_)) {
                    let result_id = instruction.result.expect("ArrayClone produces a value").0;
                    let (offsets, count) = self
                        .gc_offsets_for(element, &format!("zk.array.clone.offsets.{result_id}"));
                    self.builder.build_call(
                        self.runtime.array_clone_with_layout,
                        &[
                            receiver.into(),
                            elem_size.into(),
                            elem_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "",
                    )
                } else {
                    self.builder.build_call(
                        self.runtime.list_clone,
                        &[
                            receiver.into(),
                            elem_size.into(),
                            elem_align.into(),
                            self.bool_const(is_ref).into(),
                        ],
                        "",
                    )
                };
                call.expect("array/list clone").try_as_basic_value().basic()
            }
            // `array[start:end:step]` — shallow copy of the selected elements.
            ir::InstKind::ArraySlice {
                receiver,
                start,
                end,
                step,
            } => {
                let receiver_ty = self.value_types[&receiver.0];
                let (_, element, _) = self.array_list_element(receiver_ty);
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let result_id = instruction.result.expect("ArraySlice produces a value").0;
                let (offsets, count) =
                    self.gc_offsets_for(element, &format!("zk.array.slice.offsets.{result_id}"));
                let receiver = self.operand(*receiver).into_pointer_value();
                let start = self.operand(*start).into_int_value();
                let end = self.operand(*end).into_int_value();
                let step = self.operand(*step).into_int_value();
                self.builder
                    .build_call(
                        self.runtime.array_slice_with_layout,
                        &[
                            receiver.into(),
                            start.into(),
                            end.into(),
                            step.into(),
                            elem_size.into(),
                            elem_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "",
                    )
                    .expect("array slice")
                    .try_as_basic_value()
                    .basic()
            }

            // `Weak.from(value)` (roadmap Phase 4e, `fase-4e-weak`, design
            // D1/D2): an ordinary heap allocation — the same `alloc` call
            // `Alloc` itself uses — whose header carries the WeakCell
            // sentinel descriptor instead of a real class descriptor, and
            // whose one field is `value`'s own address. The "ever
            // allocated" flag is set unconditionally right after: a plain
            // store, no call, since nothing here needs to know its previous
            // value (design's own risk mitigation, task 4.3).
            ir::InstKind::WeakFrom(value) => {
                let struct_type = weak_cell_struct_type(self.context);
                let size = struct_type.size_of().expect("a sized WeakCell");
                let align = struct_type.get_alignment();

                let cell = self
                    .builder
                    .build_call(
                        self.runtime.alloc,
                        &[size.into(), align.into()],
                        "weak_cell",
                    )
                    .expect("call the allocator")
                    .try_as_basic_value()
                    .basic()
                    .expect("the allocator returns a pointer")
                    .into_pointer_value();

                self.builder
                    .build_store(cell, self.runtime.weak_cell_descriptor)
                    .expect("store the WeakCell sentinel descriptor");

                let target_field = self
                    .builder
                    .build_struct_gep(struct_type, cell, OBJECT_HEADER_FIELDS, "weak_target_init")
                    .expect("the WeakCell struct has a field past its header");
                self.builder
                    .build_store(target_field, self.operand(*value))
                    .expect("store the weak target");

                self.builder
                    .build_store(
                        self.runtime.weak_cell_ever_allocated,
                        self.context.i8_type().const_int(1, false),
                    )
                    .expect("set the WeakCell ever-allocated flag");

                Some(cell.into())
            }

            // `.upgrade()` (design D4): reads the WeakCell's target field and
            // builds the `T?` result — present exactly when the target is
            // not null. No collector-specific check needed here: mark/sweep
            // (design D3) guarantees the field itself is already null by the
            // time it is observed as dead, so a plain null check is
            // sufficient and correct by construction.
            ir::InstKind::WeakUpgrade(weak) => {
                let struct_type = weak_cell_struct_type(self.context);
                let cell = self.operand(*weak).into_pointer_value();
                let target_field = self
                    .builder
                    .build_struct_gep(struct_type, cell, OBJECT_HEADER_FIELDS, "weak_target")
                    .expect("the WeakCell struct has a field past its header");
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let target = self
                    .builder
                    .build_load(ptr_ty, target_field, "weak_target_value")
                    .expect("load the weak target")
                    .into_pointer_value();
                let is_null = self
                    .builder
                    .build_is_null(target, "weak_target_is_null")
                    .expect("weak target null check");
                let present = self
                    .builder
                    .build_not(is_null, "weak_target_present")
                    .expect("negation");

                let ty = llvm_type_in(
                    self.context,
                    instruction.ty,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a nullable type has a representation")
                .into_struct_type();
                let with_flag = self
                    .builder
                    .build_insert_value(ty.get_undef(), present, 0, "present")
                    .expect("present flag");
                Some(
                    self.builder
                        .build_insert_value(with_flag.into_struct_value(), target, 1, "wrapped")
                        .expect("payload")
                        .as_basic_value_enum(),
                )
            }

            // `.is_alive` (design D4): the same null check `.upgrade()`
            // does, without building a nullable payload.
            ir::InstKind::WeakIsAlive(weak) => {
                let struct_type = weak_cell_struct_type(self.context);
                let cell = self.operand(*weak).into_pointer_value();
                let target_field = self
                    .builder
                    .build_struct_gep(struct_type, cell, OBJECT_HEADER_FIELDS, "weak_target")
                    .expect("the WeakCell struct has a field past its header");
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let target = self
                    .builder
                    .build_load(ptr_ty, target_field, "weak_target_value")
                    .expect("load the weak target")
                    .into_pointer_value();
                let is_null = self
                    .builder
                    .build_is_null(target, "weak_target_is_null")
                    .expect("weak target null check");
                Some(
                    self.builder
                        .build_not(is_null, "weak_target_present")
                        .expect("negation")
                        .into(),
                )
            }

            // `x.clone()` (roadmap Phase 4e, `fase-4e-clone`, design D2):
            // one call into the runtime's own generic, descriptor-driven
            // deep-clone traversal — see `InstKind::Clone`'s own doc
            // comment for why the whole recursive walk lives there rather
            // than being unrolled into several IR instructions here.
            ir::InstKind::Clone(value) => match instruction.ty {
                ir::IrType::Callable(_) => {
                    let source = self.operand(*value).into_struct_value();
                    let capture_ptr = self
                        .builder
                        .build_extract_value(source, CALLABLE_CAPTURE_FIELD, "capture")
                        .expect("capture pointer")
                        .into_pointer_value();
                    let new_capture = self
                        .builder
                        .build_call(
                            self.runtime.clone_callable,
                            &[capture_ptr.into()],
                            "cloned_capture",
                        )
                        .expect("call zirk_rt_clone_callable")
                        .try_as_basic_value()
                        .basic()
                        .expect("zirk_rt_clone_callable returns a pointer")
                        .into_pointer_value();
                    let fn_ptr = self
                        .builder
                        .build_extract_value(source, CALLABLE_FUNCTION_FIELD, "fn")
                        .expect("function pointer");

                    let struct_type = self
                        .llvm_type(instruction.ty)
                        .expect("callable type")
                        .into_struct_type();
                    let mut cloned = self
                        .builder
                        .build_insert_value(
                            struct_type.get_undef(),
                            fn_ptr,
                            CALLABLE_FUNCTION_FIELD,
                            "fn",
                        )
                        .expect("function pointer")
                        .into_struct_value();
                    cloned = self
                        .builder
                        .build_insert_value(cloned, new_capture, CALLABLE_CAPTURE_FIELD, "capture")
                        .expect("capture pointer")
                        .into_struct_value();
                    let dispatch = self
                        .builder
                        .build_extract_value(source, CALLABLE_DISPATCH_FIELD, "dispatch")
                        .expect("uniform callable dispatch entry");
                    cloned = self
                        .builder
                        .build_insert_value(cloned, dispatch, CALLABLE_DISPATCH_FIELD, "dispatch")
                        .expect("uniform callable dispatch entry")
                        .into_struct_value();
                    Some(cloned.into())
                }
                _ => {
                    let source = self.operand(*value);
                    let cloned = self
                        .builder
                        .build_call(self.runtime.clone, &[source.into()], "cloned")
                        .expect("call zirk_rt_clone")
                        .try_as_basic_value()
                        .basic()
                        .expect("zirk_rt_clone returns a pointer");
                    Some(cloned)
                }
            },

            // `unsafe { ... }`'s own entry (roadmap Phase 4e,
            // `fase-4e-unsafe-journal`, design D1): one call into the
            // runtime's own undo-log allocator. The handle is an ordinary
            // opaque `ptr` value, never spilled to a GC root (it is a
            // `zirk-runtime`-internal allocation outside the collector's
            // own object model, the same category as `Weak<T>`'s WeakCell
            // context).
            ir::InstKind::JournalBegin => {
                let journal = self
                    .builder
                    .build_call(self.runtime.journal_begin, &[], "journal")
                    .expect("call zirk_rt_journal_begin")
                    .try_as_basic_value()
                    .basic()
                    .expect("zirk_rt_journal_begin returns a pointer");
                Some(journal)
            }

            // Before a `Store` to a local slot inside an active `unsafe`
            // block (design D1/D3): snapshots the slot's current bytes,
            // sized from its own static type.
            ir::InstKind::JournalRecordSlot { journal, slot } => {
                let journal = self.operand(*journal);
                let address = self.slots[slot];
                let len = self.byte_size_of(self.slot_types[slot]);
                self.builder
                    .build_call(
                        self.runtime.journal_record,
                        &[journal.into(), address.into(), len.into()],
                        "",
                    )
                    .expect("call zirk_rt_journal_record");
                None
            }

            // Before a `StoreField` inside an active `unsafe` block (design
            // D1/D3): snapshots the field's current bytes, sized from its
            // own static type.
            ir::InstKind::JournalRecordField {
                journal,
                object,
                index,
            } => {
                let journal = self.operand(*journal);
                let address = self.field_pointer(*object, *index);
                let id = self.object_layout_of(*object);
                let field_ty = self.module.objects[id as usize].fields[*index as usize].ty;
                let len = self.byte_size_of(field_ty);
                self.builder
                    .build_call(
                        self.runtime.journal_record,
                        &[journal.into(), address.into(), len.into()],
                        "",
                    )
                    .expect("call zirk_rt_journal_record");
                None
            }

            // An `unsafe {}` block's own normal fall-through exit, or
            // `commit {}`'s own entry against the enclosing block's journal
            // (design D1/D2): discards the undo log without restoring.
            ir::InstKind::JournalCommit(journal) => {
                let journal = self.operand(*journal);
                self.builder
                    .build_call(self.runtime.journal_commit, &[journal.into()], "")
                    .expect("call zirk_rt_journal_commit");
                None
            }

            // An `unsafe {}` block's own exit check point when an exception
            // is pending (design D1/D2): restores every recorded snapshot
            // in reverse order, then discards the log.
            ir::InstKind::JournalRollback(journal) => {
                let journal = self.operand(*journal);
                self.builder
                    .build_call(self.runtime.journal_rollback, &[journal.into()], "")
                    .expect("call zirk_rt_journal_rollback");
                None
            }

            // `transfer(source)` (roadmap Phase 4c): ownership transfer stub.
            ir::InstKind::ResourceTransfer { source } => {
                let source = self.operand(*source);
                let transferred = self
                    .builder
                    .build_call(self.runtime.resource_transfer, &[source.into()], "transfer")
                    .expect("call zirk_rt_resource_transfer")
                    .try_as_basic_value()
                    .basic()
                    .expect("zirk_rt_resource_transfer returns a pointer");
                Some(transferred)
            }

            // Build a boxed callable with its concrete target, managed
            // captures, and capture-layout-independent dispatch adapter.
            ir::InstKind::MakeCallable { target, captures } => {
                let ir::IrType::Callable(id) = instruction.ty else {
                    panic!(
                        "MakeCallable returns Callable, got {}",
                        instruction.ty.as_str()
                    )
                };
                let layout = &self.module.closures[id as usize];
                let fn_value = self.functions[target.as_str()];

                let fn_ptr = fn_value.as_global_value().as_pointer_value();

                let ptr_type = self.context.ptr_type(AddressSpace::default());
                let capture_ptr = if layout.captures.is_empty() {
                    ptr_type.const_null()
                } else {
                    let descriptor = self.closure_descriptors[id as usize];
                    let payload_size = self
                        .closure_payload_struct(layout)
                        .size_of()
                        .expect("a closure payload is sized");
                    let block = self
                        .builder
                        .build_call(
                            self.runtime.alloc_callable,
                            &[descriptor.into(), payload_size.into()],
                            "capture_block",
                        )
                        .expect("call zirk_rt_alloc_callable")
                        .try_as_basic_value()
                        .basic()
                        .expect("zirk_rt_alloc_callable returns a pointer")
                        .into_pointer_value();

                    for (index, capture) in captures.iter().enumerate() {
                        let value = self.operand(*capture);
                        let offset = self.closure_capture_offset(layout, index);
                        let slot_ptr = unsafe {
                            self.builder.build_gep(
                                self.context.i8_type(),
                                block,
                                &[offset],
                                "capture_slot",
                            )
                        }
                        .expect("capture slot gep");
                        self.builder
                            .build_store(slot_ptr, value)
                            .expect("store capture");
                    }

                    block
                };

                let dispatch_fn = self.build_callable_dispatch_adapter(target.as_str(), id);
                let struct_type = self
                    .llvm_type(instruction.ty)
                    .expect("callable type")
                    .into_struct_type();
                let mut value = self
                    .builder
                    .build_insert_value(
                        struct_type.get_undef(),
                        fn_ptr,
                        CALLABLE_FUNCTION_FIELD,
                        "fn",
                    )
                    .expect("function pointer")
                    .into_struct_value();
                value = self
                    .builder
                    .build_insert_value(value, capture_ptr, CALLABLE_CAPTURE_FIELD, "capture")
                    .expect("capture pointer")
                    .into_struct_value();
                value = self
                    .builder
                    .build_insert_value(
                        value,
                        dispatch_fn.as_global_value().as_pointer_value(),
                        CALLABLE_DISPATCH_FIELD,
                        "dispatch",
                    )
                    .expect("uniform callable dispatch entry")
                    .into_struct_value();

                Some(value.into())
            }

            ir::InstKind::CallCallable { callable, args } => {
                let callable_value = self.operand(*callable).into_struct_value();
                let dispatch_fn = self
                    .builder
                    .build_extract_value(callable_value, CALLABLE_DISPATCH_FIELD, "dispatch")
                    .expect("uniform callable dispatch entry")
                    .into_pointer_value();
                let capture_ptr = self
                    .builder
                    .build_extract_value(callable_value, CALLABLE_CAPTURE_FIELD, "capture")
                    .expect("capture pointer")
                    .into_pointer_value();

                let id = match self.value_types.get(&callable.0) {
                    Some(ir::IrType::Callable(id)) => *id,
                    other => panic!("CallCallable callable is not a Callable: {:?}", other),
                };
                let layout = &self.module.closures[id as usize];

                let mut arguments: Vec<BasicMetadataValueEnum> = Vec::with_capacity(args.len() + 1);
                arguments.push(capture_ptr.into());

                for arg in args {
                    arguments.push(self.operand(*arg).into());
                }

                let signature = callable_dispatch_signature(
                    self.context,
                    layout,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                );
                let call = self
                    .builder
                    .build_indirect_call(signature, dispatch_fn, &arguments, "call")
                    .expect("indirect callable call");

                call.try_as_basic_value().basic()
            }

            ir::InstKind::ThreadRun {
                callable_slot,
                result_slot,
                exception_slot,
            } => {
                let callable_type = self.slot_types[callable_slot];
                let callable = self
                    .builder
                    .build_load(
                        self.llvm_type(callable_type).expect("callable LLVM type"),
                        self.slots[callable_slot],
                        "thread.callable",
                    )
                    .expect("load thread callable")
                    .into_struct_value();
                let target = self
                    .builder
                    .build_extract_value(callable, CALLABLE_FUNCTION_FIELD, "thread.target")
                    .expect("thread target pointer")
                    .into_pointer_value();
                let captures = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "thread.captures")
                    .expect("thread captures pointer")
                    .into_pointer_value();
                let layout_id = match callable_type {
                    ir::IrType::Callable(id) => id,
                    _ => unreachable!("verified ThreadRun callable slot is Callable"),
                };
                let thunk = self.thread_run_thunk(layout_id);
                let thunk_ptr = thunk.as_global_value().as_pointer_value();
                let result_ptr = result_slot.map_or_else(
                    || self.context.ptr_type(AddressSpace::default()).const_null(),
                    |slot| self.slots[&slot],
                );
                Some(
                    self.builder
                        .build_call(
                            self.runtime.thread_run,
                            &[
                                thunk_ptr.into(),
                                target.into(),
                                captures.into(),
                                result_ptr.into(),
                                self.slots[exception_slot].into(),
                            ],
                            "thread.run.status",
                        )
                        .expect("zirk_rt_thread_run")
                        .try_as_basic_value()
                        .basic()
                        .expect("thread status"),
                )
            }

            // --- Structured concurrency (`concurrent-blocks-and-timers`) ---
            ir::InstKind::ScopeEnter => Some(
                self.builder
                    .build_call(self.runtime.scope_enter, &[], "scope")
                    .expect("zirk_rt_scope_enter")
                    .try_as_basic_value()
                    .basic()
                    .expect("scope id"),
            ),
            ir::InstKind::ScopeEnterCollectAll => Some(
                self.builder
                    .build_call(self.runtime.scope_enter_collect_all, &[], "collect_scope")
                    .expect("zirk_rt_scope_enter_collect_all")
                    .try_as_basic_value()
                    .basic()
                    .expect("collect-all scope id"),
            ),
            ir::InstKind::ScopeExit { scope } => {
                let scope_id = self.operand(*scope);
                self.builder
                    .build_call(self.runtime.scope_exit, &[scope_id.into()], "scope_close")
                    .expect("zirk_rt_scope_exit");
                None
            }
            ir::InstKind::ScopeExitCollectAll { scope } => {
                let scope_id = self.operand(*scope);
                self.builder
                    .build_call(
                        self.runtime.scope_exit_collect_all,
                        &[scope_id.into()],
                        "collect_scope_close",
                    )
                    .expect("zirk_rt_scope_exit_collect_all");
                None
            }
            ir::InstKind::ScopeCancel { scope } => {
                self.builder
                    .build_call(
                        self.runtime.scope_cancel,
                        &[self.operand(*scope).into()],
                        "",
                    )
                    .expect("zirk_rt_scope_cancel");
                None
            }
            ir::InstKind::ScopeOutcomeWaitTake {
                scope,
                ordinal_slot,
                value_slot,
                throwable_slot,
            } => Some(
                self.builder
                    .build_call(
                        self.runtime.scope_outcome_wait_take,
                        &[
                            self.operand(*scope).into(),
                            self.slots[ordinal_slot].into(),
                            self.slots[value_slot].into(),
                            self.slots[throwable_slot].into(),
                        ],
                        "scope_outcome_status",
                    )
                    .expect("zirk_rt_scope_outcome_wait_take")
                    .try_as_basic_value()
                    .basic()
                    .expect("scope outcome status"),
            ),
            ir::InstKind::BranchStart {
                target,
                body,
                scope,
                kind,
                delay,
                status_slot,
            } => {
                let callable = self.operand(*body).into_struct_value();
                let capture = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "branch_capture")
                    .expect("branch capture pointer")
                    .into_pointer_value();
                let layout_id = match self.value_types.get(&body.0) {
                    Some(ir::IrType::Callable(id)) => *id,
                    other => panic!("BranchStart body is not a Callable: {other:?}"),
                };
                let site = instruction.result.expect("BranchStart has a result").0;
                let thunk = self.build_branch_thunk(target.as_str(), layout_id, site);
                let thunk_ptr = thunk.as_global_value().as_pointer_value();
                let scope_id = self.operand(*scope);

                let status_ptr = self.slots[status_slot];
                let handle = match kind {
                    ir::BranchKind::Spawn => {
                        let job = self
                            .builder
                            .build_call(
                                self.runtime.spawn,
                                &[thunk_ptr.into(), capture.into(), status_ptr.into()],
                                "branch",
                            )
                            .expect("zirk_rt_spawn")
                            .try_as_basic_value()
                            .basic()
                            .expect("job handle");
                        let status = self
                            .builder
                            .build_load(self.context.i32_type(), status_ptr, "spawn_status")
                            .expect("spawn admission status")
                            .into_int_value();
                        let admitted = self
                            .builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                status,
                                self.context.i32_type().const_zero(),
                                "spawn_admitted",
                            )
                            .expect("spawn status comparison");
                        let parent = self
                            .builder
                            .get_insert_block()
                            .expect("spawn instruction has an insertion block")
                            .get_parent()
                            .expect("spawn block has a parent function");
                        let register = self.context.append_basic_block(parent, "spawn_register");
                        let done = self.context.append_basic_block(parent, "spawn_done");
                        self.builder
                            .build_conditional_branch(admitted, register, done)
                            .expect("branch on spawn admission");
                        self.builder.position_at_end(register);
                        self.builder
                            .build_call(
                                self.runtime.branch_register,
                                &[scope_id.into(), job.into()],
                                "",
                            )
                            .expect("zirk_rt_branch_register");
                        self.builder
                            .build_unconditional_branch(done)
                            .expect("join spawn status paths");
                        self.builder.position_at_end(done);
                        job
                    }
                    ir::BranchKind::TimerAfter | ir::BranchKind::TimerEvery => {
                        let entry = if matches!(kind, ir::BranchKind::TimerAfter) {
                            self.runtime.timer_after
                        } else {
                            self.runtime.timer_every
                        };
                        let nanos = self.operand(delay.expect("an ambient branch has a delay"));
                        self.builder
                            .build_call(
                                entry,
                                &[
                                    scope_id.into(),
                                    nanos.into(),
                                    thunk_ptr.into(),
                                    capture.into(),
                                    status_ptr.into(),
                                ],
                                "timer",
                            )
                            .expect("zirk_rt_timer_*")
                            .try_as_basic_value()
                            .basic()
                            .expect("timer job handle")
                    }
                };
                Some(handle)
            }
            ir::InstKind::BackgroundStart {
                target,
                body,
                status_slot,
            } => {
                let callable = self.operand(*body).into_struct_value();
                let capture = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "background_capture")
                    .expect("background capture pointer")
                    .into_pointer_value();
                let layout_id = match self.value_types.get(&body.0) {
                    Some(ir::IrType::Callable(id)) => *id,
                    other => panic!("BackgroundStart body is not a Callable: {other:?}"),
                };
                let target_name = format!("zk.background_thunk.{target}");
                let thunk =
                    self.build_branch_thunk_named(target.as_str(), layout_id, 0, &target_name);
                let status = self
                    .builder
                    .build_call(
                        self.runtime.background_spawn,
                        &[
                            thunk.as_global_value().as_pointer_value().into(),
                            capture.into(),
                        ],
                        "background_admission_status",
                    )
                    .expect("zirk_rt_background_spawn")
                    .try_as_basic_value()
                    .basic()
                    .expect("background admission status")
                    .into_int_value();
                self.builder
                    .build_store(self.slots[status_slot], status)
                    .expect("store background admission status");
                None
            }
            ir::InstKind::ServiceStart {
                target,
                body,
                status_slot,
            } => {
                let callable = self.operand(*body).into_struct_value();
                let capture = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "service_capture")
                    .expect("service capture block")
                    .into_pointer_value();
                let layout_id = match self.value_types.get(&body.0) {
                    Some(ir::IrType::Callable(id)) => *id,
                    other => panic!("ServiceStart body is not a Callable: {other:?}"),
                };
                let site = format!("{}.{}", instruction.span.file.0, instruction.span.start);
                let thunk = self.build_service_thunk(target, layout_id, &site);
                let _id = self
                    .builder
                    .build_call(
                        self.runtime.service_register,
                        &[
                            thunk.as_global_value().as_pointer_value().into(),
                            capture.into(),
                            capture.into(),
                            self.context.i32_type().const_int(1, false).into(),
                            self.slots[status_slot].into(),
                        ],
                        "service_id",
                    )
                    .expect("zirk_rt_service_register");
                None
            }
            ir::InstKind::ServiceReady {
                context,
                status_slot,
            } => {
                let status = self
                    .builder
                    .build_call(
                        self.runtime.service_ready,
                        &[self.operand(*context).into()],
                        "service_ready_status",
                    )
                    .expect("zirk_rt_service_ready")
                    .try_as_basic_value()
                    .basic()
                    .expect("service readiness status");
                self.builder
                    .build_store(self.slots[status_slot], status)
                    .expect("store service readiness status");
                None
            }
            ir::InstKind::ParallelForStart {
                target,
                body,
                count,
                budget_kind,
                budget_a,
                budget_b,
            } => {
                let callable = self.operand(*body).into_struct_value();
                let capture = self
                    .builder
                    .build_extract_value(callable, CALLABLE_CAPTURE_FIELD, "parallel_for_capture")
                    .expect("parallel_for capture pointer")
                    .into_pointer_value();
                let layout_id = match self.value_types.get(&body.0) {
                    Some(ir::IrType::Callable(id)) => *id,
                    other => panic!("ParallelForStart body is not a Callable: {other:?}"),
                };
                // `target` is already unique per source `parallel { for ... }`
                // (`Self::try_lower_parallel_for` names it from the region's
                // own span), so it doubles as the thunk's own unique suffix —
                // unlike `BranchStart`'s thunk, there is no `instruction.result`
                // to use instead: this instruction produces no value.
                let thunk =
                    self.build_parallel_for_thunk(target.as_str(), layout_id, target.as_str());
                let thunk_ptr = thunk.as_global_value().as_pointer_value();

                let kind_value = self.context.i8_type().const_int(*budget_kind as u64, false);
                let a_value = self.operand(*budget_a);
                let b_value = self.operand(*budget_b);
                let count_value = self.operand(*count);

                self.builder
                    .build_call(
                        self.runtime.parallel_for,
                        &[
                            kind_value.into(),
                            a_value.into(),
                            b_value.into(),
                            count_value.into(),
                            thunk_ptr.into(),
                            capture.into(),
                        ],
                        "",
                    )
                    .expect("zirk_rt_parallel_for");
                None
            }
            ir::InstKind::SafepointPoll => {
                self.builder
                    .build_call(self.runtime.safepoint_poll, &[], "")
                    .expect("zirk_rt_safepoint_poll");
                None
            }
            ir::InstKind::JobWait { job, result } => {
                let raw = self
                    .builder
                    .build_call(self.runtime.job_wait, &[self.operand(*job).into()], "wait")
                    .expect("zirk_rt_job_wait")
                    .try_as_basic_value()
                    .basic()
                    .expect("job result word")
                    .into_int_value();
                self.narrow_from_branch_word(raw, *result)
            }
            ir::InstKind::JobOutcomeWaitTake {
                job,
                value_slot,
                throwable_slot,
            } => Some(
                self.builder
                    .build_call(
                        self.runtime.job_outcome_wait_take,
                        &[
                            self.operand(*job).into(),
                            self.slots[value_slot].into(),
                            self.slots[throwable_slot].into(),
                        ],
                        "job_outcome_status",
                    )
                    .expect("zirk_rt_job_outcome_wait_take")
                    .try_as_basic_value()
                    .basic()
                    .expect("job outcome status"),
            ),
            ir::InstKind::JobOutcomeTake {
                job,
                value_slot,
                throwable_slot,
            } => Some(
                self.builder
                    .build_call(
                        self.runtime.job_outcome_take,
                        &[
                            self.operand(*job).into(),
                            self.slots[value_slot].into(),
                            self.slots[throwable_slot].into(),
                        ],
                        "job_outcome_status",
                    )
                    .expect("zirk_rt_job_outcome_take")
                    .try_as_basic_value()
                    .basic()
                    .expect("job outcome status"),
            ),
            ir::InstKind::JobResultLoad {
                box_pointer,
                result,
            } => {
                let pointer = self.operand(*box_pointer).into_pointer_value();
                let box_type = task_result_box_type(self.context, self.module, *result);
                let payload = self
                    .builder
                    .build_struct_gep(box_type, pointer, OBJECT_HEADER_FIELDS, "job_payload")
                    .expect("job payload field");
                Some(
                    self.builder
                        .build_load(
                            self.llvm_type(*result).expect("task payload type"),
                            payload,
                            "job_value",
                        )
                        .expect("load task payload"),
                )
            }
            ir::InstKind::JobWaitAny { jobs } | ir::InstKind::JobWaitAnyWithTimer { jobs } => {
                let (wait_any_fn, wait_any_symbol) = match &instruction.kind {
                    ir::InstKind::JobWaitAny { .. } => {
                        (self.runtime.job_wait_any, "zirk_rt_job_wait_any")
                    }
                    ir::InstKind::JobWaitAnyWithTimer { .. } => (
                        self.runtime.job_wait_any_with_timer,
                        "zirk_rt_job_wait_any_with_timer",
                    ),
                    _ => unreachable!("matched a wait-any instruction"),
                };
                let word = self.context.i64_type();
                let array_type = word.array_type(jobs.len() as u32);
                let array = self
                    .builder
                    .build_alloca(array_type, "wait_any_jobs")
                    .expect("wait-any job IDs");
                for (index, job) in jobs.iter().enumerate() {
                    // SAFETY: both indices are within the fixed array type
                    // sized from the verifier-checked bounded operand list.
                    let slot = unsafe {
                        self.builder.build_gep(
                            array_type,
                            array,
                            &[
                                self.context.i32_type().const_zero(),
                                self.context.i32_type().const_int(index as u64, false),
                            ],
                            "wait_any_job",
                        )
                    }
                    .expect("wait-any job slot");
                    self.builder
                        .build_store(slot, self.operand(*job))
                        .expect("store wait-any job ID");
                }
                // SAFETY: the zero index points at the first of the verifier-
                // checked fixed array's job IDs.
                let first = unsafe {
                    self.builder.build_gep(
                        array_type,
                        array,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "wait_any_ids",
                    )
                }
                .expect("wait-any IDs pointer");
                Some(
                    self.builder
                        .build_call(
                            wait_any_fn,
                            &[
                                first.into(),
                                word.const_int(jobs.len() as u64, false).into(),
                            ],
                            "wait_any_index",
                        )
                        .expect(wait_any_symbol)
                        .try_as_basic_value()
                        .basic()
                        .expect("wait-any returns a u64 index"),
                )
            }
            ir::InstKind::JobDone { job } => Some(
                self.builder
                    .build_call(self.runtime.job_done, &[self.operand(*job).into()], "done")
                    .expect("zirk_rt_job_done")
                    .try_as_basic_value()
                    .basic()
                    .expect("job done flag"),
            ),
            ir::InstKind::JobCancel { job } => {
                self.builder
                    .build_call(self.runtime.cancel, &[self.operand(*job).into()], "")
                    .expect("zirk_rt_cancel");
                None
            }
            ir::InstKind::JobCancelWithCleanupFailure { job } => {
                self.builder
                    .build_call(
                        self.runtime.cancel_with_cleanup_failure,
                        &[self.operand(*job).into()],
                        "",
                    )
                    .expect("zirk_rt_cancel_with_cleanup_failure");
                None
            }
            ir::InstKind::TimerSleep { nanos } => Some(
                self.builder
                    .build_call(self.runtime.sleep, &[self.operand(*nanos).into()], "")
                    .expect("zirk_rt_sleep")
                    .try_as_basic_value()
                    .basic()
                    .expect("sleep admission status"),
            ),
            ir::InstKind::AppSetShutdownGrace { nanos } => Some(
                self.builder
                    .build_call(
                        self.runtime.app_set_shutdown_grace,
                        &[self.operand(*nanos).into()],
                        "shutdown_grace_status",
                    )
                    .expect("zirk_rt_app_set_shutdown_grace")
                    .try_as_basic_value()
                    .basic()
                    .expect("shutdown grace status"),
            ),
            ir::InstKind::AppSetBackgroundError { callable, layout } => {
                let value = self.operand(*callable).into_struct_value();
                let target = self
                    .builder
                    .build_extract_value(value, CALLABLE_FUNCTION_FIELD, "background_error_target")
                    .expect("background handler function")
                    .into_pointer_value();
                let capture = self
                    .builder
                    .build_extract_value(value, CALLABLE_CAPTURE_FIELD, "background_error_capture")
                    .expect("background handler capture")
                    .into_pointer_value();
                let thunk = self.build_background_error_thunk(*layout);
                self.builder
                    .build_call(
                        self.runtime.app_set_background_error,
                        &[
                            thunk.as_global_value().as_pointer_value().into(),
                            target.into(),
                            capture.into(),
                        ],
                        "",
                    )
                    .expect("zirk_rt_app_set_background_error");
                None
            }
            ir::InstKind::ChannelNew {
                capacity,
                limit,
                element,
            } => {
                let element_size = self.byte_size_of(*element);
                let element_align = self.byte_align_of(*element);
                let paths = gc_reference_paths_of(self.module, *element);
                let offset_values: Vec<_> = paths
                    .iter()
                    .map(|path| {
                        if path.is_empty() {
                            self.context.i64_type().const_zero()
                        } else {
                            let payload = self
                                .llvm_type(*element)
                                .expect("channel element has an LLVM layout")
                                .into_struct_type();
                            const_field_offset(self.context, payload, path)
                        }
                    })
                    .collect();
                let (offsets, count) = if offset_values.is_empty() {
                    (
                        self.context.ptr_type(AddressSpace::default()).const_null(),
                        self.context.i64_type().const_zero(),
                    )
                } else {
                    let offsets_array = self.context.i64_type().const_array(&offset_values);
                    let site = instruction
                        .result
                        .expect("ChannelNew produces an endpoint")
                        .0;
                    let global = self.llvm.add_global(
                        offsets_array.get_type(),
                        None,
                        &format!("zk.channel.offsets.{site}"),
                    );
                    global.set_initializer(&offsets_array);
                    global.set_constant(true);
                    (
                        global.as_pointer_value(),
                        self.context
                            .i64_type()
                            .const_int(offset_values.len() as u64, false),
                    )
                };
                self.builder
                    .build_call(
                        self.runtime.chan_new,
                        &[
                            self.operand(*capacity).into(),
                            self.operand(*limit).into(),
                            element_size.into(),
                            element_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "channel",
                    )
                    .expect("zirk_rt_chan_new")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::ChannelSend { channel, value } => {
                let element = self.value_types[&value.0];
                let source = self.element_value_ptr(self.operand(*value), element);
                Some(
                    self.builder
                        .build_call(
                            self.runtime.chan_send,
                            &[self.operand(*channel).into(), source.into()],
                            "channel_send",
                        )
                        .expect("zirk_rt_chan_send")
                        .try_as_basic_value()
                        .basic()
                        .expect("zirk_rt_chan_send returns a status"),
                )
            }
            ir::InstKind::ChannelTrySend { channel, value } => {
                let element = self.value_types[&value.0];
                let source = self.element_value_ptr(self.operand(*value), element);
                self.builder
                    .build_call(
                        self.runtime.chan_try_send,
                        &[self.operand(*channel).into(), source.into()],
                        "channel_try_send",
                    )
                    .expect("zirk_rt_chan_try_send")
                    .try_as_basic_value()
                    .basic()
            }
            ir::InstKind::ChannelRecv { channel, element } => {
                let destination = self
                    .builder
                    .build_alloca(
                        self.llvm_type(*element).expect("channel element is sized"),
                        "channel_recv_value",
                    )
                    .expect("channel receive destination");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.chan_recv,
                        &[self.operand(*channel).into(), destination.into()],
                        "channel_recv",
                    )
                    .expect("zirk_rt_chan_recv")
                    .try_as_basic_value()
                    .basic()
                    .expect("zirk_rt_chan_recv returns a status")
                    .into_int_value();
                let payload_ty = self.llvm_type(*element).expect("channel element is sized");
                let payload = self
                    .builder
                    .build_load(payload_ty, destination, "channel_recv_loaded")
                    .expect("load channel receive destination");
                let nullable = self
                    .llvm_type(ir::IrType::Nullable(
                        ir::Nullable::of(*element).expect("channel element is nullable"),
                    ))
                    .expect("nullable channel element has a representation")
                    .into_struct_type();
                let present = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        status,
                        self.context.i32_type().const_zero(),
                        "channel_received",
                    )
                    .expect("compare channel receive status");
                let received = self
                    .builder
                    .build_insert_value(
                        nullable.const_zero(),
                        self.context.bool_type().const_int(1, false),
                        0,
                        "channel_present",
                    )
                    .expect("channel present flag")
                    .into_struct_value();
                let received = self
                    .builder
                    .build_insert_value(received, payload, 1, "channel_payload")
                    .expect("channel payload")
                    .into_struct_value();
                Some(
                    self.builder
                        .build_select(present, received, nullable.const_zero(), "channel_result")
                        .expect("select channel receive result"),
                )
            }
            ir::InstKind::ChannelTryRecv { channel, element } => {
                let destination = self
                    .builder
                    .build_alloca(
                        self.llvm_type(*element).expect("channel element is sized"),
                        "channel_try_recv_value",
                    )
                    .expect("channel try receive destination");
                let status = self
                    .builder
                    .build_call(
                        self.runtime.chan_try_recv,
                        &[self.operand(*channel).into(), destination.into()],
                        "channel_try_recv",
                    )
                    .expect("zirk_rt_chan_try_recv")
                    .try_as_basic_value()
                    .basic()
                    .expect("zirk_rt_chan_try_recv returns a status")
                    .into_int_value();
                let payload_ty = self.llvm_type(*element).expect("channel element is sized");
                let payload = self
                    .builder
                    .build_load(payload_ty, destination, "channel_try_recv_loaded")
                    .expect("load channel try receive destination");
                let result_ty = self
                    .llvm_type(instruction.ty)
                    .expect("receive outcome has an LLVM representation")
                    .into_struct_type();
                let mut result = self
                    .builder
                    .build_insert_value(result_ty.const_zero(), status, 0, "channel_try_tag")
                    .expect("channel try receive tag")
                    .into_struct_value();
                if let ir::IrType::Enum(id) = instruction.ty
                    && let Some(&field) = self.module.enums[id as usize].variants[0].first()
                {
                    result = self
                        .builder
                        .build_insert_value(result, payload, field + 1, "channel_try_payload")
                        .expect("channel try receive payload")
                        .into_struct_value();
                }
                Some(result.into())
            }
            ir::InstKind::ChannelClose { channel } => {
                self.builder
                    .build_call(
                        self.runtime.chan_close,
                        &[self.operand(*channel).into()],
                        "",
                    )
                    .expect("zirk_rt_chan_close");
                None
            }
            ir::InstKind::ChannelLength(channel) => self
                .builder
                .build_call(
                    self.runtime.chan_len,
                    &[self.operand(*channel).into()],
                    "channel_length",
                )
                .expect("zirk_rt_chan_len")
                .try_as_basic_value()
                .basic(),
            ir::InstKind::ChannelIsClosed(channel) => self
                .builder
                .build_call(
                    self.runtime.chan_is_closed,
                    &[self.operand(*channel).into()],
                    "channel_closed",
                )
                .expect("zirk_rt_chan_is_closed")
                .try_as_basic_value()
                .basic(),
            ir::InstKind::ChannelCapacity(channel) => self
                .builder
                .build_call(
                    self.runtime.chan_capacity,
                    &[self.operand(*channel).into()],
                    "channel_capacity",
                )
                .expect("zirk_rt_chan_capacity")
                .try_as_basic_value()
                .basic(),
            ir::InstKind::ChannelDrain {
                channel,
                element_id,
            } => {
                let element = self.module.list_types[*element_id as usize];
                let elem_size = self.byte_size_of(element);
                let elem_align = self.byte_align_of(element);
                let paths = gc_reference_paths_of(self.module, element);
                let offset_values: Vec<_> = paths
                    .iter()
                    .map(|path| {
                        if path.is_empty() {
                            self.context.i64_type().const_zero()
                        } else {
                            let payload = self
                                .llvm_type(element)
                                .expect("channel drain element has an LLVM layout")
                                .into_struct_type();
                            const_field_offset(self.context, payload, path)
                        }
                    })
                    .collect();
                let (offsets, count) = if offset_values.is_empty() {
                    (
                        self.context.ptr_type(AddressSpace::default()).const_null(),
                        self.context.i64_type().const_zero(),
                    )
                } else {
                    let offsets_array = self.context.i64_type().const_array(&offset_values);
                    let site = instruction.result.expect("ChannelDrain produces a list").0;
                    let global = self.llvm.add_global(
                        offsets_array.get_type(),
                        None,
                        &format!("zk.channel.drain.offsets.{site}"),
                    );
                    global.set_initializer(&offsets_array);
                    global.set_constant(true);
                    (
                        global.as_pointer_value(),
                        self.context
                            .i64_type()
                            .const_int(offset_values.len() as u64, false),
                    )
                };
                let list = self
                    .builder
                    .build_call(
                        self.runtime.list_new_with_layout,
                        &[
                            elem_size.into(),
                            elem_align.into(),
                            offsets.into(),
                            count.into(),
                        ],
                        "channel_drain_list",
                    )
                    .expect("list allocation for channel drain")
                    .try_as_basic_value()
                    .basic()
                    .expect("list_new returns a list");
                self.builder
                    .build_call(
                        self.runtime.chan_drain,
                        &[
                            self.operand(*channel).into(),
                            list.into(),
                            elem_size.into(),
                            elem_align.into(),
                        ],
                        "channel_drain",
                    )
                    .expect("zirk_rt_chan_drain");
                Some(list)
            }

            // `Dependent.from(base, field_ptr)` (roadmap Phase 4e,
            // `phase-4e-memory`, design D1): surface-only placeholder. The
            // real implementation will allocate a two-word `{ base, field }`
            // value; until then, return a null pointer of the result type so
            // the instruction has a value.
            ir::InstKind::DependentFrom { .. } => {
                let ptr = llvm_type_in(
                    self.context,
                    instruction.ty,
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("Dependent<T> has an LLVM representation")
                .into_pointer_type();
                Some(ptr.const_null().into())
            }

            // `PinObject` (roadmap Phase 4e, `phase-4e-memory`, design D1):
            // records the pin in the runtime and returns the same object
            // pointer as a `Pin<T>` value.
            ir::InstKind::PinObject { object } => {
                self.builder
                    .build_call(
                        self.runtime.pin_object,
                        &[self.operand(*object).into()],
                        "pin",
                    )
                    .expect("call zirk_rt_pin_object");
                Some(self.operand(*object))
            }
            // `UnpinObject` (roadmap Phase 4e, `phase-4e-memory`, design D1):
            // removes the object from the per-thread pin list.
            ir::InstKind::UnpinObject { object } => {
                self.builder
                    .build_call(
                        self.runtime.unpin_object,
                        &[self.operand(*object).into()],
                        "unpin",
                    )
                    .expect("call zirk_rt_unpin_object");
                None
            }
        };

        if requester.is_some() {
            self.builder
                .build_call(self.runtime.permission_requester_clear, &[], "")
                .expect("clear protected call-site requester");
        }

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

    /// A stack slot with 16-byte alignment — what a `{ i128, i8 }` `Decimal`
    /// (and a bare `i128`) needs, since Rust's `read`/`write` on those debug-
    /// checks the pointer alignment.
    fn aligned_alloca(
        &self,
        ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> inkwell::values::PointerValue<'ctx> {
        let slot = self
            .builder
            .build_alloca(ty, name)
            .expect("aligned stack slot");
        slot.as_instruction()
            .expect("an alloca is an instruction")
            .set_alignment(16)
            .expect("16 is a valid alignment");
        slot
    }

    /// The out-parameter shape every `stdin.read_*` FFI operation writes,
    /// matching `zirk_runtime::io::StdinReadOut`'s `#[repr(C)]` layout field
    /// for field (`add-stdin-stdout-standard-library`).
    fn stdin_out_struct(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i32_type().into(),
                self.context.i32_type().into(),
                self.context.ptr_type(AddressSpace::default()).into(),
            ],
            false,
        )
    }

    fn filesystem_argument_word(&mut self, operand: ir::Operand) -> IntValue<'ctx> {
        let ty = self.value_types[&operand.0];
        let value = self.operand(operand);
        match ty {
            ir::IrType::Nullable(nullable) => {
                let value = value.into_struct_value();
                let present = self
                    .builder
                    .build_extract_value(value, 0, "fs.arg.present")
                    .expect("nullable argument presence")
                    .into_int_value();
                let inner = self
                    .builder
                    .build_extract_value(value, 1, "fs.arg.value")
                    .expect("nullable argument value");
                let word = self.filesystem_scalar_word(inner, nullable.inner());
                self.builder
                    .build_select(
                        present,
                        word,
                        self.context.i64_type().const_zero(),
                        "fs.arg.nullable",
                    )
                    .expect("pack nullable argument")
                    .into_int_value()
            }
            ir::IrType::Enum(_) => {
                let tag = self
                    .builder
                    .build_extract_value(value.into_struct_value(), 0, "fs.arg.enum.tag")
                    .expect("enum argument discriminant")
                    .into_int_value();
                self.builder
                    .build_int_z_extend(tag, self.context.i64_type(), "fs.arg.enum.word")
                    .expect("extend enum discriminant")
            }
            other => self.filesystem_scalar_word(value, other),
        }
    }

    fn filesystem_scalar_word(
        &mut self,
        value: BasicValueEnum<'ctx>,
        ty: ir::IrType,
    ) -> IntValue<'ctx> {
        let target = self.context.i64_type();
        match value {
            BasicValueEnum::PointerValue(pointer) => self
                .builder
                .build_ptr_to_int(pointer, target, "fs.arg.pointer")
                .expect("pack filesystem pointer argument"),
            BasicValueEnum::IntValue(integer) => {
                let width = integer.get_type().get_bit_width();
                if width == 64 {
                    integer
                } else if width > 64 {
                    self.builder
                        .build_int_truncate(integer, target, "fs.arg.truncate")
                        .expect("truncate filesystem scalar")
                } else if self.is_signed(ty) {
                    self.builder
                        .build_int_s_extend(integer, target, "fs.arg.sign_extend")
                        .expect("sign extend filesystem scalar")
                } else {
                    self.builder
                        .build_int_z_extend(integer, target, "fs.arg.zero_extend")
                        .expect("zero extend filesystem scalar")
                }
            }
            _ => panic!("unsupported std.fs/std.path argument type: {}", ty.as_str()),
        }
    }

    fn call_filesystem(
        &mut self,
        operation: i32,
        words: &[IntValue<'ctx>],
    ) -> (
        IntValue<'ctx>,
        IntValue<'ctx>,
        IntValue<'ctx>,
        PointerValue<'ctx>,
    ) {
        let args_pointer = if words.is_empty() {
            self.context.ptr_type(AddressSpace::default()).const_null()
        } else {
            let array_type = self.context.i64_type().array_type(words.len() as u32);
            let slot = self
                .builder
                .build_alloca(array_type, "fs.args")
                .expect("filesystem argument array");
            for (index, word) in words.iter().enumerate() {
                let element = unsafe {
                    self.builder.build_gep(
                        array_type,
                        slot,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_int(index as u64, false),
                        ],
                        "fs.arg.slot",
                    )
                }
                .expect("filesystem argument slot");
                self.builder
                    .build_store(element, *word)
                    .expect("store filesystem argument");
            }
            slot
        };
        let operation = self.context.i32_type().const_int(operation as u64, false);
        let count = self.context.i64_type().const_int(words.len() as u64, false);
        self.call_network_out(
            self.runtime.filesystem_call,
            &[operation.into(), args_pointer.into(), count.into()],
        )
    }

    fn build_filesystem_outcome(
        &mut self,
        result_type: ir::IrType,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        scalar: IntValue<'ctx>,
        payload: PointerValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match result_type {
            ir::IrType::Enum(enum_id) => {
                let layout = self.module.enums[enum_id as usize].clone();
                let value_type = layout.fields[layout.variants[0][0] as usize].ty;
                match value_type {
                    ir::IrType::Void => self.build_void_tagged_outcome(enum_id, kind, error),
                    ir::IrType::Int(ir::IntWidth::U64) => {
                        self.build_u64_tagged_outcome(enum_id, kind, error, scalar)
                    }
                    ir::IrType::Boolean => {
                        self.build_filesystem_boolean_result(enum_id, kind, error, scalar)
                    }
                    ir::IrType::Nullable(ir::Nullable::String) => self
                        .build_nullable_string_tagged_outcome(enum_id, kind, error, payload, 0, 1),
                    ir::IrType::Enum(inner_id)
                        if self.module.enums[inner_id as usize].name == "FileLine" =>
                    {
                        self.build_file_line_result(enum_id, inner_id, kind, error, payload)
                    }
                    ir::IrType::String | ir::IrType::Array(_) | ir::IrType::List(_) => {
                        self.build_tagged_outcome(enum_id, kind, error, payload, 0, 1)
                    }
                    other => panic!("unsupported filesystem Result payload: {}", other.as_str()),
                }
            }
            ir::IrType::Boolean => self
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    scalar,
                    self.context.i64_type().const_zero(),
                    "fs.boolean",
                )
                .expect("convert filesystem boolean")
                .into(),
            ir::IrType::String | ir::IrType::List(_) | ir::IrType::Array(_) => payload.into(),
            ir::IrType::Nullable(ir::Nullable::String) => {
                let nullable_type = self
                    .llvm_type(result_type)
                    .expect("nullable filesystem string type")
                    .into_struct_type();
                let present = self
                    .builder
                    .build_is_not_null(payload, "fs.nullable.present")
                    .expect("check nullable filesystem string");
                let value = self
                    .builder
                    .build_insert_value(
                        nullable_type.get_undef(),
                        present,
                        0,
                        "fs.nullable.presence",
                    )
                    .expect("insert nullable presence")
                    .into_struct_value();
                self.builder
                    .build_insert_value(value, payload, 1, "fs.nullable.value")
                    .expect("insert nullable payload")
                    .as_basic_value_enum()
            }
            other => panic!("unsupported direct filesystem result: {}", other.as_str()),
        }
    }

    fn build_filesystem_boolean_result(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        scalar: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let boolean = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                scalar,
                self.context.i64_type().const_zero(),
                "fs.boolean.value",
            )
            .expect("convert filesystem boolean");
        let tag = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                kind,
                self.context.i32_type().const_int(1, false),
                "fs.boolean.error_tag",
            )
            .expect("select filesystem result tag");
        let outer_tag = self
            .builder
            .build_select(
                tag,
                self.context.i32_type().const_int(1, false),
                self.context.i32_type().const_zero(),
                "fs.boolean.result_tag",
            )
            .expect("select filesystem result tag")
            .into_int_value();
        let mut built = self
            .builder
            .build_insert_value(
                struct_type.const_zero(),
                outer_tag,
                0,
                "fs.boolean.result.kind",
            )
            .expect("insert filesystem result tag")
            .into_struct_value();
        let value_field = layout.variants[0][0] + ENUM_HEADER_FIELDS;
        built = self
            .builder
            .build_insert_value(built, boolean, value_field, "fs.boolean.result.value")
            .expect("insert filesystem boolean value")
            .into_struct_value();
        let error_field = layout.variants[1][0] + ENUM_HEADER_FIELDS;
        built = self
            .builder
            .build_insert_value(built, error, error_field, "fs.boolean.result.error")
            .expect("insert filesystem boolean error")
            .into_struct_value();
        built.into()
    }

    fn build_file_line_result(
        &mut self,
        result_id: u32,
        line_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        payload: PointerValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let line_layout = self.module.enums[line_id as usize].clone();
        let line_type = enum_struct(
            self.context,
            &line_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let is_end = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                kind,
                self.context.i32_type().const_int(2, false),
                "fs.line.eof",
            )
            .expect("check filesystem EOF");
        let line_tag = self
            .builder
            .build_select(
                is_end,
                self.context.i32_type().const_int(1, false),
                self.context.i32_type().const_zero(),
                "fs.line.tag",
            )
            .expect("select FileLine variant")
            .into_int_value();
        let mut line = self
            .builder
            .build_insert_value(line_type.const_zero(), line_tag, 0, "fs.line.value.tag")
            .expect("insert FileLine tag")
            .into_struct_value();
        let text_field = line_layout.variants[0][0] + ENUM_HEADER_FIELDS;
        line = self
            .builder
            .build_insert_value(line, payload, text_field, "fs.line.text")
            .expect("insert FileLine text")
            .into_struct_value();

        let result_layout = self.module.enums[result_id as usize].clone();
        let result_type = enum_struct(
            self.context,
            &result_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let failed = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                kind,
                self.context.i32_type().const_int(1, false),
                "fs.line.failed",
            )
            .expect("check filesystem line error");
        let tag = self
            .builder
            .build_select(
                failed,
                self.context.i32_type().const_int(1, false),
                self.context.i32_type().const_zero(),
                "fs.line.result.tag",
            )
            .expect("select filesystem result variant")
            .into_int_value();
        let mut result = self
            .builder
            .build_insert_value(result_type.const_zero(), tag, 0, "fs.line.result.kind")
            .expect("insert Result tag")
            .into_struct_value();
        let value_field = result_layout.variants[0][0] + ENUM_HEADER_FIELDS;
        result = self
            .builder
            .build_insert_value(result, line, value_field, "fs.line.result.value")
            .expect("insert FileLine result")
            .into_struct_value();
        let error_field = result_layout.variants[1][0] + ENUM_HEADER_FIELDS;
        result = self
            .builder
            .build_insert_value(result, error, error_field, "fs.line.result.error")
            .expect("insert FileError result")
            .into_struct_value();
        result.into()
    }

    /// Calls a `stdin.read_*` native operation through its out-parameter
    /// convention: allocates a `StdinReadOut` stack slot, calls `function`
    /// with `extra_args` followed by a pointer to that slot, and loads the
    /// `(kind, error, payload)` fields back out.
    fn call_stdin_read(
        &mut self,
        function: FunctionValue<'ctx>,
        extra_args: &[BasicValueEnum<'ctx>],
    ) -> (IntValue<'ctx>, IntValue<'ctx>, PointerValue<'ctx>) {
        let out_ty = self.stdin_out_struct();
        let slot = self.aligned_alloca(out_ty.into(), "stdin.out");
        let mut args: Vec<BasicMetadataValueEnum> =
            extra_args.iter().map(|value| (*value).into()).collect();
        args.push(slot.into());
        self.builder
            .build_call(function, &args, "stdin_read")
            .expect("call to a stdin read operation");
        let loaded = self
            .builder
            .build_load(out_ty, slot, "stdin.out.load")
            .expect("load the stdin read outcome")
            .into_struct_value();
        let kind = self
            .builder
            .build_extract_value(loaded, 0, "stdin.kind")
            .expect("extract kind")
            .into_int_value();
        let error = self
            .builder
            .build_extract_value(loaded, 1, "stdin.error")
            .expect("extract error")
            .into_int_value();
        let payload = self
            .builder
            .build_extract_value(loaded, 2, "stdin.payload")
            .expect("extract payload")
            .into_pointer_value();
        (kind, error, payload)
    }

    fn network_out_struct(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i32_type().into(),
                self.context.i32_type().into(),
                self.context.i64_type().into(),
                self.context.ptr_type(AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Calls one compiler-owned network leaf. The shared out record contains
    /// `(kind, error, scalar, payload)` so pointer-backed arrays and integer
    /// handles/counts use one stable C ABI layout.
    fn call_network_out(
        &mut self,
        function: FunctionValue<'ctx>,
        extra_args: &[BasicMetadataValueEnum<'ctx>],
    ) -> (
        IntValue<'ctx>,
        IntValue<'ctx>,
        IntValue<'ctx>,
        PointerValue<'ctx>,
    ) {
        let out_ty = self.network_out_struct();
        let slot = self.aligned_alloca(out_ty.into(), "network.out");
        let mut args = extra_args.to_vec();
        args.push(slot.into());
        self.builder
            .build_call(function, &args, "network_call")
            .expect("call to a network runtime operation");
        let loaded = self
            .builder
            .build_load(out_ty, slot, "network.out.load")
            .expect("load the network result record")
            .into_struct_value();
        let extract_int = |builder: &Builder<'ctx>, index, name| {
            builder
                .build_extract_value(loaded, index, name)
                .expect("extract network result field")
                .into_int_value()
        };
        let kind = extract_int(self.builder, 0, "network.kind");
        let error = extract_int(self.builder, 1, "network.error");
        let scalar = extract_int(self.builder, 2, "network.scalar");
        let payload = self
            .builder
            .build_extract_value(loaded, 3, "network.payload")
            .expect("extract network payload")
            .into_pointer_value();
        (kind, error, scalar, payload)
    }

    fn build_u64_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        scalar: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = self
            .builder
            .build_insert_value(struct_type.const_zero(), kind, 0, "network.result.kind")
            .expect("insert network result tag")
            .into_struct_value();
        let value_field = layout.variants[0][0];
        debug_assert_eq!(
            layout.fields[value_field as usize].ty,
            ir::IrType::Int(ir::IntWidth::U64),
            "TCP handles and write counts use UInt64 result payloads"
        );
        let built = self
            .builder
            .build_insert_value(
                built,
                scalar,
                value_field + ENUM_HEADER_FIELDS,
                "network.result.value",
            )
            .expect("insert network scalar result")
            .into_struct_value();
        let error_field = layout.variants[1][0];
        debug_assert_eq!(
            layout.fields[error_field as usize].ty,
            ir::IrType::Int(ir::IntWidth::I32),
            "TCPError is represented by its i32 discriminant"
        );
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "network.result.error",
            )
            .expect("insert network error")
            .into_struct_value()
            .into()
    }

    fn build_u64_object_tagged_outcome(
        &mut self,
        enum_id: u32,
        class_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        handle: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = &self.module.objects[class_id as usize];
        debug_assert_eq!(layout.fields.len(), 1);
        debug_assert_eq!(layout.fields[0].ty, ir::IrType::Int(ir::IntWidth::U64));
        let object_type = object_struct(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let object = self
            .builder
            .build_call(
                self.runtime.alloc,
                &[
                    object_type.size_of().expect("TCPStream object size").into(),
                    object_type.get_alignment().into(),
                ],
                "tcp.stream.object",
            )
            .expect("allocate accepted TCPStream")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        self.builder
            .build_store(object, self.descriptors[class_id as usize])
            .expect("write TCPStream type descriptor");
        let field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS,
                "tcp.stream.handle",
            )
            .expect("address TCPStream handle field");
        self.builder
            .build_store(field, handle)
            .expect("store accepted TCPStream handle");

        let result_layout = self.module.enums[enum_id as usize].clone();
        let result_type = enum_struct(
            self.context,
            &result_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = self
            .builder
            .build_insert_value(result_type.const_zero(), kind, 0, "tcp.accept.kind")
            .expect("insert TCP accept result discriminant")
            .into_struct_value();
        let value_field = result_layout.variants[0][0];
        let built = self
            .builder
            .build_insert_value(
                built,
                object,
                value_field + ENUM_HEADER_FIELDS,
                "tcp.accept.stream",
            )
            .expect("insert accepted TCPStream")
            .into_struct_value();
        let error_field = result_layout.variants[1][0];
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "tcp.accept.error",
            )
            .expect("insert TCP accept error")
            .into_struct_value()
            .into()
    }

    fn build_udp_datagram_tagged_outcome(
        &mut self,
        enum_id: u32,
        class_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        bytes: PointerValue<'ctx>,
        peer: PointerValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let rooted_bytes = self
            .builder
            .build_alloca(pointer_type, "udp.datagram.bytes.root")
            .expect("allocate UDP payload root slot");
        self.builder
            .build_store(rooted_bytes, bytes)
            .expect("root UDP payload across allocation");
        let rooted_peer = self
            .builder
            .build_alloca(pointer_type, "udp.datagram.peer.root")
            .expect("allocate UDP peer root slot");
        self.builder
            .build_store(rooted_peer, peer)
            .expect("root UDP peer across allocation");

        let layout = self.module.objects[class_id as usize].clone();
        let object_type = object_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let object = self
            .builder
            .build_call(
                self.runtime.alloc,
                &[
                    object_type
                        .size_of()
                        .expect("UDPDatagram object size")
                        .into(),
                    object_type.get_alignment().into(),
                ],
                "udp.datagram.object",
            )
            .expect("allocate UDPDatagram")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        self.builder
            .build_store(object, self.descriptors[class_id as usize])
            .expect("write UDPDatagram type descriptor");
        for (field_index, rooted, name) in [
            (0, rooted_bytes, "udp.datagram.bytes"),
            (1, rooted_peer, "udp.datagram.peer"),
        ] {
            let field = self
                .builder
                .build_struct_gep(
                    object_type,
                    object,
                    OBJECT_HEADER_FIELDS + field_index,
                    &format!("{name}.field"),
                )
                .expect("address UDPDatagram field");
            let value = self
                .builder
                .build_load(pointer_type, rooted, &format!("{name}.load"))
                .expect("load rooted UDPDatagram field");
            self.builder
                .build_store(field, value)
                .expect("store UDPDatagram field");
        }

        let result_layout = self.module.enums[enum_id as usize].clone();
        let result_type = enum_struct(
            self.context,
            &result_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = self
            .builder
            .build_insert_value(result_type.const_zero(), kind, 0, "udp.receive.kind")
            .expect("insert UDP result tag")
            .into_struct_value();
        let value_field = result_layout.variants[0][0];
        let built = self
            .builder
            .build_insert_value(
                built,
                object,
                value_field + ENUM_HEADER_FIELDS,
                "udp.receive.datagram",
            )
            .expect("insert UDPDatagram")
            .into_struct_value();
        let error_field = result_layout.variants[1][0];
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "udp.receive.error",
            )
            .expect("insert UDP receive error")
            .into_struct_value()
            .into()
    }

    // This mirrors the HTTP result layout fields; keeping them explicit makes
    // the GC-rooting and protocol metadata writes visible at the call site.
    #[allow(clippy::too_many_arguments)]
    fn build_http_get_result_with_protocol(
        &mut self,
        enum_id: u32,
        response_class_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        status: IntValue<'ctx>,
        response_buffers: (PointerValue<'ctx>, PointerValue<'ctx>),
        protocol: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let (body, headers) = response_buffers;
        let ptr_type = self.context.ptr_type(AddressSpace::default());

        // The response body was allocated by the runtime adapter. Root it
        // across the response-object allocation, which may trigger a GC.
        let rooted_body = self
            .builder
            .build_alloca(ptr_type, "http.get.body.root")
            .expect("allocate HTTP body root slot");
        self.builder
            .build_store(rooted_body, body)
            .expect("store HTTP body root");
        let rooted_headers = self
            .builder
            .build_alloca(ptr_type, "http.get.headers.root")
            .expect("allocate HTTP headers root slot");
        self.builder
            .build_store(rooted_headers, headers)
            .expect("store HTTP headers root");
        let roots_type = ptr_type.array_type(2);
        let roots = self
            .builder
            .build_alloca(roots_type, "http.get.roots")
            .expect("allocate HTTP root array");
        let root_entry = unsafe {
            self.builder.build_gep(
                roots_type,
                roots,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "http.get.root.entry",
            )
        }
        .expect("index HTTP root array");
        self.builder
            .build_store(root_entry, rooted_body)
            .expect("store HTTP root address");
        let header_root_entry = unsafe {
            self.builder.build_gep(
                roots_type,
                roots,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_int(1, false),
                ],
                "http.get.headers.root.entry",
            )
        }
        .expect("index HTTP headers root array");
        self.builder
            .build_store(header_root_entry, rooted_headers)
            .expect("store HTTP headers root address");
        self.builder
            .build_call(
                self.runtime.push_frame,
                &[
                    roots.into(),
                    self.context.i64_type().const_int(2, false).into(),
                ],
                "http.get.push.roots",
            )
            .expect("push temporary HTTP response roots");

        let layout = &self.module.objects[response_class_id as usize];
        // HTTPResponse adds consumption, client stream, and server producer
        // state plus optional SSE heartbeat metadata after buffered values.
        debug_assert_eq!(layout.fields.len(), 8);
        let object_type = object_struct(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let object = self
            .builder
            .build_call(
                self.runtime.alloc,
                &[
                    object_type
                        .size_of()
                        .expect("HTTP response object size")
                        .into(),
                    object_type.get_alignment().into(),
                ],
                "http.response.object",
            )
            .expect("allocate HTTPResponse")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        self.builder
            .build_store(object, self.descriptors[response_class_id as usize])
            .expect("write HTTPResponse type descriptor");
        let status_field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS,
                "http.response.status",
            )
            .expect("address HTTPResponse status field");
        self.builder
            .build_store(status_field, status)
            .expect("store HTTPResponse status");
        let body_field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS + 1,
                "http.response.body",
            )
            .expect("address HTTPResponse body field");
        let rooted_body_value = self
            .builder
            .build_load(ptr_type, rooted_body, "http.get.body.rooted")
            .expect("reload rooted HTTP body");
        self.builder
            .build_store(body_field, rooted_body_value)
            .expect("store HTTPResponse body");
        let headers_field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS + 2,
                "http.response.headers",
            )
            .expect("address HTTPResponse headers field");
        let rooted_headers_value = self
            .builder
            .build_load(ptr_type, rooted_headers, "http.get.headers.rooted")
            .expect("reload rooted HTTP headers");
        self.builder
            .build_store(headers_field, rooted_headers_value)
            .expect("store HTTPResponse headers");
        let protocol_field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS + 7,
                "http.response.protocol",
            )
            .expect("address HTTPResponse protocol field");
        self.builder
            .build_store(protocol_field, protocol)
            .expect("store HTTPResponse protocol");
        self.builder
            .build_call(self.runtime.pop_frame, &[], "http.get.pop.roots")
            .expect("pop temporary HTTP response roots");

        let result_layout = self.module.enums[enum_id as usize].clone();
        let result_type = enum_struct(
            self.context,
            &result_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = self
            .builder
            .build_insert_value(result_type.const_zero(), kind, 0, "http.result.kind")
            .expect("insert HTTP Result discriminant")
            .into_struct_value();
        let response_field = result_layout.variants[0][0];
        let built = self
            .builder
            .build_insert_value(
                built,
                object,
                response_field + ENUM_HEADER_FIELDS,
                "http.result.response",
            )
            .expect("insert HTTP response object")
            .into_struct_value();
        let error_field = result_layout.variants[1][0];
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "http.result.error",
            )
            .expect("insert HTTP error discriminant")
            .into_struct_value()
            .into()
    }

    fn build_http_stream_chunk(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        bytes: PointerValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        debug_assert_eq!(layout.variants.len(), 3);
        let built = self
            .builder
            .build_insert_value(struct_type.const_zero(), kind, 0, "http.chunk.kind")
            .expect("insert HTTPStreamChunk variant")
            .into_struct_value();
        let bytes_field = layout.variants[0][0];
        let built = self
            .builder
            .build_insert_value(
                built,
                bytes,
                bytes_field + ENUM_HEADER_FIELDS,
                "http.chunk.bytes",
            )
            .expect("insert HTTPStreamChunk bytes")
            .into_struct_value();
        let error_field = layout.variants[2][0];
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "http.chunk.error",
            )
            .expect("insert HTTPStreamChunk error")
            .into_struct_value()
            .into()
    }

    fn build_http_stream_open_result(
        &mut self,
        enum_id: u32,
        response_class_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        status_and_handle: (IntValue<'ctx>, IntValue<'ctx>),
        headers: PointerValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let (status, handle) = status_and_handle;
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let rooted_headers = self
            .builder
            .build_alloca(ptr_type, "http.stream.headers.root")
            .expect("allocate streaming header root");
        self.builder
            .build_store(rooted_headers, headers)
            .expect("root streaming response headers");
        let roots_type = ptr_type.array_type(1);
        let roots = self
            .builder
            .build_alloca(roots_type, "http.stream.roots")
            .expect("allocate streaming response roots");
        let root_entry = unsafe {
            self.builder.build_gep(
                roots_type,
                roots,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "http.stream.root.entry",
            )
        }
        .expect("index streaming response root array");
        self.builder
            .build_store(root_entry, rooted_headers)
            .expect("store streaming header root address");
        self.builder
            .build_call(
                self.runtime.push_frame,
                &[
                    roots.into(),
                    self.context.i64_type().const_int(1, false).into(),
                ],
                "http.stream.push.roots",
            )
            .expect("push streaming response roots");

        let layout = &self.module.objects[response_class_id as usize];
        debug_assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "status_code_value")
        );
        debug_assert!(layout.fields.iter().any(|field| field.name == "body_value"));
        debug_assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "header_data")
        );
        debug_assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "body_consumed")
        );
        debug_assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "stream_handle")
        );
        debug_assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "server_producer_handle")
        );
        let object_type = object_struct(
            self.context,
            layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let object = self
            .builder
            .build_call(
                self.runtime.alloc,
                &[
                    object_type
                        .size_of()
                        .expect("HTTP stream response size")
                        .into(),
                    object_type.get_alignment().into(),
                ],
                "http.response.object",
            )
            .expect("allocate HTTPResponse")
            .try_as_basic_value()
            .basic()
            .expect("runtime allocator returns a pointer")
            .into_pointer_value();
        self.builder
            .build_store(object, self.descriptors[response_class_id as usize])
            .expect("write HTTPResponse type descriptor");

        let store_field =
            |this: &mut Self, field_name: &str, value: BasicValueEnum<'ctx>, label: &str| {
                let index = layout
                    .fields
                    .iter()
                    .position(|field| field.name == field_name)
                    .expect("HTTPResponse layout contains expected field");
                let field = this
                    .builder
                    .build_struct_gep(
                        object_type,
                        object,
                        OBJECT_HEADER_FIELDS + index as u32,
                        label,
                    )
                    .expect("address HTTPResponse field");
                this.builder
                    .build_store(field, value)
                    .expect("store HTTPResponse field");
            };
        store_field(
            self,
            "status_code_value",
            status.into(),
            "http.response.status.field",
        );
        store_field(
            self,
            "body_value",
            ptr_type.const_null().into(),
            "http.response.body.field",
        );
        store_field(
            self,
            "body_consumed",
            self.context.bool_type().const_zero().into(),
            "http.response.consumed.field",
        );
        store_field(
            self,
            "stream_handle",
            handle.into(),
            "http.response.handle.field",
        );
        store_field(
            self,
            "server_producer_handle",
            self.context.i64_type().const_zero().into(),
            "http.response.producer.handle.field",
        );
        store_field(
            self,
            "server_sse_heartbeat_ms",
            self.context.i32_type().const_zero().into(),
            "http.response.sse.heartbeat.field",
        );
        let headers_index = layout
            .fields
            .iter()
            .position(|field| field.name == "header_data")
            .expect("HTTPResponse layout contains headers");
        let headers_field = self
            .builder
            .build_struct_gep(
                object_type,
                object,
                OBJECT_HEADER_FIELDS + headers_index as u32,
                "http.response.headers.field",
            )
            .expect("address HTTPResponse headers field");
        let headers_value = self
            .builder
            .build_load(ptr_type, rooted_headers, "http.stream.headers.rooted")
            .expect("reload rooted HTTP response headers");
        self.builder
            .build_store(headers_field, headers_value)
            .expect("store HTTPResponse headers");
        self.builder
            .build_call(self.runtime.pop_frame, &[], "http.stream.pop.roots")
            .expect("pop streaming response roots");

        let result_layout = self.module.enums[enum_id as usize].clone();
        let result_type = enum_struct(
            self.context,
            &result_layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let mut built = self
            .builder
            .build_insert_value(result_type.const_zero(), kind, 0, "http.stream.result.kind")
            .expect("insert HTTP stream Result discriminant")
            .into_struct_value();
        let response_field = result_layout.variants[0][0];
        built = self
            .builder
            .build_insert_value(
                built,
                object,
                response_field + ENUM_HEADER_FIELDS,
                "http.stream.result.value",
            )
            .expect("insert HTTPResponse into Result")
            .into_struct_value();
        let error_field = result_layout.variants[1][0];
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "http.stream.result.error",
            )
            .expect("insert HTTP stream error")
            .into_struct_value()
            .into()
    }

    fn build_void_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = self
            .builder
            .build_insert_value(struct_type.const_zero(), kind, 0, "network.void.kind")
            .expect("insert network void result tag")
            .into_struct_value();
        let error_field = layout.variants[1][0];
        debug_assert_eq!(
            layout.fields[error_field as usize].ty,
            ir::IrType::Int(ir::IntWidth::I32),
            "TCPError is represented by its i32 discriminant"
        );
        self.builder
            .build_insert_value(
                built,
                error,
                error_field + ENUM_HEADER_FIELDS,
                "network.void.error",
            )
            .expect("insert network void error")
            .into_struct_value()
            .into()
    }

    /// Builds a `Result<Void, IoError>` value from `stdout.flush`/
    /// `stderr.flush`'s own status code (`0` = `Ok`, `1` = `Error`, matching
    /// `register_native_result_enum`'s `Ok = 0, Error = 1` order) — the only
    /// failure this operation can report is `IoError.Io`.
    fn build_flush_result(&mut self, enum_id: u32, status: IntValue<'ctx>) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = struct_type.const_zero();
        let built = self
            .builder
            .build_insert_value(built, status, 0, "flush.kind")
            .expect("insert the discriminant")
            .into_struct_value();
        // The `Error` variant (index 1) carries one field, `error: IoError`,
        // always `Io` — `flush()` reports no other failure. The `Ok` variant
        // (index 0) carries `value: Void`, which — like every `Void` field
        // (`InstKind::BuildEnum`'s own arm) — is left undefined: nothing
        // ever reads it.
        let error_field_index = layout.variants[1][0];
        debug_assert_eq!(
            layout.fields[error_field_index as usize].ty,
            ir::IrType::Int(ir::IntWidth::I32),
            "IoError is a fieldless enum, represented by its i32 discriminant"
        );
        let io_error_zero = self.context.i32_type().const_zero();
        let built = self
            .builder
            .build_insert_value(
                built,
                io_error_zero,
                error_field_index + ENUM_HEADER_FIELDS,
                "flush.error",
            )
            .expect("insert the error field")
            .into_struct_value();
        built.into()
    }

    /// Builds a `ReadResult<T>`/`Result<T, IoError>` value directly from a
    /// `stdin.read_*` host outcome (`add-stdin-stdout-standard-library`,
    /// design D2): `kind` is already the outer discriminant exactly as the
    /// native/interpreter host produced it, `error` is the nested
    /// `IoError` discriminant (meaningful only when `kind` denotes an
    /// error), and `payload` is an already-built `String`/`Char`/
    /// `Array<Byte>` handle (meaningful only when `kind` denotes a value),
    /// or a null pointer otherwise.
    ///
    /// Both variant fields are populated unconditionally rather than
    /// branching on `kind`: an inactive variant's slot is never read by
    /// verified Zirk code, the same invariant `InstKind::BuildEnum`'s own
    /// codegen already relies on for an omitted field.
    fn build_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        payload: PointerValue<'ctx>,
        value_variant: u32,
        error_variant: u32,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = struct_type.const_zero();
        let built = self
            .builder
            .build_insert_value(built, kind, 0, "stdin.kind")
            .expect("insert the discriminant")
            .into_struct_value();

        let value_field_index = layout.variants[value_variant as usize][0];
        let built = self
            .builder
            .build_insert_value(
                built,
                payload,
                value_field_index + ENUM_HEADER_FIELDS,
                "stdin.value",
            )
            .expect("insert the value field")
            .into_struct_value();

        let error_field_index = layout.variants[error_variant as usize][0];
        debug_assert_eq!(
            layout.fields[error_field_index as usize].ty,
            ir::IrType::Int(ir::IntWidth::I32),
            "IoError is a fieldless enum, represented by its i32 discriminant"
        );
        let built = self
            .builder
            .build_insert_value(
                built,
                error,
                error_field_index + ENUM_HEADER_FIELDS,
                "stdin.error",
            )
            .expect("insert the error field")
            .into_struct_value();
        built.into()
    }

    /// Builds `Result<String?, E>` from an optional native string pointer.
    /// Nullable values use an explicit presence flag in Zirk's ABI.
    fn build_nullable_string_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        error: IntValue<'ctx>,
        payload: PointerValue<'ctx>,
        value_variant: u32,
        error_variant: u32,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let nullable_type = self
            .llvm_type(ir::IrType::Nullable(ir::Nullable::String))
            .expect("nullable String has an LLVM representation")
            .into_struct_type();
        let present = self
            .builder
            .build_is_not_null(payload, "tls.alpn.present")
            .expect("compare optional ALPN pointer");
        let nullable = self
            .builder
            .build_insert_value(nullable_type.get_undef(), present, 0, "tls.alpn.presence")
            .expect("insert nullable ALPN presence")
            .into_struct_value();
        let nullable = self
            .builder
            .build_insert_value(nullable, payload, 1, "tls.alpn.value")
            .expect("insert nullable ALPN string")
            .into_struct_value();
        let built = self
            .builder
            .build_insert_value(struct_type.const_zero(), kind, 0, "tls.alpn.kind")
            .expect("insert TLS ALPN result discriminant")
            .into_struct_value();
        let value_field = layout.variants[value_variant as usize][0] + ENUM_HEADER_FIELDS;
        let built = self
            .builder
            .build_insert_value(built, nullable, value_field, "tls.alpn.result")
            .expect("insert nullable ALPN result")
            .into_struct_value();
        let error_field = layout.variants[error_variant as usize][0] + ENUM_HEADER_FIELDS;
        self.builder
            .build_insert_value(built, error, error_field, "tls.alpn.error")
            .expect("insert TLS ALPN error")
            .as_basic_value_enum()
    }

    /// Builds `Result<Boolean, EnvironmentError>` from an environment
    /// presence check. The native `detail` field is the boolean value for an
    /// `Ok` result and the error discriminant for an `Error` result.
    fn build_boolean_tagged_outcome(
        &mut self,
        enum_id: u32,
        kind: IntValue<'ctx>,
        detail: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let layout = self.module.enums[enum_id as usize].clone();
        let struct_type = enum_struct(
            self.context,
            &layout,
            &self.module.closures,
            &self.module.values,
            &self.module.enums,
        );
        let built = struct_type.const_zero();
        let built = self
            .builder
            .build_insert_value(built, kind, 0, "environment.contains.kind")
            .expect("insert the result discriminant")
            .into_struct_value();
        let bool_value = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                detail,
                self.context.i32_type().const_zero(),
                "environment.contains.value",
            )
            .expect("compare presence value");
        let value_field = layout.variants[0][0] + ENUM_HEADER_FIELDS;
        let built = self
            .builder
            .build_insert_value(built, bool_value, value_field, "environment.contains.ok")
            .expect("insert the Boolean result")
            .into_struct_value();
        let error_field = layout.variants[1][0] + ENUM_HEADER_FIELDS;
        let built = self
            .builder
            .build_insert_value(built, detail, error_field, "environment.contains.error")
            .expect("insert the environment error discriminant")
            .into_struct_value();
        built.into()
    }

    /// Converts an `fp128` value to `double` by decoding its IEEE 754
    /// binary128 bit pattern in `zirk-runtime` (`zirk_float128_to_f64`),
    /// not through LLVM's native `fptrunc fp128 to double`: that
    /// instruction depends on the target's quad-precision libcalls
    /// (`__trunctfdf2`), which are unreliable on Windows/MSVC (confirmed:
    /// `1.5_Float128` silently truncated to `0`). The `bitcast` to `i128`
    /// is always exact — same bit width, no arithmetic — so the value
    /// crosses the FFI boundary by the same pointer-passing convention
    /// `declare_extern_fn` uses for `zirk_int_*`/128-bit values.
    fn emit_f128_to_f64(&mut self, value: FloatValue<'ctx>) -> FloatValue<'ctx> {
        let bits = self
            .builder
            .build_bit_cast(value, self.context.i128_type(), "f128.bits")
            .expect("bitcasting f128 to i128");
        let slot = self.aligned_alloca(self.context.i128_type().into(), "f128.bits.slot");
        self.builder
            .build_store(slot, bits)
            .expect("store f128 bits");
        let call = self
            .builder
            .build_call(self.runtime.float128_to_f64, &[slot.into()], "f128.to_f64")
            .expect("call zirk_float128_to_f64");
        call.try_as_basic_value()
            .basic()
            .expect("zirk_float128_to_f64 returns a value")
            .into_float_value()
    }

    /// The reverse bridge of [`Self::emit_f128_to_f64`]: widens a `double`
    /// to `fp128` by encoding its bit pattern in `zirk-runtime`
    /// (`zirk_f64_to_float128`), sidestepping LLVM's native `fpext double
    /// to fp128` for the same reason.
    fn emit_f64_to_f128(&mut self, value: FloatValue<'ctx>) -> FloatValue<'ctx> {
        let slot = self.aligned_alloca(self.context.i128_type().into(), "f128.from_f64.slot");
        self.builder
            .build_call(
                self.runtime.f64_to_float128,
                &[slot.into(), value.into()],
                "f64.to_f128",
            )
            .expect("call zirk_f64_to_float128");
        let bits = self
            .builder
            .build_load(self.context.i128_type(), slot, "f128.from_f64.bits")
            .expect("load f128 bits");
        self.builder
            .build_bit_cast(bits, self.context.f128_type(), "f128.from_f64")
            .expect("bitcasting i128 to f128")
            .into_float_value()
    }

    /// Whether an exact-`Float` helper carries `ty` across the C boundary by
    /// pointer — every `Decimal` and every 128-bit integer.
    fn decimal_helper_by_pointer(ty: ir::IrType) -> bool {
        matches!(
            ty,
            ir::IrType::Decimal | ir::IrType::Int(ir::IntWidth::I128 | ir::IntWidth::U128)
        )
    }

    /// Calls a `zirk_rt_decimal_*` / `zirk_str_from_decimal` helper, spilling
    /// each by-pointer argument to a stack slot and routing a by-pointer
    /// result through a leading out-parameter — matching
    /// [`declare_extern_fn`]'s `ptr`-shaped declaration.
    fn emit_decimal_call(
        &mut self,
        callee: &str,
        args: &[(BasicValueEnum<'ctx>, ir::IrType)],
        return_ty: ir::IrType,
    ) -> BasicValueEnum<'ctx> {
        let mut actual: Vec<BasicMetadataValueEnum> = Vec::new();
        let returns_by_pointer = Self::decimal_helper_by_pointer(return_ty);

        let out_slot = if returns_by_pointer {
            let ty = self
                .llvm_type(return_ty)
                .expect("a by-pointer decimal result is never Void");
            let slot = self.aligned_alloca(ty, "dec.out");
            actual.push(slot.into());
            Some((slot, ty))
        } else {
            None
        };

        for (value, ty) in args {
            if Self::decimal_helper_by_pointer(*ty) {
                let slot = self.aligned_alloca(value.get_type(), "dec.arg");
                self.builder
                    .build_store(slot, *value)
                    .expect("store a decimal helper argument");
                actual.push(slot.into());
            } else {
                actual.push((*value).into());
            }
        }

        let call = self
            .builder
            .build_call(self.functions[callee], &actual, "dec.call")
            .expect("call to a decimal helper");

        match out_slot {
            Some((slot, ty)) => self
                .builder
                .build_load(ty, slot, "dec.result")
                .expect("load a decimal helper result"),
            None => call
                .try_as_basic_value()
                .basic()
                .expect("this decimal helper returns a value"),
        }
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
                self.builder
                    .build_int_sub(zero, value, "neg")
                    .expect("neg")
                    .into()
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
        if op == Identical
            && left.is_struct_value()
            && matches!(left_ty, ir::IrType::Closure(_) | ir::IrType::Callable(_))
        {
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
            let ir::IrType::Float(width) = left_ty else {
                unreachable!("LLVM float value has a Float IR type")
            };
            return self.emit_float_binary(
                op,
                left.into_float_value(),
                right.into_float_value(),
                width,
            );
        }

        if left_ty == ir::IrType::Decimal
            && matches!(op, Eq | NotEq | Lt | LtEq | Gt | GtEq | Identical)
        {
            let ordering = self.emit_decimal_call(
                "zirk_rt_decimal_cmp",
                &[(left, ir::IrType::Decimal), (right, ir::IrType::Decimal)],
                ir::IrType::Int(ir::IntWidth::I32),
            );
            let predicate = match op {
                Eq | Identical => IntPredicate::EQ,
                NotEq => IntPredicate::NE,
                Lt => IntPredicate::SLT,
                LtEq => IntPredicate::SLE,
                Gt => IntPredicate::SGT,
                GtEq => IntPredicate::SGE,
                _ => unreachable!("guarded by decimal comparison operators"),
            };
            return self.compare(
                predicate,
                ordering.into_int_value(),
                self.context.i32_type().const_zero(),
            );
        }

        let l = left.into_int_value();
        let r = right.into_int_value();

        match op {
            // `ZIRK_LANGUAGE_SPEC.md` section 3: ordinary overflow now throws
            // `ArithmeticOverflowError` from `zirk-ir` (`CheckedArithmetic` + a
            // branch), so `Binary` itself is just the raw LLVM operation.
            Add => self.builder.build_int_add(l, r, "add").expect("add").into(),
            Sub => self.builder.build_int_sub(l, r, "sub").expect("sub").into(),
            Mul => self.builder.build_int_mul(l, r, "mul").expect("mul").into(),

            Div | Rem => self.emit_int_div_rem(op, l, r, signed),

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
            ir::IrType::Callable(_) => {
                let l = left.into_struct_value();
                let r = right.into_struct_value();

                let l_fn = self
                    .builder
                    .build_extract_value(l, CALLABLE_FUNCTION_FIELD, "l_fn")
                    .expect("function pointer")
                    .into_pointer_value();
                let r_fn = self
                    .builder
                    .build_extract_value(r, CALLABLE_FUNCTION_FIELD, "r_fn")
                    .expect("function pointer")
                    .into_pointer_value();
                let l_fn_addr = self
                    .builder
                    .build_ptr_to_int(l_fn, self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r_fn_addr = self
                    .builder
                    .build_ptr_to_int(r_fn, self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                let same_fn = self
                    .compare(IntPredicate::EQ, l_fn_addr, r_fn_addr)
                    .into_int_value();

                let l_cap = self
                    .builder
                    .build_extract_value(l, CALLABLE_CAPTURE_FIELD, "l_cap")
                    .expect("capture pointer")
                    .into_pointer_value();
                let r_cap = self
                    .builder
                    .build_extract_value(r, CALLABLE_CAPTURE_FIELD, "r_cap")
                    .expect("capture pointer")
                    .into_pointer_value();
                let l_cap_addr = self
                    .builder
                    .build_ptr_to_int(l_cap, self.context.i64_type(), "lhs")
                    .expect("compare addresses");
                let r_cap_addr = self
                    .builder
                    .build_ptr_to_int(r_cap, self.context.i64_type(), "rhs")
                    .expect("compare addresses");
                let same_cap = self
                    .compare(IntPredicate::EQ, l_cap_addr, r_cap_addr)
                    .into_int_value();

                self.builder
                    .build_and(same_fn, same_cap, "identical")
                    .expect("combine")
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

    /// Overflow flag for an integer `+`, `-`, or `*` (roadmap Phase 4b).
    ///
    /// The matching `InstKind::Binary` computes the raw arithmetic result;
    /// this helper just extracts the `overflowed` bit from the matching
    /// `llvm.*.with.overflow` intrinsic so `zirk-ir`'s `Lower` can branch on
    /// it and throw `ArithmeticOverflowError` itself.
    fn emit_overflowed(
        &mut self,
        op: ir::BinaryOp,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        signed: bool,
    ) -> BasicValueEnum<'ctx> {
        let operation = match (op, signed) {
            (ir::BinaryOp::Add, true) => "sadd",
            (ir::BinaryOp::Add, false) => "uadd",
            (ir::BinaryOp::Sub, true) => "ssub",
            (ir::BinaryOp::Sub, false) => "usub",
            (ir::BinaryOp::Mul, true) => "smul",
            (ir::BinaryOp::Mul, false) => "umul",
            other => unreachable!("CheckedArithmetic does not cover {other:?}"),
        };

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

        self.builder
            .build_extract_value(call.into_struct_value(), 1, "overflowed")
            .expect("overflow flag")
    }

    /// Raw integer division and remainder.
    ///
    /// The divisor and the `MIN / -1` pair are no longer checked here
    /// (`fase-4b-excepciones`): `zirk-ir/src/lower.rs` guards both upstream
    /// with ordinary `Branch`es that throw `DivisionByZeroError` and
    /// `ArithmeticOverflowError`, so `Binary` itself reaches `sdiv`/`srem`
    /// only on safe inputs.
    fn emit_int_div_rem(
        &mut self,
        op: ir::BinaryOp,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        signed: bool,
    ) -> BasicValueEnum<'ctx> {
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
        width: ir::FloatWidth,
    ) -> BasicValueEnum<'ctx> {
        use ir::BinaryOp::*;

        if width == ir::FloatWidth::F128 {
            return self.emit_float128_binary(op, left, right);
        }

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

    fn float128_bits_slot(&mut self, value: FloatValue<'ctx>, name: &str) -> PointerValue<'ctx> {
        let bits = self
            .builder
            .build_bit_cast(value, self.context.i128_type(), &format!("{name}.bits"))
            .expect("bitcast binary128 value to its bit representation");
        let slot = self
            .builder
            .build_alloca(self.context.i128_type(), &format!("{name}.slot"))
            .expect("allocate binary128 bit slot");
        self.builder
            .build_store(slot, bits)
            .expect("store binary128 bits");
        slot
    }

    fn emit_float128_binary(
        &mut self,
        op: ir::BinaryOp,
        left: FloatValue<'ctx>,
        right: FloatValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        use ir::BinaryOp::*;
        let left = self.float128_bits_slot(left, "f128.left");
        let right = self.float128_bits_slot(right, "f128.right");
        let (operation, is_comparison) = match op {
            Add => (0, false),
            Sub => (1, false),
            Mul => (2, false),
            Div => (3, false),
            Rem => (4, false),
            Eq => (0, true),
            NotEq => (1, true),
            Lt => (2, true),
            LtEq => (3, true),
            Gt => (4, true),
            GtEq => (5, true),
            Identical | And | Or | BitAnd | BitOr | BitXor | Shl | Shr => {
                unreachable!("operator {op:?} over Float128")
            }
        };
        if is_comparison {
            let call = self
                .builder
                .build_call(
                    self.runtime.float128_compare,
                    &[
                        self.context.i32_type().const_int(operation, false).into(),
                        left.into(),
                        right.into(),
                    ],
                    "f128.compare",
                )
                .expect("call portable Float128 comparison helper")
                .try_as_basic_value()
                .basic()
                .expect("Float128 comparison returns a Boolean status")
                .into_int_value();
            return self
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    call,
                    self.context.i32_type().const_zero(),
                    "f128.compare.bool",
                )
                .expect("convert Float128 comparison status to Boolean")
                .into();
        }

        let output = self
            .builder
            .build_alloca(self.context.i128_type(), "f128.result.slot")
            .expect("allocate Float128 result bits");
        let status = self
            .builder
            .build_call(
                self.runtime.float128_binary,
                &[
                    self.context.i32_type().const_int(operation, false).into(),
                    left.into(),
                    right.into(),
                    output.into(),
                ],
                "f128.binary.status",
            )
            .expect("call portable Float128 arithmetic helper")
            .try_as_basic_value()
            .basic()
            .expect("Float128 arithmetic returns status")
            .into_int_value();
        let valid = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                status,
                self.context.i32_type().const_zero(),
                "f128.binary.valid",
            )
            .expect("compare Float128 arithmetic status");
        let function = self
            .builder
            .get_insert_block()
            .expect("Float128 arithmetic emitted inside a block")
            .get_parent()
            .expect("Float128 arithmetic block belongs to a function");
        let success = self
            .context
            .append_basic_block(function, "f128.binary.success");
        let failure = self
            .context
            .append_basic_block(function, "f128.binary.failure");
        self.builder
            .build_conditional_branch(valid, success, failure)
            .expect("branch on Float128 arithmetic status");
        self.builder.position_at_end(failure);
        self.builder
            .build_call(self.runtime.invalid_cast, &[], "f128.binary.failure")
            .expect("report unsupported Float128 arithmetic result");
        self.builder
            .build_unreachable()
            .expect("invalid Float128 arithmetic handler does not return");
        self.builder.position_at_end(success);
        let bits = self
            .builder
            .build_load(self.context.i128_type(), output, "f128.result.bits")
            .expect("load Float128 result bits");
        self.builder
            .build_bit_cast(bits, self.context.f128_type(), "f128.result")
            .expect("restore binary128 floating value")
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
            ir::Terminator::Cancel => {
                self.pop_gc_frame();
                let function = self
                    .builder
                    .get_insert_block()
                    .expect("cancellation block exists")
                    .get_parent()
                    .expect("cancellation block has a function");
                let result = function
                    .get_type()
                    .get_return_type()
                    .map(|ty| ty.const_zero());
                self.builder
                    .build_return(result.as_ref().map(|value| value as &dyn BasicValue))
                    .expect("return cancellation marker");
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
            ir::Terminator::UnionDispatch {
                value,
                union_id,
                targets,
            } => {
                let layout = &self.module.union_types[*union_id as usize];
                let enum_id = layout
                    .representation_layout
                    .expect("verified executable union");
                let tag = self
                    .builder
                    .build_extract_value(
                        self.operand(*value).into_struct_value(),
                        0,
                        "union.dispatch.tag",
                    )
                    .expect("extract union dispatch tag")
                    .into_int_value();
                let function = self
                    .builder
                    .get_insert_block()
                    .expect("dispatch block")
                    .get_parent()
                    .expect("dispatch function");
                let invalid = self
                    .context
                    .append_basic_block(function, "invalid.union.tag");
                let cases: Vec<_> = targets
                    .iter()
                    .enumerate()
                    .map(|(index, target)| {
                        (
                            self.context.i32_type().const_int(index as u64, false),
                            self.blocks[target],
                        )
                    })
                    .collect();
                self.builder
                    .build_switch(tag, invalid, &cases)
                    .expect("emit union dispatch switch");
                self.builder.position_at_end(invalid);
                self.builder
                    .build_unreachable()
                    .expect("invalid union tag is unreachable");
                let _ = enum_id;
            }
        }
    }
}
