//! The compilation driver: coordinates the backend pipeline stages.
//!
//! The CLI implements no compilation logic of its own. Each stage lives in its
//! crate; here they are coordinated, the linker is invoked and the result is
//! turned into terminal output and exit codes.
//!
//! This module is only compiled when the `backend` feature is enabled.

use std::path::{Path, PathBuf};
use std::process::Command;
use zirk_codegen_llvm::compile_to_object;
use zirk_diagnostics::{Diagnostic, DiagnosticSink};

use crate::codes;

/// What to do with a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Produce an executable.
    Build,
    /// Produce an executable and run it.
    Run,
}

/// Result of a compilation.
pub struct Compilation {
    pub executable: Option<PathBuf>,
    pub sink: DiagnosticSink,
}

impl Compilation {
    fn failed(sink: DiagnosticSink) -> Self {
        Self {
            executable: None,
            sink,
        }
    }
}

/// Compiles a source file into a native executable.
pub fn compile(path: &Path, output_dir: &Path) -> Compilation {
    let mut sink = DiagnosticSink::new();

    let Some(crate::frontend::FrontendResult {
        program, checked, ..
    }) = crate::frontend::run_frontend(path, &mut sink)
    else {
        return Compilation::failed(sink);
    };

    let ir = zirk_ir::lower(&program, &checked);

    // A malformed IR is a compiler bug, not a user error. It is reported as
    // such so nobody wastes time looking for the mistake in their own code.
    if let Err(errors) = zirk_ir::verify(&ir) {
        let detail = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        sink.emit(
            Diagnostic::error(codes::INTERNAL_ERROR, "the compiler produced invalid IR")
                .with_cause(detail)
                .with_help(
                    "this is a compiler bug: please report it with the source that triggered it",
                ),
        );
        return Compilation::failed(sink);
    }

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".to_string());

    if let Err(error) = std::fs::create_dir_all(output_dir) {
        sink.emit(
            Diagnostic::error(
                codes::OUTPUT_UNAVAILABLE,
                format!("could not create `{}`", output_dir.display()),
            )
            .with_cause(error.to_string()),
        );
        return Compilation::failed(sink);
    }

    let object = output_dir.join(format!("{stem}{}", object_extension()));
    let executable = output_dir.join(exe_name(&stem));

    if let Err(diagnostic) = compile_to_object(&ir, &stem, &object) {
        sink.emit(*diagnostic);
        return Compilation::failed(sink);
    }

    if let Err(diagnostic) = link(&object, &executable) {
        sink.emit(*diagnostic);
        return Compilation::failed(sink);
    }

    Compilation {
        executable: Some(executable),
        sink,
    }
}

/// Links the object with the Zirk runtime.
fn link(object: &Path, executable: &Path) -> Result<(), Box<Diagnostic>> {
    let runtime = locate_runtime()?;
    let driver = link_driver();

    let output = Command::new(&driver)
        .arg(object)
        .arg(&runtime)
        .args(system_libraries())
        .arg(dead_strip_flag())
        .arg("-o")
        .arg(executable)
        .output()
        .map_err(|error| {
            Diagnostic::error(codes::LINK_FAILED, "could not invoke the linker")
                .with_cause(format!("`{}`: {error}", driver.display()))
                .with_help("check the LLVM installation; see docs/TOOLCHAIN.md")
                .boxed()
        })?;

    if !output.status.success() {
        // Both streams are included: on Windows the linker writes the
        // unresolved symbols to stdout and only the summary to stderr, and
        // without them the diagnostic says nothing actionable.
        let mut detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&output.stdout);
        if !out.trim().is_empty() {
            detail.push('\n');
            detail.push_str(out.trim());
        }

        return Err(Diagnostic::error(codes::LINK_FAILED, "linking failed")
            .with_cause(detail)
            .with_help("check the LLVM installation; see docs/TOOLCHAIN.md")
            .boxed());
    }

    Ok(())
}

/// Requests dead-code elimination at link time, per container format.
fn dead_strip_flag() -> &'static str {
    if cfg!(target_os = "macos") {
        "-Wl,-dead_strip"
    } else if cfg!(windows) {
        // COFF's `lld-link` has no `--gc-sections`; the equivalent is `/OPT:REF`.
        "-Wl,/OPT:REF"
    } else {
        "-Wl,--gc-sections"
    }
}

/// System libraries the runtime needs, per platform.
fn system_libraries() -> Vec<String> {
    if cfg!(windows) {
        return [
            "advapi32",
            "bcrypt",
            "dbghelp",
            "kernel32",
            "ntdll",
            "ole32",
            "oleaut32",
            "shell32",
            "synchronization",
            "user32",
            "userenv",
            "uuid",
            "ws2_32",
        ]
        .iter()
        .map(|lib| format!("-l{lib}"))
        .collect();
    }

    if cfg!(target_os = "linux") {
        // Rust's own `std` calls into libm (`f64::log10` and friends,
        // `zirk-runtime`'s temporal/decimal formatting reaches these) and
        // libpthread (`crates/zirk-runtime/src/pool.rs`'s worker threads,
        // `Mutex`/`Condvar`). `cargo build`'s own linker invocation gets
        // these transitively from `libstd.so`'s own recorded dependencies;
        // linking `libzirk_runtime.a` directly through `clang` here does
        // not, so they must be named explicitly. macOS/Windows resolve both
        // through their own C runtime by default and need neither.
        return vec!["-lm".to_string(), "-lpthread".to_string()];
    }

    Vec::new()
}

/// Finds the runtime static library.
fn locate_runtime() -> Result<PathBuf, Box<Diagnostic>> {
    let name = if cfg!(windows) {
        "zirk_runtime.lib"
    } else {
        "libzirk_runtime.a"
    };

    let exe = std::env::current_exe().ok();
    let candidates = exe
        .as_ref()
        .and_then(|e| e.parent())
        .map(|dir| vec![dir.join(name), dir.join("deps").join(name)])
        .unwrap_or_default();

    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .ok_or_else(|| {
            Diagnostic::error(codes::RUNTIME_NOT_FOUND, "the Zirk runtime was not found")
                .with_cause(format!("`{name}` is not next to the compiler executable"))
                .with_help("build the workspace with `cargo build` so the runtime is produced")
                .boxed()
        })
}

fn link_driver() -> PathBuf {
    // The same criterion as the sanity check: prefer the pinned installation.
    if let Ok(prefix) = std::env::var("LLVM_SYS_201_PREFIX") {
        let candidate = Path::new(&prefix).join("bin").join(exe_name("clang"));
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(exe_name("clang"))
}

fn object_extension() -> &'static str {
    if cfg!(windows) { ".obj" } else { ".o" }
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

/// Runs an already compiled executable and returns its exit code.
pub fn run(executable: &Path) -> Result<i32, Box<Diagnostic>> {
    let status = Command::new(executable).status().map_err(|error| {
        Diagnostic::error(
            codes::EXECUTION_FAILED,
            format!("could not run `{}`", executable.display()),
        )
        .with_cause(error.to_string())
        .boxed()
    })?;

    // A process killed by a signal has no code. 130 is the convention for
    // SIGINT and keeps the caller from reading it as success.
    Ok(status.code().unwrap_or(130))
}
