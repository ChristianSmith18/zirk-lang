//! # zirk-codegen-llvm
//!
//! **Responsibility:** translate the `zirk-ir` IR into LLVM IR and emit object
//! files for the targets of `ZIRK_COMPILER_SPEC.md` section 6.
//!
//! **Boundary:** it is the only crate that knows LLVM. That containment is what
//! makes it possible to add another backend later without touching public
//! semantics, as `ZIRK_COMPILER_SPEC.md` section 5 anticipates.
//!
//! Final linking of the executable does **not** happen here: this crate
//! produces objects. Invoking the linker is the job of `zirk-cli`.
//!
//! # State
//!
//! Phase 0 provides the target and emission infrastructure, verified end to
//! end, but not the translation from `zirk-ir` — that IR is still empty.

mod emit;
mod runtime;

pub use emit::emit;
pub use runtime::symbols;

use inkwell::OptimizationLevel;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use std::path::Path;
use std::sync::Once;
use zirk_diagnostics::{Diagnostic, DiagnosticResult};

/// Diagnostic codes of this crate.
pub mod codes {
    use zirk_diagnostics::Code;

    /// The requested triple is not recognized by the linked LLVM.
    pub const UNKNOWN_TARGET: Code = Code::new("E0100");
    /// LLVM could not build a target machine for the triple.
    pub const TARGET_MACHINE_UNAVAILABLE: Code = Code::new("E0101");
    /// Failed to write the object file.
    pub const EMISSION_FAILED: Code = Code::new("E0102");
}

/// Container format of the produced object file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    MachO,
    Elf,
    Coff,
}

/// A target supported by Zirk.
///
/// The canonical name is the one in `ZIRK_COMPILER_SPEC.md` section 6; the
/// triple is its LLVM equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZirkTarget {
    pub name: &'static str,
    pub triple: &'static str,
    pub container: Container,
}

/// The targets from the spec.
///
/// Actual validity depends on the operating system, the linker, the SDK and
/// native dependencies; this table describes what the backend must **emit**,
/// not what can be linked without a sysroot.
///
/// Column alignment is preserved: this is a data table and expanding it to one
/// field per line makes it considerably less readable.
#[rustfmt::skip]
pub const TARGETS: &[ZirkTarget] = &[
    ZirkTarget { name: "x86-windows",     triple: "i686-pc-windows-msvc",          container: Container::Coff },
    ZirkTarget { name: "x86_64-windows",  triple: "x86_64-pc-windows-msvc",        container: Container::Coff },
    ZirkTarget { name: "aarch64-windows", triple: "aarch64-pc-windows-msvc",       container: Container::Coff },
    ZirkTarget { name: "x86-linux",       triple: "i386-unknown-linux-gnu",        container: Container::Elf },
    ZirkTarget { name: "x86_64-linux",    triple: "x86_64-unknown-linux-gnu",      container: Container::Elf },
    ZirkTarget { name: "armv7-linux",     triple: "armv7-unknown-linux-gnueabihf", container: Container::Elf },
    ZirkTarget { name: "aarch64-linux",   triple: "aarch64-unknown-linux-gnu",     container: Container::Elf },
    ZirkTarget { name: "x86_64-macos",    triple: "x86_64-apple-darwin",           container: Container::MachO },
    ZirkTarget { name: "aarch64-macos",   triple: "aarch64-apple-darwin",          container: Container::MachO },
];

/// Emits a backend progress trace when `ZIRK_TRACE` is set.
///
/// It exists because a failure inside LLVM can abort the process leaving
/// neither a message nor a backtrace, and in that situation the only way to
/// locate the exact point is to have printed before reaching it. See issue #2.
fn trace(step: &str) {
    if std::env::var_os("ZIRK_TRACE").is_some() {
        eprintln!("[zirk-codegen] {step}");
    }
}

/// Looks up a target by its canonical Zirk name.
pub fn target_by_name(name: &str) -> Option<&'static ZirkTarget> {
    TARGETS.iter().find(|t| t.name == name)
}

/// Initializes the LLVM targets. Idempotent and thread-safe.
///
/// LLVM target initialization is **not** thread-safe: it registers global
/// process structures. Without this guard, two simultaneous calls corrupt that
/// state. The Rust test harness runs in parallel, so the condition is easy to
/// hit.
pub fn initialize_targets() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Target::initialize_all(&InitializationConfig::default());
    });
}

/// Triple of the host the compiler runs on.
pub fn host_triple() -> String {
    trace("querying default triple");
    TargetMachine::get_default_triple()
        .as_str()
        .to_string_lossy()
        .into_owned()
}

/// Builds the target machine for an arbitrary triple.
///
/// Fails with a diagnostic when the triple is not recognized, instead of
/// producing an invalid object.
fn target_machine(triple: &str) -> DiagnosticResult<TargetMachine> {
    trace("initializing targets");
    initialize_targets();

    trace("creating triple");
    let target_triple = TargetTriple::create(triple);

    trace("looking up target for triple");
    let target = Target::from_triple(&target_triple).map_err(|error| {
        Diagnostic::error(
            codes::UNKNOWN_TARGET,
            format!("unsupported target: `{triple}`"),
        )
        .with_cause(format!("LLVM does not recognize the triple: {error}"))
        .with_help("run `zirk build --list-targets` to see the available targets")
        .boxed()
    })?;

    // RelocMode::Default lets LLVM pick the right model for each target.
    // Forcing PIC is a Unix concept that does not apply to COFF and caused an
    // access violation when emitting for Windows.
    trace("creating target machine");
    target
        .create_target_machine(
            &target_triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| {
            Diagnostic::error(
                codes::TARGET_MACHINE_UNAVAILABLE,
                format!("could not build the target machine for `{triple}`"),
            )
            .with_cause("the linked LLVM does not include support for this architecture")
            .with_help("check that the LLVM installation includes the corresponding backend")
            .boxed()
        })
}

/// Emits an object file for the given triple.
pub fn emit_object_for_triple(
    module: &Module<'_>,
    triple: &str,
    output: &Path,
) -> DiagnosticResult<()> {
    let machine = target_machine(triple)?;

    trace("writing object file to disk");
    machine
        .write_to_file(module, FileType::Object, output)
        .map_err(|error| {
            Diagnostic::error(
                codes::EMISSION_FAILED,
                format!("could not emit the object file at `{}`", output.display()),
            )
            .with_cause(error.to_string())
            .with_help("check write permissions on the output directory")
            .boxed()
        })
}

/// Emits an object file for a target from the spec.
pub fn emit_object(
    module: &Module<'_>,
    target: &ZirkTarget,
    output: &Path,
) -> DiagnosticResult<()> {
    emit_object_for_triple(module, target.triple, output)
}

/// Emits an object file for the host.
///
/// It does not initialize the native target separately: `initialize_targets`
/// already registers every target, the host one included, and doing it through
/// two different paths reintroduces the race the guard prevents.
pub fn emit_object_for_host(module: &Module<'_>, output: &Path) -> DiagnosticResult<()> {
    emit_object_for_triple(module, &host_triple(), output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_covers_the_nine_targets_of_the_spec() {
        assert_eq!(TARGETS.len(), 9);
    }

    #[test]
    fn target_names_are_unique() {
        let mut names: Vec<_> = TARGETS.iter().map(|t| t.name).collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total);
    }

    #[test]
    fn a_target_can_be_looked_up_by_name() {
        let target = target_by_name("aarch64-macos").expect("target from the spec");
        assert_eq!(target.triple, "aarch64-apple-darwin");
        assert_eq!(target.container, Container::MachO);
    }

    #[test]
    fn an_unknown_name_does_not_resolve() {
        assert!(target_by_name("riscv64-plan9").is_none());
    }
}
