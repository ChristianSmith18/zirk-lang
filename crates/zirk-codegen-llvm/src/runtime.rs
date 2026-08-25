//! Declaration of the runtime symbols the generated code calls.
//!
//! These are the C ABI boundary of `docs/decisions/ADR-002-runtime-staticlib.md`.
//! Codegen declares them and calls them; the implementation lives in
//! `zirk-runtime` and is resolved at link time.
//!
//! The names are a compatibility surface: changing one breaks already compiled
//! binaries.

use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::FunctionValue;

/// Names of the runtime symbols, in one place so codegen and tests agree.
pub mod symbols {
    /// Initializes the runtime before `main`.
    pub const INIT: &str = "zirk_rt_init";
    /// Shuts the runtime down after `main`.
    pub const SHUTDOWN: &str = "zirk_rt_shutdown";
    /// Builds a `String` from UTF-8 bytes and a length.
    pub const STR_FROM_UTF8: &str = "zirk_str_from_utf8";
    /// Converts an `Int8` into a `String`.
    pub const STR_FROM_I8: &str = "zirk_str_from_i8";
    /// Converts an `Int16` into a `String`.
    pub const STR_FROM_I16: &str = "zirk_str_from_i16";
    /// Converts an `Int32` into a `String`.
    pub const STR_FROM_I32: &str = "zirk_str_from_i32";
    /// Converts an `Int64` into a `String`.
    pub const STR_FROM_I64: &str = "zirk_str_from_i64";
    /// Converts an `Int128` into a `String`, taken by pointer (no stable
    /// cross-target ABI for a by-value 128-bit integer).
    pub const STR_FROM_I128: &str = "zirk_str_from_i128";
    /// Converts a `UInt8` into a `String`.
    pub const STR_FROM_U8: &str = "zirk_str_from_u8";
    /// Converts a `UInt16` into a `String`.
    pub const STR_FROM_U16: &str = "zirk_str_from_u16";
    /// Converts a `UInt32` into a `String`.
    pub const STR_FROM_U32: &str = "zirk_str_from_u32";
    /// Converts a `UInt64` into a `String`.
    pub const STR_FROM_U64: &str = "zirk_str_from_u64";
    /// Converts a `UInt128` into a `String`, taken by pointer — see
    /// `STR_FROM_I128`.
    pub const STR_FROM_U128: &str = "zirk_str_from_u128";
    /// Converts a `Float32` into a `String`.
    pub const STR_FROM_F32: &str = "zirk_str_from_f32";
    /// Converts a `Float64` into a `String`.
    pub const STR_FROM_F64: &str = "zirk_str_from_f64";
    /// Converts a `Boolean` into a `String`.
    pub const STR_FROM_BOOL: &str = "zirk_str_from_bool";
    /// Byte length of the grapheme at a given offset, or `-1` past the end
    /// (roadmap Phase 3b, task 6.3: `for ... in` over `String`).
    pub const STR_GRAPHEME_LEN_AT: &str = "zirk_str_grapheme_len_at";
    /// Builds a `Char` from a byte range already known to be one grapheme.
    pub const STR_GRAPHEME_SLICE: &str = "zirk_str_grapheme_slice";
    /// Structural equality of two strings.
    pub const STR_EQ: &str = "zirk_str_eq";
    /// Writes a `String` to standard output with a line break.
    pub const IO_PRINTLN: &str = "zirk_io_println";
    /// Reports an arithmetic overflow and terminates.
    pub const OVERFLOW: &str = "zirk_rt_overflow";
    /// Reports a division by zero and terminates.
    pub const DIVISION_BY_ZERO: &str = "zirk_rt_division_by_zero";
    /// Obtains storage for an object. The strategy behind it is the runtime's
    /// (ADR-003), which is why the IR only ever says `alloc <type>`.
    pub const ALLOC: &str = "zirk_rt_alloc";
    /// Reports that an object could not be allocated and terminates.
    pub const ALLOCATION_FAILED: &str = "zirk_rt_allocation_failed";
    /// Concatenates two strings.
    pub const STR_CONCAT: &str = "zirk_str_concat";
    /// Repeats a string a non-negative number of times.
    pub const STR_REPEAT: &str = "zirk_str_repeat";
    /// Reports an invalid repetition count and terminates.
    pub const INVALID_REPEAT: &str = "zirk_rt_invalid_repeat";
    /// Reports a shift by a negative amount or by too much and terminates.
    pub const INVALID_SHIFT: &str = "zirk_rt_invalid_shift";
    /// Finds the dispatch table a descriptor holds for a contract.
    pub const CONTRACT_TABLE: &str = "zirk_rt_contract_table";
    /// Reports a descriptor missing a contract it was said to satisfy.
    pub const MISSING_CONTRACT: &str = "zirk_rt_missing_contract";
    /// Confirms a checked cast against a descriptor's ancestor list,
    /// terminating if it is not one of them.
    pub const CHECK_CAST: &str = "zirk_rt_check_cast";
    /// Reports a checked cast whose runtime type does not match and
    /// terminates.
    pub const INVALID_CAST: &str = "zirk_rt_invalid_cast";
    /// Reports a `Float` operation that would produce `NaN` and terminates
    /// (roadmap Phase 3b, design decision D2: `Float` prohibits `NaN` in the
    /// type, not after the fact — an infinite result is a valid value,
    /// unlike `NaN`).
    pub const FLOAT_NAN: &str = "zirk_rt_float_nan";
    /// Reports a program-supplied `fatalError(message)` and terminates
    /// (roadmap Phase 4a).
    pub const FATAL_ERROR: &str = "zirk_rt_fatal_error";
    /// Records the pending exception (roadmap Phase 4b) — `throw`'s own
    /// lowering.
    pub const THROW: &str = "zirk_rt_throw";
    /// Whether an exception is pending (roadmap Phase 4b) — checked after
    /// every call to a function that can throw.
    pub const HAS_PENDING_EXCEPTION: &str = "zirk_rt_has_pending_exception";
    /// Takes the pending exception, clearing the slot (roadmap Phase 4b) —
    /// a matching `catch`'s own lowering.
    pub const TAKE_PENDING_EXCEPTION: &str = "zirk_rt_take_pending_exception";
    /// Tests a descriptor's ancestor list against a target class, without
    /// terminating when it does not match (roadmap Phase 4b) — a `catch`
    /// clause's own coverage test.
    pub const IS_INSTANCE: &str = "zirk_rt_is_instance";
    /// Reports an exception that escaped `main` uncaught and terminates
    /// (roadmap Phase 4b).
    pub const UNCAUGHT_EXCEPTION: &str = "zirk_rt_uncaught_exception";
    /// Pushes this function activation's shadow-stack frame
    /// (`fase-4e-colector-mark-sweep`, design D2) — called once at function
    /// entry, after every reference-typed slot has been zero-initialized
    /// (design D5).
    pub const PUSH_FRAME: &str = "zirk_rt_push_frame";
    /// Pops the shadow-stack frame [`PUSH_FRAME`] pushed — called immediately
    /// before every `Terminator::Return` lowers to `ret` (design D2).
    pub const POP_FRAME: &str = "zirk_rt_pop_frame";
}

/// The runtime functions available to generated code.
pub struct Runtime<'ctx> {
    pub init: FunctionValue<'ctx>,
    pub shutdown: FunctionValue<'ctx>,
    pub str_from_utf8: FunctionValue<'ctx>,
    pub str_from_i8: FunctionValue<'ctx>,
    pub str_from_i16: FunctionValue<'ctx>,
    pub str_from_i32: FunctionValue<'ctx>,
    pub str_from_i64: FunctionValue<'ctx>,
    pub str_from_i128: FunctionValue<'ctx>,
    pub str_from_u8: FunctionValue<'ctx>,
    pub str_from_u16: FunctionValue<'ctx>,
    pub str_from_u32: FunctionValue<'ctx>,
    pub str_from_u64: FunctionValue<'ctx>,
    pub str_from_u128: FunctionValue<'ctx>,
    pub str_from_f32: FunctionValue<'ctx>,
    pub str_from_f64: FunctionValue<'ctx>,
    pub str_from_bool: FunctionValue<'ctx>,
    pub str_grapheme_len_at: FunctionValue<'ctx>,
    pub str_grapheme_slice: FunctionValue<'ctx>,
    pub str_eq: FunctionValue<'ctx>,
    pub io_println: FunctionValue<'ctx>,
    pub overflow: FunctionValue<'ctx>,
    pub alloc: FunctionValue<'ctx>,
    pub contract_table: FunctionValue<'ctx>,
    pub str_concat: FunctionValue<'ctx>,
    pub str_repeat: FunctionValue<'ctx>,
    pub check_cast: FunctionValue<'ctx>,
    pub fatal_error: FunctionValue<'ctx>,
    pub throw: FunctionValue<'ctx>,
    pub has_pending_exception: FunctionValue<'ctx>,
    pub take_pending_exception: FunctionValue<'ctx>,
    pub is_instance: FunctionValue<'ctx>,
    pub uncaught_exception: FunctionValue<'ctx>,
    pub push_frame: FunctionValue<'ctx>,
    pub pop_frame: FunctionValue<'ctx>,
}

/// Declares every runtime symbol in the module.
pub fn declare<'ctx>(context: &'ctx Context, module: &Module<'ctx>) -> Runtime<'ctx> {
    let void = context.void_type();
    let i64 = context.i64_type();
    let ptr = context.ptr_type(AddressSpace::default());

    let external = Some(Linkage::External);

    let init = module.add_function(symbols::INIT, void.fn_type(&[], false), external);
    let shutdown = module.add_function(symbols::SHUTDOWN, void.fn_type(&[], false), external);

    let str_from_utf8 = module.add_function(
        symbols::STR_FROM_UTF8,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );

    let str_from_i8 = module.add_function(
        symbols::STR_FROM_I8,
        ptr.fn_type(&[context.i8_type().into()], false),
        external,
    );
    let str_from_i16 = module.add_function(
        symbols::STR_FROM_I16,
        ptr.fn_type(&[context.i16_type().into()], false),
        external,
    );
    let str_from_i32 = module.add_function(
        symbols::STR_FROM_I32,
        ptr.fn_type(&[context.i32_type().into()], false),
        external,
    );
    let str_from_i64 = module.add_function(
        symbols::STR_FROM_I64,
        ptr.fn_type(&[context.i64_type().into()], false),
        external,
    );
    // Taken by pointer: no stable cross-target ABI for a by-value 128-bit
    // integer (`zirk-runtime/src/string.rs`'s own doc comment on the
    // handler explains why).
    let str_from_i128 = module.add_function(
        symbols::STR_FROM_I128,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let str_from_u8 = module.add_function(
        symbols::STR_FROM_U8,
        ptr.fn_type(&[context.i8_type().into()], false),
        external,
    );
    let str_from_u16 = module.add_function(
        symbols::STR_FROM_U16,
        ptr.fn_type(&[context.i16_type().into()], false),
        external,
    );
    let str_from_u32 = module.add_function(
        symbols::STR_FROM_U32,
        ptr.fn_type(&[context.i32_type().into()], false),
        external,
    );
    let str_from_u64 = module.add_function(
        symbols::STR_FROM_U64,
        ptr.fn_type(&[context.i64_type().into()], false),
        external,
    );
    let str_from_u128 = module.add_function(
        symbols::STR_FROM_U128,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let str_from_f32 = module.add_function(
        symbols::STR_FROM_F32,
        ptr.fn_type(&[context.f32_type().into()], false),
        external,
    );
    let str_from_f64 = module.add_function(
        symbols::STR_FROM_F64,
        ptr.fn_type(&[context.f64_type().into()], false),
        external,
    );

    let str_from_bool = module.add_function(
        symbols::STR_FROM_BOOL,
        ptr.fn_type(&[context.bool_type().into()], false),
        external,
    );

    let str_grapheme_len_at = module.add_function(
        symbols::STR_GRAPHEME_LEN_AT,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let str_grapheme_slice = module.add_function(
        symbols::STR_GRAPHEME_SLICE,
        ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );

    let str_eq = module.add_function(
        symbols::STR_EQ,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );

    let io_println = module.add_function(
        symbols::IO_PRINTLN,
        void.fn_type(&[ptr.into()], false),
        external,
    );

    // The failure handlers never return: marking them `noreturn` lets LLVM
    // treat the code after them as unreachable and optimize accordingly.
    let overflow = module.add_function(symbols::OVERFLOW, void.fn_type(&[], false), external);
    let division_by_zero = module.add_function(
        symbols::DIVISION_BY_ZERO,
        void.fn_type(&[], false),
        external,
    );

    let alloc = module.add_function(
        symbols::ALLOC,
        ptr.fn_type(&[i64.into(), i64.into()], false),
        external,
    );
    // Declared so the allocator can reach it, and marked `noreturn` with the
    // rest: generated code never calls it directly, the runtime does.
    let allocation_failed = module.add_function(
        symbols::ALLOCATION_FAILED,
        void.fn_type(&[], false),
        external,
    );

    let str_concat = module.add_function(
        symbols::STR_CONCAT,
        ptr.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let str_repeat = module.add_function(
        symbols::STR_REPEAT,
        ptr.fn_type(&[ptr.into(), context.i32_type().into()], false),
        external,
    );
    let invalid_repeat =
        module.add_function(symbols::INVALID_REPEAT, void.fn_type(&[], false), external);
    let invalid_shift =
        module.add_function(symbols::INVALID_SHIFT, void.fn_type(&[], false), external);

    let contract_table = module.add_function(
        symbols::CONTRACT_TABLE,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let missing_contract = module.add_function(
        symbols::MISSING_CONTRACT,
        void.fn_type(&[], false),
        external,
    );

    let check_cast = module.add_function(
        symbols::CHECK_CAST,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let invalid_cast =
        module.add_function(symbols::INVALID_CAST, void.fn_type(&[], false), external);
    let float_nan = module.add_function(symbols::FLOAT_NAN, void.fn_type(&[], false), external);
    let fatal_error = module.add_function(
        symbols::FATAL_ERROR,
        void.fn_type(&[ptr.into()], false),
        external,
    );

    let throw = module.add_function(symbols::THROW, void.fn_type(&[ptr.into()], false), external);
    let bool_ty = context.bool_type();
    let has_pending_exception = module.add_function(
        symbols::HAS_PENDING_EXCEPTION,
        bool_ty.fn_type(&[], false),
        external,
    );
    let take_pending_exception = module.add_function(
        symbols::TAKE_PENDING_EXCEPTION,
        ptr.fn_type(&[], false),
        external,
    );
    let is_instance = module.add_function(
        symbols::IS_INSTANCE,
        bool_ty.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let uncaught_exception = module.add_function(
        symbols::UNCAUGHT_EXCEPTION,
        void.fn_type(&[], false),
        external,
    );

    // `roots`: the address of an array of root addresses (design D2) — each
    // element is itself the address of a slot (or a slot's inner
    // reference-typed field) holding a managed reference, not the reference
    // itself, so the collector dereferences once more to reach the candidate
    // object. `count`: how many elements that array has.
    let push_frame = module.add_function(
        symbols::PUSH_FRAME,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let pop_frame = module.add_function(symbols::POP_FRAME, void.fn_type(&[], false), external);

    for handler in [
        overflow,
        division_by_zero,
        allocation_failed,
        missing_contract,
        invalid_repeat,
        invalid_cast,
        invalid_shift,
        float_nan,
        fatal_error,
        uncaught_exception,
    ] {
        let noreturn = context.create_enum_attribute(
            inkwell::attributes::Attribute::get_named_enum_kind_id("noreturn"),
            0,
        );
        handler.add_attribute(inkwell::attributes::AttributeLoc::Function, noreturn);
    }

    Runtime {
        init,
        shutdown,
        str_from_utf8,
        str_from_i8,
        str_from_i16,
        str_from_i32,
        str_from_i64,
        str_from_i128,
        str_from_u8,
        str_from_u16,
        str_from_u32,
        str_from_u64,
        str_from_u128,
        str_from_f32,
        str_from_f64,
        str_from_bool,
        str_grapheme_len_at,
        str_grapheme_slice,
        str_eq,
        io_println,
        overflow,
        alloc,
        contract_table,
        str_concat,
        str_repeat,
        check_cast,
        fatal_error,
        throw,
        has_pending_exception,
        take_pending_exception,
        is_instance,
        uncaught_exception,
        push_frame,
        pop_frame,
    }
}
