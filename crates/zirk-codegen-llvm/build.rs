//! Verifies the LLVM pin before `llvm-sys` attempts to link.
//!
//! Without this check, a wrong LLVM major version shows up as an unreadable
//! link error with hundreds of unresolved symbols. The project requires
//! diagnostics with a cause and help (`ZIRK_COMPILER_SPEC.md` section 8);
//! applying that to the compiler's own build is consistent.
//!
//! See `docs/decisions/ADR-001-pin-llvm.md` and `docs/TOOLCHAIN.md`.

use std::path::PathBuf;
use std::process::Command;

/// LLVM major version the project builds against.
/// Must match the `llvm20-1` inkwell feature in `Cargo.toml`.
const LLVM_MAJOR: u32 = 20;
const PREFIX_VAR: &str = "LLVM_SYS_201_PREFIX";

fn main() {
    println!("cargo:rerun-if-env-changed={PREFIX_VAR}");

    let llvm_config = match locate_llvm_config() {
        Some(path) => path,
        None => fail(
            "no LLVM installation was found",
            &format!("{PREFIX_VAR} is not set and there is no `llvm-config` on PATH"),
            &format!("install LLVM {LLVM_MAJOR}.1 and set {PREFIX_VAR}; see docs/TOOLCHAIN.md"),
        ),
    };

    let version = match llvm_version(&llvm_config) {
        Some(version) => version,
        None => fail(
            "could not determine the LLVM version",
            &format!("`{}` did not respond to --version", llvm_config.display()),
            "check that the LLVM installation is not corrupted; see docs/TOOLCHAIN.md",
        ),
    };

    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok())
        .unwrap_or(0);

    if major != LLVM_MAJOR {
        fail(
            &format!("incompatible LLVM major version: found {major}, required {LLVM_MAJOR}"),
            &format!(
                "`llvm-sys` links against the C++ ABI of a specific major version; \
                 {} reports {version}",
                llvm_config.display()
            ),
            &format!(
                "install LLVM {LLVM_MAJOR}.1 and point {PREFIX_VAR} at its prefix; see docs/TOOLCHAIN.md"
            ),
        );
    }
}

/// Looks for `llvm-config` first in the declared prefix, then on `PATH`.
fn locate_llvm_config() -> Option<PathBuf> {
    if let Ok(prefix) = std::env::var(PREFIX_VAR) {
        let candidate = PathBuf::from(prefix).join("bin").join(exe("llvm-config"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let from_path = PathBuf::from(exe("llvm-config"));
    llvm_version(&from_path).map(|_| from_path)
}

fn llvm_version(llvm_config: &PathBuf) -> Option<String> {
    let output = Command::new(llvm_config).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Emits the failure in the format of `COMPILER_SPEC` section 8 and aborts the
/// build.
fn fail(message: &str, cause: &str, help: &str) -> ! {
    // `cargo:warning` is the only channel Cargo shows untruncated from a build
    // script, so the diagnostic is emitted line by line.
    println!("cargo:warning=error[E0001]: {message}");
    println!("cargo:warning=  = cause: {cause}");
    println!("cargo:warning=  = help: {help}");
    panic!("{message}");
}
