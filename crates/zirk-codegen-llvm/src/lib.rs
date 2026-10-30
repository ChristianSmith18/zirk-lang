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

pub use emit::{emit, emit_with_sources};
pub use runtime::symbols;

use inkwell::OptimizationLevel;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::values::BasicValue;
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
    /// Managed task-result boxes require a target-native GC descriptor ABI.
    pub const TASK_RESULT_ABI_UNSUPPORTED: Code = Code::new("E0103");
    /// `Thread.run` requires native managed workers and is unavailable on wasm.
    pub const MANAGED_THREADS_UNSUPPORTED: Code = Code::new("E0104");
    /// Abstract mutex verifier operations have no runtime lowering yet.
    pub const MUTEX_IR_NOT_LOWERED: Code = Code::new("E0105");
}

/// Container format of the produced object file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    MachO,
    Elf,
    Coff,
}

/// Machine-code optimization selected when LLVM emits an object.
///
/// `None` remains the compiler default and correctness oracle. Higher levels
/// are opt-in experiments until the interpreter/native parity corpus and
/// representative benchmarks pass; this enum does not change CLI defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenOptimizationLevel {
    /// Emit with LLVM's unoptimized backend.
    None,
    /// Ask LLVM for its low optimization level.
    Less,
    /// Ask LLVM for its standard optimization level.
    Default,
    /// Ask LLVM for its highest optimization level.
    Aggressive,
}

impl From<CodegenOptimizationLevel> for OptimizationLevel {
    fn from(level: CodegenOptimizationLevel) -> Self {
        match level {
            CodegenOptimizationLevel::None => Self::None,
            CodegenOptimizationLevel::Less => Self::Less,
            CodegenOptimizationLevel::Default => Self::Default,
            CodegenOptimizationLevel::Aggressive => Self::Aggressive,
        }
    }
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

/// The relocation model to emit with, which is not the same on every target.
///
/// Linux distributions link executables as PIE, and a PIE cannot hold the
/// absolute relocations `RelocMode::Default` produces on x86_64: taking the
/// address of a function — which is what building a closure does — fails at
/// link time with `R_X86_64_32 against .rodata`. It surfaced only there:
/// aarch64 has no absolute relocation of that shape to pick in the first place.
///
/// Windows keeps `Default`. PIC is a Unix concept that does not apply to COFF,
/// and forcing it caused an access violation while emitting.
fn relocation_model(triple: &str) -> RelocMode {
    if triple.contains("windows") {
        RelocMode::Default
    } else {
        RelocMode::PIC
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
fn target_machine(
    triple: &str,
    optimization: CodegenOptimizationLevel,
) -> DiagnosticResult<TargetMachine> {
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

    trace("creating target machine");
    target
        .create_target_machine(
            &target_triple,
            "generic",
            "",
            optimization.into(),
            relocation_model(triple),
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
    emit_object_for_triple_with_optimization(module, triple, output, CodegenOptimizationLevel::None)
}

/// Emits an object file at an explicit LLVM machine-code optimization level.
/// Higher levels remain experimental; callers must establish corpus parity
/// and benchmark evidence before selecting them for release builds.
pub fn emit_object_for_triple_with_optimization(
    module: &Module<'_>,
    triple: &str,
    output: &Path,
    optimization: CodegenOptimizationLevel,
) -> DiagnosticResult<()> {
    let has_managed_thread_use = module
        .get_function(crate::runtime::symbols::THREAD_RUN)
        .is_some_and(|function| function.as_global_value().get_first_use().is_some());
    if triple.starts_with("wasm") && has_managed_thread_use {
        return Err(Diagnostic::error(
            codes::MANAGED_THREADS_UNSUPPORTED,
            "managed `Thread.run` is not supported for WebAssembly targets",
        )
        .with_help("compile for a native target with managed OS-thread support")
        .boxed());
    }
    let machine = target_machine(triple, optimization)?;

    let has_task_result_descriptor = module
        .get_first_global()
        .into_iter()
        .flat_map(|first| std::iter::successors(Some(first), |global| global.get_next_global()))
        .any(|global| {
            global
                .get_name()
                .to_string_lossy()
                .starts_with("zk.task.result.descriptor.")
        });
    let pointer_bytes = machine.get_target_data().get_pointer_byte_size(None);
    if has_managed_thread_use && pointer_bytes != 8 {
        return Err(Diagnostic::error(
            codes::MANAGED_THREADS_UNSUPPORTED,
            format!(
                "managed `Thread.run` is not supported for the {pointer_bytes}-byte pointer ABI"
            ),
        )
        .with_help("use a native 64-bit target with the managed collector ABI")
        .boxed());
    }
    if has_task_result_descriptor && pointer_bytes != 8 {
        return Err(Diagnostic::error(
            codes::TASK_RESULT_ABI_UNSUPPORTED,
            format!(
                "managed task-result boxes are not supported for the {pointer_bytes}-byte pointer ABI"
            ),
        )
        .with_help("use a 64-bit target until GC descriptors and result-box headers use target-native words")
        .boxed());
    }

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

/// Compiles an IR module into an object file for the host.
///
/// This is the entry point the rest of the compiler uses: it keeps `inkwell`
/// contained in this crate, which is what the workspace requires — no other
/// crate may know LLVM.
pub fn compile_to_object(
    module: &zirk_ir::Module,
    name: &str,
    output: &Path,
) -> DiagnosticResult<()> {
    compile_to_object_with_optimization(module, name, output, CodegenOptimizationLevel::None)
}

/// Compiles an IR module into an object file for an explicit target triple.
///
/// This keeps LLVM values private to the codegen crate while allowing callers
/// to validate cross-target object emission without invoking the system linker.
pub fn compile_to_object_for_triple(
    module: &zirk_ir::Module,
    name: &str,
    triple: &str,
    output: &Path,
) -> DiagnosticResult<()> {
    if module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .any(|instruction| {
            matches!(
                instruction.kind,
                zirk_ir::InstKind::MutexAcquire { .. }
                    | zirk_ir::InstKind::MutexRelease { .. }
                    | zirk_ir::InstKind::BorrowViewStart { .. }
                    | zirk_ir::InstKind::BorrowViewEnd { .. }
            )
        })
    {
        return Err(Diagnostic::error(
            codes::MUTEX_IR_NOT_LOWERED,
            "mutex and borrowed-view IR markers are not executable yet",
        )
        .with_help("use the source mutex API after payload access, runtime, and cleanup lowering are implemented")
        .boxed());
    }
    let context = inkwell::context::Context::create();
    let llvm = emit(&context, module, name);
    emit_object_for_triple(&llvm, triple, output)
}

/// Compiles an IR module into a host object with full source-level debug data.
///
/// The source map supplies the per-file paths and line tables used for native
/// breakpoints. Ordinary builds keep using [`compile_to_object`] and do not
/// carry DWARF unless the caller explicitly requests it.
pub fn compile_to_object_with_debug_info(
    module: &zirk_ir::Module,
    name: &str,
    sources: &zirk_diagnostics::SourceMap,
    output: &Path,
) -> DiagnosticResult<()> {
    if module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .any(|instruction| {
            matches!(
                instruction.kind,
                zirk_ir::InstKind::MutexAcquire { .. }
                    | zirk_ir::InstKind::MutexRelease { .. }
                    | zirk_ir::InstKind::BorrowViewStart { .. }
                    | zirk_ir::InstKind::BorrowViewEnd { .. }
            )
        })
    {
        return Err(Diagnostic::error(
            codes::MUTEX_IR_NOT_LOWERED,
            "mutex and borrowed-view IR markers are not executable yet",
        )
        .with_help("use the source mutex API after payload access, runtime, and cleanup lowering are implemented")
        .boxed());
    }
    let context = inkwell::context::Context::create();
    let llvm = emit_with_sources(&context, module, name, sources);
    emit_object_for_host(&llvm, output)
}

/// Compiles an IR module at an explicit experimental optimization level.
pub fn compile_to_object_with_optimization(
    module: &zirk_ir::Module,
    name: &str,
    output: &Path,
    optimization: CodegenOptimizationLevel,
) -> DiagnosticResult<()> {
    if module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .any(|instruction| {
            matches!(
                instruction.kind,
                zirk_ir::InstKind::MutexAcquire { .. }
                    | zirk_ir::InstKind::MutexRelease { .. }
                    | zirk_ir::InstKind::BorrowViewStart { .. }
                    | zirk_ir::InstKind::BorrowViewEnd { .. }
            )
        })
    {
        return Err(Diagnostic::error(
            codes::MUTEX_IR_NOT_LOWERED,
            "mutex and borrowed-view IR markers are not executable yet",
        )
        .with_help("use the source mutex API after payload access, runtime, and cleanup lowering are implemented")
        .boxed());
    }
    let context = inkwell::context::Context::create();
    let llvm = emit(&context, module, name);
    emit_object_for_host_with_optimization(&llvm, output, optimization)
}

/// Emits an object file for the host.
///
/// It does not initialize the native target separately: `initialize_targets`
/// already registers every target, the host one included, and doing it through
/// two different paths reintroduces the race the guard prevents.
pub fn emit_object_for_host(module: &Module<'_>, output: &Path) -> DiagnosticResult<()> {
    emit_object_for_host_with_optimization(module, output, CodegenOptimizationLevel::None)
}

/// Emits an object for the host at an explicit optimization level.
pub fn emit_object_for_host_with_optimization(
    module: &Module<'_>,
    output: &Path,
    optimization: CodegenOptimizationLevel,
) -> DiagnosticResult<()> {
    emit_object_for_triple_with_optimization(module, &host_triple(), output, optimization)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_view_ir_cannot_be_silently_compiled_as_an_object() {
        let module = zirk_ir::Module {
            functions: vec![zirk_ir::Function {
                name: "borrow_marker".into(),
                params: vec![],
                return_type: zirk_ir::IrType::Void,
                slots: vec![],
                blocks: vec![zirk_ir::Block {
                    id: zirk_ir::BlockId(0),
                    instructions: vec![
                        zirk_ir::Instruction {
                            result: Some(zirk_ir::ValueId(1)),
                            kind: zirk_ir::InstKind::Undefined,
                            ty: zirk_ir::IrType::Mutex(0),
                            span: zirk_diagnostics::Span::new(0, 1),
                        },
                        zirk_ir::Instruction {
                            result: Some(zirk_ir::ValueId(0)),
                            kind: zirk_ir::InstKind::BorrowViewStart {
                                region: 1,
                                mutex: zirk_ir::Operand(zirk_ir::ValueId(1)),
                            },
                            ty: zirk_ir::IrType::BorrowedView {
                                region: 1,
                                mutex_type: 0,
                            },
                            span: zirk_diagnostics::Span::new(0, 1),
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: zirk_ir::BlockId(0),
                span: zirk_diagnostics::Span::new(0, 1),
                gc_roots: vec![],
            }],
            mutex_types: vec![zirk_ir::IrType::Int(zirk_ir::IntWidth::I32)],
            ..zirk_ir::Module::default()
        };
        let output =
            std::env::temp_dir().join(format!("zirk-borrow-marker-{}.o", std::process::id()));
        let error = compile_to_object(&module, "borrow_marker", &output)
            .expect_err("an abstract borrow marker has no payload access lowering");
        assert_eq!(error.code, codes::MUTEX_IR_NOT_LOWERED);
        assert!(!output.exists());
    }

    fn add_managed_thread_call(module: &Module<'_>) {
        let context = module.get_context();
        let thread_run = module
            .get_function(crate::runtime::symbols::THREAD_RUN)
            .expect("thread runtime declaration");
        let caller = module.add_function(
            "thread_run_caller",
            context.void_type().fn_type(&[], false),
            None,
        );
        let block = context.append_basic_block(caller, "entry");
        let builder = context.create_builder();
        builder.position_at_end(block);
        let ptr = context.ptr_type(Default::default());
        builder
            .build_call(
                thread_run,
                &[
                    ptr.const_null().into(),
                    ptr.const_null().into(),
                    ptr.const_null().into(),
                    ptr.const_null().into(),
                    ptr.const_null().into(),
                ],
                "thread.status",
            )
            .expect("build thread runtime call");
        builder.build_return(None).expect("return from caller");
    }

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

    #[test]
    fn task_result_boxes_fail_closed_on_32_bit_targets() {
        let context = inkwell::context::Context::create();
        let module = context.create_module("task_result_32_bit_gate");
        module.add_global(context.i8_type(), None, "zk.task.result.descriptor.0");
        let output =
            std::env::temp_dir().join(format!("zirk-task-result-32-bit-{}.o", std::process::id()));

        let error = emit_object_for_triple(&module, "i386-unknown-linux-gnu", &output)
            .expect_err("the current collector descriptor ABI is not 32-bit safe");

        assert_eq!(error.code, codes::TASK_RESULT_ABI_UNSUPPORTED);
        assert!(error.message.contains("4-byte pointer ABI"));
        assert!(!output.exists());
    }

    #[test]
    fn managed_thread_runtime_symbol_is_rejected_on_wasm_targets() {
        let context = inkwell::context::Context::create();
        let module = context.create_module("thread_run_wasm_gate");
        module.add_function(
            crate::runtime::symbols::THREAD_RUN,
            context.i32_type().fn_type(
                &[
                    context.ptr_type(Default::default()).into(),
                    context.ptr_type(Default::default()).into(),
                    context.ptr_type(Default::default()).into(),
                    context.ptr_type(Default::default()).into(),
                    context.ptr_type(Default::default()).into(),
                ],
                false,
            ),
            None,
        );
        add_managed_thread_call(&module);
        let output =
            std::env::temp_dir().join(format!("zirk-thread-run-wasm-{}.o", std::process::id()));
        let error = emit_object_for_triple(&module, "wasm32-unknown-unknown", &output)
            .expect_err("managed OS-thread execution is unsupported on wasm");
        assert_eq!(error.code, codes::MANAGED_THREADS_UNSUPPORTED);
        assert!(!output.exists());
    }

    #[test]
    fn managed_thread_runtime_symbol_is_rejected_on_i386() {
        let context = inkwell::context::Context::create();
        let module = context.create_module("thread_run_i386_gate");
        let ptr = context.ptr_type(Default::default());
        module.add_function(
            crate::runtime::symbols::THREAD_RUN,
            context.i32_type().fn_type(
                &[ptr.into(), ptr.into(), ptr.into(), ptr.into(), ptr.into()],
                false,
            ),
            None,
        );
        add_managed_thread_call(&module);
        let output =
            std::env::temp_dir().join(format!("zirk-thread-run-i386-{}.o", std::process::id()));
        let error = emit_object_for_triple(&module, "i386-unknown-linux-gnu", &output)
            .expect_err("managed collector roots are not implemented for a 32-bit pointer ABI");
        assert_eq!(error.code, codes::MANAGED_THREADS_UNSUPPORTED);
        assert!(error.message.contains("4-byte pointer ABI"));
        assert!(!output.exists());
    }

    #[test]
    fn managed_thread_callback_emits_on_supported_64_bit_targets() {
        use zirk_ir::{
            Block, BlockId, ClosureLayout, Function, Instruction, IrType, Module, ObjectLayout,
            Slot, SlotId, ValueId,
        };

        let context = inkwell::context::Context::create();
        let span = zirk_diagnostics::Span::empty(0);
        let ir = Module {
            functions: vec![Function {
                name: "thread_run_target_probe".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![
                    Slot {
                        name: "callable".into(),
                        ty: IrType::Callable(0),
                        span,
                    },
                    Slot {
                        name: "result".into(),
                        ty: IrType::String,
                        span,
                    },
                    Slot {
                        name: "exception".into(),
                        ty: IrType::Object(0),
                        span,
                    },
                ],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: zirk_ir::InstKind::Load(SlotId(0)),
                            ty: IrType::Callable(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: zirk_ir::InstKind::ThreadRun {
                                callable_slot: SlotId(0),
                                result_slot: Some(SlotId(1)),
                                exception_slot: SlotId(2),
                            },
                            ty: IrType::Int(zirk_ir::IntWidth::U32),
                            span,
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span,
                gc_roots: vec![SlotId(0), SlotId(1), SlotId(2)],
            }],
            closures: vec![ClosureLayout {
                captures: vec![IrType::String],
                params: vec![],
                returns: IrType::String,
            }],
            objects: vec![ObjectLayout {
                name: "Throwable".into(),
                fields: vec![],
                contracts: vec![],
                methods: vec![],
                ancestors: vec![0],
            }],
            ..Module::default()
        };
        zirk_ir::verify(&ir).expect("valid managed Thread.run target probe");
        let llvm = emit(&context, &ir, "thread_run_target_probe");
        llvm.verify().expect("Thread.run LLVM module verifies");

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "zirk-thread-run-target-{}-{nonce}",
            std::process::id()
        ));
        for target in TARGETS.iter().filter(|target| {
            target.triple.starts_with("x86_64-") || target.triple.starts_with("aarch64-")
        }) {
            let path = root.with_extension(format!("{}.o", target.name));
            emit_object_for_triple(&llvm, target.triple, &path)
                .unwrap_or_else(|error| panic!("{}: {}", target.name, error.render()));
            assert!(path.is_file(), "{} object was not emitted", target.name);
            std::fs::remove_file(path).expect("remove emitted target object");
        }

        for target in TARGETS.iter().filter(|target| {
            target.triple.starts_with("i386-") || target.triple.starts_with("armv7-")
        }) {
            let path = root.with_extension(format!("{}.o", target.name));
            let error = emit_object_for_triple(&llvm, target.triple, &path)
                .expect_err("32-bit managed-thread codegen must fail closed");
            assert_eq!(error.code, codes::MANAGED_THREADS_UNSUPPORTED);
            assert!(!path.exists(), "{} object must not be emitted", target.name);
        }
    }

    #[test]
    fn safepoint_poll_runtime_abi_emits_void_declaration_and_call() {
        let context = inkwell::context::Context::create();
        let span = zirk_diagnostics::Span::new(0, 1);
        let ir = zirk_ir::Module {
            functions: vec![zirk_ir::Function {
                name: "exercise_safepoint_poll".into(),
                params: vec![],
                return_type: zirk_ir::IrType::Void,
                slots: vec![],
                blocks: vec![zirk_ir::Block {
                    id: zirk_ir::BlockId(0),
                    instructions: vec![zirk_ir::Instruction {
                        result: None,
                        kind: zirk_ir::InstKind::SafepointPoll,
                        ty: zirk_ir::IrType::Void,
                        span,
                    }],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: zirk_ir::BlockId(0),
                span,
                gc_roots: vec![],
            }],
            ..zirk_ir::Module::default()
        };
        zirk_ir::verify(&ir).expect("valid safepoint poll IR");
        let module = emit(&context, &ir, "safepoint_poll_abi");
        module.verify().expect("valid safepoint poll LLVM module");
        let text = module.to_string();
        assert!(
            text.contains("declare void @zirk_rt_safepoint_poll()"),
            "missing void runtime declaration:\n{text}"
        );
        assert!(
            text.contains("call void @zirk_rt_safepoint_poll()"),
            "missing runtime call:\n{text}"
        );
    }

    #[test]
    fn atomic_cell_abi_emits_exact_external_symbols_and_opaque_pointer_signatures() {
        let context = inkwell::context::Context::create();
        let mut ir = zirk_ir::Module::default();
        zirk_ir::declare_atomic_cell_abi(&mut ir).expect("declare scalar AtomicCell ABI");
        let handle = ir.pointer_types[1];
        let handle_out = ir.externs[0].params[2];
        let span = zirk_diagnostics::Span::new(0, 1);
        ir.functions.push(zirk_ir::Function {
            name: "exercise_atomic_abi".into(),
            params: vec![],
            return_type: zirk_ir::IrType::Void,
            slots: vec![zirk_ir::Slot {
                name: "cell".into(),
                ty: handle,
                span,
            }],
            blocks: vec![zirk_ir::Block {
                id: zirk_ir::BlockId(0),
                instructions: vec![
                    zirk_ir::Instruction {
                        result: Some(zirk_ir::ValueId(0)),
                        kind: zirk_ir::InstKind::ConstInt(1),
                        ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U32),
                        span,
                    },
                    zirk_ir::Instruction {
                        result: Some(zirk_ir::ValueId(1)),
                        kind: zirk_ir::InstKind::ConstInt(7),
                        ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U64),
                        span,
                    },
                    zirk_ir::Instruction {
                        result: Some(zirk_ir::ValueId(2)),
                        kind: zirk_ir::InstKind::PointerFromSlot(zirk_ir::SlotId(0)),
                        ty: handle_out,
                        span,
                    },
                    zirk_ir::Instruction {
                        result: Some(zirk_ir::ValueId(3)),
                        kind: zirk_ir::InstKind::Call {
                            callee: symbols::ATOMIC_NEW.into(),
                            args: vec![
                                zirk_ir::Operand(zirk_ir::ValueId(0)),
                                zirk_ir::Operand(zirk_ir::ValueId(1)),
                                zirk_ir::Operand(zirk_ir::ValueId(2)),
                            ],
                        },
                        ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::I32),
                        span,
                    },
                ],
                terminator: Some(zirk_ir::Terminator::Return(None)),
            }],
            entry: zirk_ir::BlockId(0),
            span,
            gc_roots: vec![],
        });
        zirk_ir::verify(&ir).expect("well-typed AtomicCell call in IR");
        let llvm = emit(&context, &ir, "atomic_cell_abi");
        llvm.verify().expect("valid LLVM AtomicCell declarations");
        let text = llvm.to_string();
        for symbol in [
            symbols::ATOMIC_NEW,
            symbols::ATOMIC_FREE,
            symbols::ATOMIC_LOAD,
            symbols::ATOMIC_STORE,
            symbols::ATOMIC_EXCHANGE,
            symbols::ATOMIC_COMPARE_EXCHANGE,
            symbols::ATOMIC_FETCH_ADD,
            symbols::ATOMIC_FETCH_SUB,
        ] {
            assert!(
                text.contains(&format!("@{symbol}")),
                "missing {symbol}:\n{text}"
            );
        }
        assert!(
            text.contains("declare i32 @zirk_rt_atomic_new(i32, i64, ptr)"),
            "{text}"
        );
        assert!(
            text.contains("call i32 @zirk_rt_atomic_new(i32 1, i64 7, ptr"),
            "{text}"
        );
        assert!(
            text.contains("alloca ptr"),
            "aligned output slot missing:\n{text}"
        );
        assert!(
            text.contains("declare void @zirk_rt_atomic_free(ptr)"),
            "{text}"
        );
        assert!(
            text.contains(
                "declare i32 @zirk_rt_atomic_compare_exchange(ptr, i64, i64, i32, i32, ptr, ptr)"
            ),
            "{text}"
        );
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("zirk-atomic-abi-{}-{nonce}", std::process::id()));
        for target in TARGETS {
            let path = root.with_extension(format!("{}.o", target.name));
            emit_object_for_triple(&llvm, target.triple, &path)
                .unwrap_or_else(|error| panic!("{}: {}", target.name, error.render()));
            assert!(path.is_file(), "{} object was not emitted", target.name);
            std::fs::remove_file(path).expect("remove emitted target object");
        }
    }

    #[test]
    fn gc_mutex_runtime_declarations_match_the_native_abi_shapes() {
        let context = inkwell::context::Context::create();
        let llvm = emit(&context, &zirk_ir::Module::default(), "gc_mutex_abi");
        llvm.verify().expect("valid GC mutex declarations");
        let text = llvm.to_string();
        assert!(
            text.contains("declare ptr @zirk_rt_gc_mutex_alloc(ptr, i64, i64, ptr, i64, ptr)"),
            "{text}"
        );
        assert!(
            text.contains("declare i32 @zirk_rt_gc_mutex_acquire(ptr, ptr, ptr)"),
            "{text}"
        );
        assert!(
            text.contains("declare i32 @zirk_rt_gc_mutex_release(ptr, ptr)"),
            "{text}"
        );
        assert!(
            text.contains("declare i32 @zirk_rt_gc_mutex_with(ptr, ptr, ptr)"),
            "{text}"
        );
    }

    #[test]
    fn each_explicit_optimization_level_emits_a_host_object() {
        let context = inkwell::context::Context::create();
        let module = context.create_module("optimization_policy");
        let function = module.add_function("main", context.i32_type().fn_type(&[], false), None);
        let block = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(block);
        builder
            .build_return(Some(&context.i32_type().const_zero()))
            .expect("return zero");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("zirk-opt-{}-{nonce}", std::process::id()));
        for level in [
            CodegenOptimizationLevel::None,
            CodegenOptimizationLevel::Less,
            CodegenOptimizationLevel::Default,
            CodegenOptimizationLevel::Aggressive,
        ] {
            let path = root.with_extension(format!("{level:?}.o"));
            emit_object_for_host_with_optimization(&module, &path, level)
                .unwrap_or_else(|error| panic!("{level:?}: {}", error.render()));
            assert!(path.is_file(), "{level:?} did not emit an object");
            std::fs::remove_file(path).expect("remove emitted object");
        }
    }
}
