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
    /// Converts an `Int32` into a `String`.
    pub const STR_FROM_I32: &str = "zirk_str_from_i32";
    /// Converts a `Boolean` into a `String`.
    pub const STR_FROM_BOOL: &str = "zirk_str_from_bool";
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
}

/// The runtime functions available to generated code.
pub struct Runtime<'ctx> {
    pub init: FunctionValue<'ctx>,
    pub shutdown: FunctionValue<'ctx>,
    pub str_from_utf8: FunctionValue<'ctx>,
    pub str_from_i32: FunctionValue<'ctx>,
    pub str_from_bool: FunctionValue<'ctx>,
    pub str_eq: FunctionValue<'ctx>,
    pub io_println: FunctionValue<'ctx>,
    pub overflow: FunctionValue<'ctx>,
    pub division_by_zero: FunctionValue<'ctx>,
    pub alloc: FunctionValue<'ctx>,
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

    let str_from_i32 = module.add_function(
        symbols::STR_FROM_I32,
        ptr.fn_type(&[context.i32_type().into()], false),
        external,
    );

    let str_from_bool = module.add_function(
        symbols::STR_FROM_BOOL,
        ptr.fn_type(&[context.bool_type().into()], false),
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

    for handler in [overflow, division_by_zero, allocation_failed] {
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
        str_from_i32,
        str_from_bool,
        str_eq,
        io_println,
        overflow,
        division_by_zero,
        alloc,
    }
}
