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
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, PointerValue,
};
use inkwell::{AddressSpace, IntPredicate};
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
        ir::IrType::Int32 => context.i32_type().into(),
        // `Boolean` is `i1`: LLVM's natural type for a condition, and what a
        // conditional branch expects.
        ir::IrType::Boolean => context.bool_type().into(),
        // `String` is an opaque pointer. Its layout belongs to the runtime.
        ir::IrType::String => context.ptr_type(AddressSpace::default()).into(),
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
            let inner = llvm_type_in(context, base.inner(), closures, values, enums)
                .expect("a nullable payload is not Void");
            context
                .struct_type(&[context.bool_type().into(), inner], false)
                .into()
        }
        // A closure is its function pointer followed by its captures, inline.
        // Nothing is allocated: it cannot escape in this phase (D10).
        ir::IrType::Closure(id) => {
            let layout = closures
                .get(id as usize)
                .expect("a verified module declares every closure layout");
            let mut fields: Vec<BasicTypeEnum> =
                vec![context.ptr_type(AddressSpace::default()).into()];
            for capture in &layout.captures {
                fields.push(
                    llvm_type_in(context, *capture, closures, values, enums)
                        .expect("a capture is not Void"),
                );
            }
            context.struct_type(&fields, false).into()
        }
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
        fields.push(
            llvm_type_in(context, field.ty, closures, values, enums)
                .expect("an enum field is not Void"),
        );
    }
    context.struct_type(&fields, false)
}

/// Where a field sits inside an enum's struct, discriminant included.
const ENUM_HEADER_FIELDS: u32 = 1;

/// The struct an object of this layout occupies.
///
/// ```text
///    [ type descriptor | field₁ | field₂ | … ]
/// ```
///
/// The descriptor comes first so every object starts the same way, whatever
/// its fields — which is what will let a subclass share its base's prefix.
fn object_struct<'ctx>(
    context: &'ctx Context,
    layout: &ir::ObjectLayout,
    closures: &[ir::ClosureLayout],
    values: &[ir::ValueLayout],
    enums: &[ir::EnumLayout],
) -> inkwell::types::StructType<'ctx> {
    let mut fields: Vec<BasicTypeEnum> = vec![context.ptr_type(AddressSpace::default()).into()];
    for field in &layout.fields {
        fields.push(
            llvm_type_in(context, field.ty, closures, values, enums)
                .expect("an object field is not Void"),
        );
    }
    context.struct_type(&fields, false)
}

/// Where a field sits inside the struct, header included.
const OBJECT_HEADER_FIELDS: u32 = 1;

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
            llvm_type_in(context, function.slots[slot.0 as usize].ty, closures, values, enums)
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
            let ty = llvm_type_in(self.context, slot.ty, &self.module.closures, &self.module.values, &self.module.enums)
                .expect("a slot cannot be Void");
            let pointer = self
                .builder
                .build_alloca(ty, &slot.name)
                .expect("slot allocation");
            self.slots.insert(ir::SlotId(index as u32), pointer);
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

    /// The address of one field inside an object.
    fn field_pointer(&self, object: ir::Operand, index: u32) -> PointerValue<'ctx> {
        let id = self.object_layout_of(object);
        let layout = &self.module.objects[id as usize];
        let struct_type = object_struct(self.context, layout, &self.module.closures, &self.module.values, &self.module.enums);

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

            // A proven-safe widening (a subclass where its base is
            // expected, or a class where a contract it implements is): the
            // pointer itself is unchanged, only its declared type differs
            // from here on.
            ir::InstKind::Retype(operand) => Some(self.operand(*operand)),

            ir::InstKind::CallVirtual {
                object,
                index,
                args,
            } => {
                let receiver = self.operand(*object).into_pointer_value();
                let id = self.object_layout_of(*object);
                let layout = &self.module.objects[id as usize];

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

                let signature = self.functions[&layout.methods[*index as usize]].get_type();
                let mut arguments: Vec<BasicMetadataValueEnum> = vec![receiver.into()];
                arguments.extend(
                    args.iter()
                        .map(|a| BasicMetadataValueEnum::from(self.operand(*a))),
                );

                let call = self
                    .builder
                    .build_indirect_call(signature, target, &arguments, "call")
                    .expect("call through the table");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::Alloc(id) => {
                let layout = &self.module.objects[*id as usize];
                let struct_type = object_struct(self.context, layout, &self.module.closures, &self.module.values, &self.module.enums);

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
                let struct_type =
                    value_struct(self.context, layout, &self.module.closures, &self.module.values, &self.module.enums);
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
                        self.context.i32_type().const_int(u64::from(*variant), false),
                        0,
                        "discriminant",
                    )
                    .expect("insert the discriminant")
                    .into_struct_value();
                let indices = &layout.variants[*variant as usize];
                for (&index, field) in indices.iter().zip(fields) {
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

            ir::InstKind::Load(slot) => {
                let ty = llvm_type_in(self.context, instruction.ty, &self.module.closures, &self.module.values, &self.module.enums)
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
                Some(self.emit_unary(*op, self.operand(*operand), function))
            }

            ir::InstKind::Binary { op, left, right } => {
                Some(self.emit_binary(*op, self.operand(*left), self.operand(*right), function))
            }

            ir::InstKind::Call { callee, args } => {
                let arguments: Vec<_> = args.iter().map(|a| self.operand(*a).into()).collect();
                let call = self
                    .builder
                    .build_call(self.functions[callee], &arguments, "call")
                    .expect("function call");
                call.try_as_basic_value().basic()
            }

            ir::InstKind::ToString(operand) => {
                // The conversion goes through the runtime: the compiler does
                // not know how a `String` is built (ADR-005).
                let value = self.operand(*operand);
                let converter = match value {
                    BasicValueEnum::IntValue(int) if int.get_type().get_bit_width() == 1 => {
                        self.runtime.str_from_bool
                    }
                    BasicValueEnum::IntValue(_) => self.runtime.str_from_i32,
                    // A `String` needs no conversion; the lowering does not emit
                    // `ToString` over one, so reaching here means malformed IR.
                    other => unreachable!("ToString over {other:?}"),
                };

                let call = self
                    .builder
                    .build_call(converter, &[value.into()], "str")
                    .expect("call to the converter");
                Some(
                    call.try_as_basic_value()
                        .basic()
                        .expect("the converter returns a value"),
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

            // A nullable value is `{ i1 present, T value }`. The absent form
            // still carries a payload slot, left undefined: nothing reads it
            // without checking the flag first, and the verifier enforces that.
            ir::InstKind::NullValue(base) => {
                let ty = llvm_type_in(self.context, ir::IrType::Nullable(*base), &self.module.closures, &self.module.values, &self.module.enums)
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
                let ty = llvm_type_in(self.context, ir::IrType::Nullable(*base), &self.module.closures, &self.module.values, &self.module.enums)
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

            ir::InstKind::MakeClosure { id, captures } => {
                let layout = &self.module.closures[*id as usize];
                let ty = llvm_type_in(
                    self.context,
                    ir::IrType::Closure(*id),
                    &self.module.closures,
                    &self.module.values,
                    &self.module.enums,
                )
                .expect("a closure has a representation")
                .into_struct_type();

                let target = self.functions[layout.function.as_str()];

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
        };

        if let (Some(result), Some(value)) = (instruction.result, value) {
            self.values.insert(result, value);
            // An object is an opaque pointer once emitted, so which layout it
            // belongs to has to be remembered from the IR: it is what a field
            // access needs to know where to point.
            self.value_types.insert(result, instruction.ty);
        }
    }

    fn emit_unary(
        &mut self,
        op: ir::UnaryOp,
        operand: BasicValueEnum<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match op {
            // Negation is a subtraction from zero, so it goes through the same
            // overflow check: `-Int32.MIN` does not fit in Int32.
            ir::UnaryOp::Neg => {
                let zero = self.context.i32_type().const_zero();
                self.checked_arithmetic("ssub", zero, operand.into_int_value(), function)
            }
            ir::UnaryOp::Not => self
                .builder
                .build_not(operand.into_int_value(), "not")
                .expect("logical negation")
                .into(),
        }
    }

    fn emit_binary(
        &mut self,
        op: ir::BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        use ir::BinaryOp::*;

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

        let l = left.into_int_value();
        let r = right.into_int_value();

        match op {
            // `ZIRK_LANGUAGE_SPEC.md` section 3: ordinary overflow produces a
            // controlled error. The wrapping variants are explicit operations
            // that do not exist in this subset, so every arithmetic operation
            // is checked.
            Add => self.checked_arithmetic("sadd", l, r, function),
            Sub => self.checked_arithmetic("ssub", l, r, function),
            Mul => self.checked_arithmetic("smul", l, r, function),

            Div | Rem => self.checked_division(op, l, r, function),

            Eq => self.compare(IntPredicate::EQ, l, r),
            NotEq => self.compare(IntPredicate::NE, l, r),
            Lt => self.compare(IntPredicate::SLT, l, r),
            LtEq => self.compare(IntPredicate::SLE, l, r),
            Gt => self.compare(IntPredicate::SGT, l, r),
            GtEq => self.compare(IntPredicate::SGE, l, r),
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

        let declaration = intrinsic
            .get_declaration(self.llvm, &[self.context.i32_type().into()])
            .expect("the intrinsic accepts i32");

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

    /// Division and remainder, checking the divisor and the one overflow case.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 9 requires that a division by zero never
    /// become undefined behaviour, and in LLVM `sdiv` by zero is exactly that.
    ///
    /// `Int32::MIN / -1` is the other one: its result is one past the maximum,
    /// so it overflows, and in LLVM it is undefined rather than wrapping. It is
    /// the only pair of operands that overflows a division, which is why it is
    /// checked here instead of through the overflow intrinsics.
    fn checked_division(
        &mut self,
        op: ir::BinaryOp,
        left: inkwell::values::IntValue<'ctx>,
        right: inkwell::values::IntValue<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let zero = self.context.i32_type().const_zero();
        let is_zero = self
            .builder
            .build_int_compare(IntPredicate::EQ, right, zero, "is_zero")
            .expect("divisor comparison");

        self.trap_if(is_zero, self.runtime.division_by_zero, function);

        let min = self.context.i32_type().const_int(i32::MIN as u64, true);
        let minus_one = self.context.i32_type().const_all_ones();

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

        match op {
            ir::BinaryOp::Div => self
                .builder
                .build_int_signed_div(left, right, "div")
                .expect("division")
                .into(),
            _ => self
                .builder
                .build_int_signed_rem(left, right, "rem")
                .expect("remainder")
                .into(),
        }
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

    fn emit_terminator(&mut self, terminator: &ir::Terminator) {
        match terminator {
            // LLVM has this exact concept, so nothing is invented here.
            ir::Terminator::Unreachable => {
                self.builder
                    .build_unreachable()
                    .expect("unreachable terminator");
            }
            ir::Terminator::Return(None) => {
                self.builder.build_return(None).expect("empty return");
            }
            ir::Terminator::Return(Some(operand)) => {
                let value = self.operand(*operand);
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
