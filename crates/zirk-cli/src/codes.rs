use zirk_diagnostics::Code;

/// The source file could not be read.
pub const UNREADABLE_FILE: Code = Code::new("E0501");
/// The output directory could not be prepared.
pub const OUTPUT_UNAVAILABLE: Code = Code::new("E0502");
/// The linker failed or could not be invoked.
pub const LINK_FAILED: Code = Code::new("E0503");
/// The runtime static library was not found.
pub const RUNTIME_NOT_FOUND: Code = Code::new("E0504");
/// The produced executable could not be run.
pub const EXECUTION_FAILED: Code = Code::new("E0505");
/// A subcommand that arrives in a later phase.
pub const NOT_IMPLEMENTED: Code = Code::new("E0506");
/// Wrong invocation of the CLI.
pub const INVALID_USAGE: Code = Code::new("E0507");
/// The compiler produced invalid IR: a compiler bug.
pub const INTERNAL_ERROR: Code = Code::new("E0508");
/// The imports of a crate form a cycle.
pub const IMPORT_CYCLE: Code = Code::new("E0509");
