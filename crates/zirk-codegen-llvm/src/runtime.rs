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
    /// Writes a `String` to standard output with a line break.
    pub const IO_PRINTLN: &str = "zirk_io_println";
    /// Reports an arithmetic overflow and terminates.
    pub const OVERFLOW: &str = "zirk_rt_overflow";
    /// Reports a division by zero and terminates.
    pub const DIVISION_BY_ZERO: &str = "zirk_rt_division_by_zero";
}

/// The runtime functions available to generated code.
pub struct Runtime<'ctx> {
    pub init: FunctionValue<'ctx>,
    pub shutdown: FunctionValue<'ctx>,
    pub str_from_utf8: FunctionValue<'ctx>,
    pub io_println: FunctionValue<'ctx>,
    pub overflow: FunctionValue<'ctx>,
    pub division_by_zero: FunctionValue<'ctx>,
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

    for handler in [overflow, division_by_zero] {
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
        io_println,
        overflow,
        division_by_zero,
    }
}
