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
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};
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

/// Translates an IR module into an LLVM module.
pub fn emit<'ctx>(context: &'ctx Context, module: &ir::Module, name: &str) -> LlvmModule<'ctx> {
    let llvm = context.create_module(name);
    let builder = context.create_builder();

    let runtime = runtime::declare(context, &llvm);

    // Every function is declared before any body is emitted, so a call can
    // reference a function defined further down the file.
    let mut functions = HashMap::new();
    for function in &module.functions {
        let declared = declare_function(context, &llvm, function);
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

    for function in &module.functions {
        FunctionEmitter {
            context,
            builder: &builder,
            llvm: &llvm,
            module,
            runtime: &runtime,
            functions: &functions,
            strings: &strings,
            values: HashMap::new(),
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

fn llvm_type<'ctx>(context: &'ctx Context, ty: ir::IrType) -> Option<BasicTypeEnum<'ctx>> {
    Some(match ty {
        ir::IrType::Void => return None,
        ir::IrType::Int32 => context.i32_type().into(),
        // `Boolean` is `i1`: LLVM's natural type for a condition, and what a
        // conditional branch expects.
        ir::IrType::Boolean => context.bool_type().into(),
        // `String` is an opaque pointer. Its layout belongs to the runtime.
        ir::IrType::String => context.ptr_type(AddressSpace::default()).into(),
    })
}

fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    function: &ir::Function,
) -> FunctionValue<'ctx> {
    let params: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|slot| {
            llvm_type(context, function.slots[slot.0 as usize].ty)
                .expect("a parameter cannot be Void")
                .into()
        })
        .collect();

    let signature = match llvm_type(context, function.return_type) {
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

    values: HashMap<ir::ValueId, BasicValueEnum<'ctx>>,
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
            let ty = llvm_type(self.context, slot.ty).expect("a slot cannot be Void");
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
                let ty = llvm_type(self.context, instruction.ty).expect("a load cannot be Void");
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
        };

        if let (Some(result), Some(value)) = (instruction.result, value) {
            self.values.insert(result, value);
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

    /// Division and remainder, checking the divisor.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 9 requires that a division by zero never
    /// become undefined behaviour, and in LLVM `sdiv` by zero is exactly that.
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
