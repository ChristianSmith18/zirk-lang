//! The compilation driver: coordinates the backend pipeline stages.
//!
//! The CLI implements no compilation logic of its own. Each stage lives in its
//! crate; here they are coordinated, the linker is invoked and the result is
//! turned into terminal output and exit codes.
//!
//! This module is only compiled when the `backend` feature is enabled.

use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::process::ExitStatus;
pub use zirk_codegen_llvm::CodegenOptimizationLevel;
use zirk_codegen_llvm::{
    compile_to_object, compile_to_object_with_debug_info, compile_to_object_with_optimization,
};
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
    compile_with_optimization(path, output_dir, CodegenOptimizationLevel::None)
}

/// Compiles a source file at an explicit backend optimization level.
/// Higher levels are for parity/benchmark experiments; the CLI continues to
/// call [`compile`] and therefore always selects the unoptimized oracle.
pub fn compile_with_optimization(
    path: &Path,
    output_dir: &Path,
    optimization: CodegenOptimizationLevel,
) -> Compilation {
    compile_internal(path, output_dir, optimization, false)
}

/// Compiles a native executable with source-level DWARF for native debugging.
pub fn compile_with_debug_info(path: &Path, output_dir: &Path) -> Compilation {
    compile_internal(path, output_dir, CodegenOptimizationLevel::None, true)
}

fn compile_internal(
    path: &Path,
    output_dir: &Path,
    optimization: CodegenOptimizationLevel,
    debug_info: bool,
) -> Compilation {
    let mut sink = DiagnosticSink::new();

    let Some(crate::frontend::FrontendResult {
        program,
        checked,
        sources,
        ..
    }) = crate::frontend::run_frontend(path, &mut sink)
    else {
        return Compilation::failed(sink);
    };

    let ir = zirk_ir::lower(&program, &checked);

    if let Some((_, size_bytes, span)) = ir.oversized_union_payloads().into_iter().next() {
        sink.emit(
            Diagnostic::error(
                codes::UNION_PAYLOAD_TOO_LARGE,
                format!(
                    "union payload is {size_bytes} bytes, exceeding the {}-byte inline limit",
                    zirk_ir::MAX_INLINE_UNION_PAYLOAD_BYTES
                ),
            )
            .at(sources.location(span))
            .with_cause("large union payloads do not use an implicit boxed fallback")
            .with_help(
                "split the payload into smaller alternatives or pass it by managed reference",
            ),
        );
        return Compilation::failed(sink);
    }

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

    let emission = if debug_info {
        compile_to_object_with_debug_info(&ir, &stem, &sources, &object)
    } else if optimization == CodegenOptimizationLevel::None {
        compile_to_object(&ir, &stem, &object)
    } else {
        compile_to_object_with_optimization(&ir, &stem, &object, optimization)
    };
    if let Err(diagnostic) = emission {
        sink.emit(*diagnostic);
        return Compilation::failed(sink);
    }

    if let Err(diagnostic) = link(&object, &executable, debug_info) {
        sink.emit(*diagnostic);
        return Compilation::failed(sink);
    }
    if debug_info {
        if let Err(diagnostic) = write_native_debug_symbols(&executable) {
            sink.emit(*diagnostic);
            return Compilation::failed(sink);
        }
    }

    Compilation {
        executable: Some(executable),
        sink,
    }
}

/// Links the object with the Zirk runtime.
fn link(
    object: &Path,
    executable: &Path,
    preserve_debug_info: bool,
) -> Result<(), Box<Diagnostic>> {
    let runtime = locate_runtime()?;
    let driver = link_driver();

    let output = Command::new(&driver)
        .arg(object)
        .arg(&runtime)
        .args(system_libraries())
        .arg(dead_strip_flag())
        .args(if preserve_debug_info {
            &[][..]
        } else {
            strip_debug_info_args()
        })
        .arg("-o")
        .arg(executable)
        .output()
        .map_err(|error| {
            Diagnostic::error(codes::LINK_FAILED, "could not invoke the linker")
                .with_cause(format!("`{}`: {error}", driver.display()))
                .with_help(linker_help())
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
        if detail.is_empty() {
            detail = format!("clang exited with {} and produced no output", output.status);
        }

        return Err(Diagnostic::error(codes::LINK_FAILED, "linking failed")
            .with_cause(detail)
            .with_help(linker_help())
            .boxed());
    }

    Ok(())
}

fn linker_help() -> &'static str {
    if cfg!(target_os = "macos") {
        "install Xcode or the Command Line Tools to provide Apple clang and the macOS SDK; see docs/TOOLCHAIN.md"
    } else {
        "check the native linker toolchain; see docs/TOOLCHAIN.md"
    }
}

/// Materializes Mach-O DWARF into the standard companion bundle consumed by
/// LLDB. Apple's linker drops object DWARF from the executable itself, so a
/// debug build must run `dsymutil` after linking.
fn write_native_debug_symbols(executable: &Path) -> Result<(), Box<Diagnostic>> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    let symbols = executable.with_extension("dSYM");
    let output = Command::new("xcrun")
        .arg("dsymutil")
        .arg(executable)
        .arg("-o")
        .arg(&symbols)
        .output()
        .map_err(|error| {
            Diagnostic::error(
                codes::LINK_FAILED,
                "could not generate native debug symbols",
            )
            .with_cause(format!("could not invoke `xcrun dsymutil`: {error}"))
            .with_help("install Xcode or the Command Line Tools to provide `dsymutil`")
            .boxed()
        })?;
    if !output.status.success() {
        let mut detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if detail.is_empty() {
            detail = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        }
        if detail.is_empty() {
            detail = format!(
                "dsymutil exited with {} and produced no output",
                output.status
            );
        }
        return Err(Diagnostic::error(
            codes::LINK_FAILED,
            "could not generate native debug symbols",
        )
        .with_cause(detail)
        .with_help("check that the Xcode or Command Line Tools installation is complete")
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

/// Rust's Linux static runtime archive carries DWARF sections in development
/// builds. GNU ld's `--gc-sections` does not discard those non-allocatable
/// sections, so a tiny Zirk executable can otherwise inherit tens of MiB of
/// runtime debug information. Ordinary Zirk builds do not yet emit source-level
/// DWARF; keep the ordinary symbol table and unwind metadata while excluding
/// only debug sections from the linked application. `--debug-info` bypasses
/// this flag and creates a native debug-symbol bundle.
fn strip_debug_info_args() -> &'static [&'static str] {
    if cfg!(target_os = "linux") {
        &["-Wl,--strip-debug"]
    } else {
        &[]
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

    if cfg!(target_os = "macos") {
        // HTTPS certificate loading uses rustls-native-certs, whose Security
        // and CoreFoundation references are not carried transitively when
        // Zirk links its runtime archive directly through clang.
        return [
            "-framework",
            "Security",
            "-framework",
            "CoreFoundation",
            "-framework",
            "CoreServices",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
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

    // `current_exe` can preserve the invoked symlink path. Resolve it before
    // searching beside the compiler so version-manager style launchers (for
    // example ~/.local/bin/zirk -> ~/.local/share/zirk/current/bin/zirk)
    // keep the compiler paired with its own runtime archive.
    let exe = std::env::current_exe()
        .ok()
        .map(|path| path.canonicalize().unwrap_or(path));
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
///
/// On Unix, SIGINT received by the CLI is forwarded to the direct child and
/// the child's resulting status is preserved. Other platforms retain the
/// standard-library wait behavior; signal forwarding is not implemented
/// there yet.
pub fn run(executable: &Path) -> Result<i32, Box<Diagnostic>> {
    let mut command = Command::new(executable);
    command.env_remove("ZIRK_RUNTIME_PERMISSION_GRANT");
    run_command(&mut command, executable)
}

/// Runs a generated program with an authenticated approval token. Removing
/// any inherited token first prevents the invoking shell from accidentally
/// lending another project's grant to this executable.
pub fn run_with_runtime_grant(
    executable: &Path,
    token: Option<&str>,
    json_diagnostics: bool,
) -> Result<i32, Box<Diagnostic>> {
    run_with_runtime_permissions(executable, token, json_diagnostics, None)
}

/// Runs with the authenticated grant and project identity issued by the
/// launcher. All inherited Zirk runtime metadata is removed before the
/// trusted values are installed.
pub fn run_with_runtime_permissions(
    executable: &Path,
    token: Option<&str>,
    json_diagnostics: bool,
    runtime_environment: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<i32, Box<Diagnostic>> {
    let mut command = Command::new(executable);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ZIRK_RUNTIME_") {
            command.env_remove(key);
        }
    }
    if let Some(token) = token {
        command.env("ZIRK_RUNTIME_PERMISSION_GRANT", token);
    }
    if let Some(runtime_environment) = runtime_environment {
        command.envs(runtime_environment);
    }
    if json_diagnostics {
        command.env("ZIRK_RUNTIME_DIAGNOSTIC_JSON", "1");
    }
    run_command(&mut command, executable)
}

fn run_command(command: &mut Command, executable: &Path) -> Result<i32, Box<Diagnostic>> {
    #[cfg(unix)]
    let status = run_command_with_sigint_forwarding(command, executable)?;

    // No portable, safe standard-library API currently forwards console
    // signals to one child on non-Unix targets. Keep the existing wait
    // behavior there; in particular, this does not promise Windows handling.
    #[cfg(not(unix))]
    let status = command
        .status()
        .map_err(|error| execution_error(executable, error))?;

    if let Some(code) = status.code() {
        return Ok(code);
    }

    // Preserve the conventional 128 + signal status and make the cause
    // visible to callers that capture stderr (for example, end-to-end tests).
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            eprintln!("zirk: child process terminated by signal {signal}");
            return Ok(128 + signal);
        }
    }
    Ok(130)
}

fn execution_error(executable: &Path, error: std::io::Error) -> Box<Diagnostic> {
    Diagnostic::error(
        codes::EXECUTION_FAILED,
        format!("could not run `{}`", executable.display()),
    )
    .with_cause(error.to_string())
    .boxed()
}

#[cfg(unix)]
fn run_command_with_sigint_forwarding(
    command: &mut Command,
    executable: &Path,
) -> Result<ExitStatus, Box<Diagnostic>> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    use signal_hook::consts::SIGINT;
    use signal_hook::low_level::unregister;

    // Respect an inherited ignore disposition. Unix preserves SIG_IGN across
    // exec; replacing it in the CLI would unexpectedly make both the parent
    // and generated child respond to Ctrl-C.
    let mut inherited: libc::sigaction = unsafe { std::mem::zeroed() };
    // SAFETY: query the disposition for SIGINT without changing it.
    if unsafe { libc::sigaction(libc::SIGINT, std::ptr::null(), &mut inherited) } == 0
        && inherited.sa_sigaction == libc::SIG_IGN
    {
        return command
            .status()
            .map_err(|error| execution_error(executable, error));
    }

    // signal-hook confines the OS signal handler to an atomic flag. Process
    // signaling and child waiting happen on this ordinary thread.
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal_id = signal_hook::flag::register(SIGINT, Arc::clone(&interrupted))
        .map_err(|error| execution_error(executable, error))?;
    struct SignalRegistration(signal_hook::SigId);
    impl Drop for SignalRegistration {
        fn drop(&mut self) {
            unregister(self.0);
        }
    }
    let _signal_registration = SignalRegistration(signal_id);

    let mut child = command
        .spawn()
        .map_err(|error| execution_error(executable, error))?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                // Keep ownership of the child until it is reaped, including
                // when polling fails, so an I/O error cannot orphan it.
                let _ = child.kill();
                let _ = child.wait();
                return Err(execution_error(executable, error));
            }
        }

        if interrupted.swap(false, Ordering::Relaxed) {
            // Forward only to this direct child. In particular, do not signal
            // its process group or descendants.
            let Ok(pid) = i32::try_from(child.id()) else {
                let _ = child.kill();
                let _ = child.wait();
                return Err(execution_error(
                    executable,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "child process ID is outside the Unix pid_t range",
                    ),
                ));
            };
            if let Err(error) = kill(Pid::from_raw(pid), Signal::SIGINT) {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        return child
                            .wait()
                            .map_err(|wait_error| execution_error(executable, wait_error));
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(execution_error(executable, error.into()));
                    }
                    Err(wait_error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(execution_error(executable, wait_error));
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(test, unix))]
mod signal_forwarding_tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    static NEXT_TEST: AtomicUsize = AtomicUsize::new(0);

    fn test_path(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "zirk-cli-signal-{}-{}-{suffix}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ))
    }

    // This test is re-entered as a helper process by the parent-only and
    // direct-child cases below. It verifies the actual `run_command` wait and
    // exit-code handling without requiring a compiled Zirk program.
    #[test]
    fn signal_helper_process() {
        if std::env::var_os("ZIRK_CLI_SIGNAL_HELPER_IGNORE").is_some() {
            // SAFETY: this subprocess intentionally inherits SIG_IGN into the
            // child program so the CLI's forwarding layer can preserve it.
            assert_ne!(
                unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) },
                libc::SIG_ERR
            );
            let executable = Path::new("/bin/sh");
            let mut command = Command::new(executable);
            command
                .arg("-c")
                .arg("echo READY; sleep 0.4; echo FINISHED");
            assert_eq!(run_command(&mut command, executable).unwrap(), 0);
            return;
        }
        let Ok(command_line) = std::env::var("ZIRK_CLI_SIGNAL_HELPER_COMMAND") else {
            return;
        };
        let executable = Path::new("/bin/sh");
        let mut command = Command::new(executable);
        command.arg("-c").arg(command_line);
        let actual = run_command(&mut command, executable).expect("helper child runs");
        let expected: i32 = std::env::var("ZIRK_CLI_SIGNAL_HELPER_EXPECT")
            .expect("expected child exit status")
            .parse()
            .expect("valid exit status");
        assert_eq!(actual, expected);
    }

    fn exercise_signal(target: SignalTarget, expected: i32) {
        let ready = test_path("ready");
        let cleaned = test_path("cleaned");
        let shell = format!(
            "trap \"echo cleaned > '{}'; exit {}\" INT; echo $$ > '{}'; while :; do sleep 0.02; done",
            cleaned.display(),
            expected,
            ready.display()
        );
        let mut helper = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "driver::signal_forwarding_tests::signal_helper_process",
                "--nocapture",
            ])
            .env("ZIRK_CLI_SIGNAL_HELPER_COMMAND", shell)
            .env("ZIRK_CLI_SIGNAL_HELPER_EXPECT", expected.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn helper process");

        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "child did not report readiness");

        let pid = match target {
            SignalTarget::ParentOnly => helper.id() as i32,
            SignalTarget::DirectChild => fs::read_to_string(&ready)
                .expect("read child pid")
                .trim()
                .parse()
                .expect("valid child pid"),
        };
        kill(Pid::from_raw(pid), Signal::SIGINT).expect("send SIGINT");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = helper.try_wait().expect("wait for helper") {
                assert!(
                    status.success(),
                    "helper failed: {}",
                    String::from_utf8_lossy(
                        &helper
                            .wait_with_output()
                            .expect("collect helper output")
                            .stderr
                    )
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "helper did not exit after SIGINT"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(cleaned.exists(), "child cleanup handler did not run");
        let _ = fs::remove_file(ready);
        let _ = fs::remove_file(cleaned);
    }

    #[derive(Clone, Copy)]
    enum SignalTarget {
        ParentOnly,
        DirectChild,
    }

    #[test]
    fn parent_only_sigint_is_forwarded_and_child_exit_124_is_preserved() {
        exercise_signal(SignalTarget::ParentOnly, 124);
    }

    #[test]
    fn direct_child_sigint_keeps_its_exit_status() {
        exercise_signal(SignalTarget::DirectChild, 37);
    }

    #[test]
    fn inherited_ignored_sigint_is_not_forwarded_or_overridden() {
        let mut helper = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "driver::signal_forwarding_tests::signal_helper_process",
                "--nocapture",
            ])
            .env("ZIRK_CLI_SIGNAL_HELPER_IGNORE", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn ignored-signal helper");
        let mut stdout = std::io::BufReader::new(helper.stdout.take().expect("helper stdout"));
        let mut ready = String::new();
        while !ready.contains("READY") {
            let mut line = String::new();
            if std::io::BufRead::read_line(&mut stdout, &mut line).expect("read helper output") == 0
            {
                break;
            }
            ready.push_str(&line);
        }
        if !ready.contains("READY") {
            let output = helper.wait_with_output().expect("collect helper failure");
            panic!(
                "helper did not report readiness (output={ready:?}, status={}, stderr={:?})",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        kill(Pid::from_raw(helper.id() as i32), Signal::SIGINT)
            .expect("send SIGINT to ignored helper");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if helper.try_wait().expect("poll helper").is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "helper did not finish normally");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut remaining = String::new();
        std::io::Read::read_to_string(&mut stdout, &mut remaining)
            .expect("read remaining child output");
        let output = helper.wait_with_output().expect("collect helper output");
        assert!(output.status.success(), "stderr: {:?}", output.stderr);
        assert!(remaining.contains("FINISHED"), "output: {remaining:?}");
    }
}
