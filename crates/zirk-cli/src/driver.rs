//! The compilation driver: coordinates the pipeline stages.
//!
//! The CLI implements no compilation logic of its own. Each stage lives in its
//! crate; here they are coordinated, the linker is invoked and the result is
//! turned into terminal output and exit codes.

use std::path::{Path, PathBuf};
use std::process::Command;
use zirk_codegen_llvm::compile_to_object;
use zirk_diagnostics::{Diagnostic, DiagnosticSink, RenderStyle, SourceFile};

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

    let Ok(text) = std::fs::read_to_string(path) else {
        sink.emit(
            Diagnostic::error(
                codes::UNREADABLE_FILE,
                format!("could not read `{}`", path.display()),
            )
            .with_cause("the file does not exist or is not readable")
            .with_help("check the path and its permissions"),
        );
        return Compilation::failed(sink);
    };

    let source = SourceFile::new(path.display().to_string(), text);

    // The frontend accumulates every diagnostic it finds; the pipeline only
    // stops between stages, so one run reports as much as it can.
    let tokens = zirk_lexer::tokenize(&source, &mut sink);
    let program = zirk_parser::parse(&source, &tokens, &mut sink);
    if sink.has_errors() {
        return Compilation::failed(sink);
    }

    let checked = zirk_sema::check(&source, &program, &mut sink);
    if sink.has_errors() {
        return Compilation::failed(sink);
    }

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
///
/// The link driver is the `clang` of the pinned LLVM installation rather than
/// whatever C compiler the host has (design D7): the project already requires
/// that installation, and relying on the host compiler reintroduces the
/// variability ADR-004 set out to remove.
fn link(object: &Path, executable: &Path) -> Result<(), Box<Diagnostic>> {
    let runtime = locate_runtime()?;
    let driver = link_driver();

    let output = Command::new(&driver)
        .arg(object)
        .arg(&runtime)
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
        return Err(Diagnostic::error(codes::LINK_FAILED, "linking failed")
            .with_cause(String::from_utf8_lossy(&output.stderr).trim().to_string())
            .with_help("check the LLVM installation; see docs/TOOLCHAIN.md")
            .boxed());
    }

    Ok(())
}

/// Finds the runtime static library.
///
/// It is looked up next to the compiler executable, which is where Cargo leaves
/// it. This is Phase 1 pragmatism: proper toolchain distribution is Phase 8,
/// and inventing a layout now would be a decision with no information behind it.
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

/// Renders the accumulated diagnostics.
pub fn render(sink: &DiagnosticSink, json: bool) -> String {
    sink.render(if json {
        RenderStyle::Json
    } else {
        RenderStyle::Human
    })
}
