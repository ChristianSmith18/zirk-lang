//! The compilation driver: coordinates the pipeline stages.
//!
//! The CLI implements no compilation logic of its own. Each stage lives in its
//! crate; here they are coordinated, the linker is invoked and the result is
//! turned into terminal output and exit codes.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use zirk_codegen_llvm::compile_to_object;
use zirk_diagnostics::{Color, Diagnostic, DiagnosticSink, RenderStyle};

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

    // The crate is the entry file plus everything it imports, walked from the
    // entry rather than from a directory listing: a `.zrk` nobody imports is
    // not part of the program.
    let Some(loaded) = crate::modules::load(path, &mut sink) else {
        return Compilation::failed(sink);
    };
    if sink.has_errors() {
        return Compilation::failed(sink);
    }

    let program = merge(&loaded);
    let checked = zirk_sema::check(&loaded.sources, &program, &mut sink);
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

/// Merges the files of a crate into the program the checker sees.
///
/// There is one namespace per crate in this phase: which file a declaration
/// came from still matters for visibility, and that travels in its spans, but
/// two declarations cannot share a name even in different files. Proper
/// per-module namespacing belongs with the project system of Phase 6.
fn merge(loaded: &crate::modules::Crate) -> zirk_ast::Program {
    let mut program = zirk_ast::Program {
        imports: Vec::new(),
        uses: Vec::new(),
        enums: Vec::new(),
        classes: Vec::new(),
        contracts: Vec::new(),
        functions: Vec::new(),
        span: loaded.sources.entry().span(0, 0),
    };

    for unit in &loaded.units {
        program.imports.extend(unit.program.imports.iter().cloned());
        program.uses.extend(unit.program.uses.iter().cloned());
        program.enums.extend(unit.program.enums.iter().cloned());
        program.classes.extend(unit.program.classes.iter().cloned());
        program
            .contracts
            .extend(unit.program.contracts.iter().cloned());
        program
            .functions
            .extend(unit.program.functions.iter().cloned());
    }

    program
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
///
/// `zirk-runtime` exposes nine `extern "C"` symbols (`zirk_rt_init`,
/// `zirk_str_from_i32`, `zirk_str_eq`, ...). A static archive is linked at
/// whole-object-file granularity: if any symbol in an object file is
/// referenced, the linker keeps the entire file. With nine separate entry
/// points, that retains far more of Rust's `std` than any single Zirk program
/// actually calls.
///
/// Without this flag, `fn main(): Void { stdout.println("..."); }` links to
/// roughly 1.4 MB; with it, to roughly 400 KB — in line with a plain Rust
/// `println!("...")` binary. Measured locally on `aarch64-macos`; Linux and
/// Windows are confirmed in CI.
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
///
/// The Zirk runtime uses Rust's `std`, which on Windows depends on system
/// libraries the linker does not add on its own — on Unix, libc arrives by
/// default. The list is the one `rustc --print native-static-libs` reports for
/// the `windows-msvc` target.
///
/// Hardcoding it is Phase 1 pragmatism, the same as locating the runtime next
/// to the executable. Deriving it from the toolchain belongs with proper
/// distribution in Phase 8.
fn system_libraries() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }

    [
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
    .collect()
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
pub fn render(sink: &DiagnosticSink, json: bool, color: Color) -> String {
    let style = if json {
        RenderStyle::Json
    } else {
        RenderStyle::Human
    };
    sink.render_colored(style, color)
}

/// Decides whether the diagnostics carry colour.
///
/// `ZIRK_COMPILER_SPEC.md` section 9 requires deterministic output, so colour
/// only appears when the destination is a terminal a person is reading. Piping
/// the output must yield exactly the same bytes as an uncoloured run.
///
/// The precedence is the usual one in the ecosystem, from strongest to weakest:
///
/// 1. an explicit `--color` on the command line;
/// 2. `NO_COLOR`, which by convention disables colour whatever its value
///    (<https://no-color.org>);
/// 3. whether standard error is a terminal.
pub fn resolve_color(requested: Option<&str>) -> Color {
    match requested {
        Some("always") => return Color::Ansi,
        Some("never") => return Color::Never,
        _ => {}
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return Color::Never;
    }

    // Standard error and not output: that is where diagnostics go.
    if std::io::stderr().is_terminal() {
        Color::Ansi
    } else {
        Color::Never
    }
}
