//! Verifica el pin de LLVM antes de que `llvm-sys` intente enlazar.
//!
//! Sin esta comprobación, una versión mayor equivocada de LLVM se manifiesta
//! como un error de enlace ilegible con cientos de símbolos no resueltos. El
//! proyecto exige diagnósticos con causa y ayuda (`ZIRK_COMPILER_SPEC.md`
//! sección 8); aplicárselo al propio build del compilador es consistente.
//!
//! Ver `docs/decisions/ADR-001-pin-llvm.md` y `docs/TOOLCHAIN.md`.

use std::path::PathBuf;
use std::process::Command;

/// Versión mayor de LLVM contra la que se construye el proyecto.
/// Debe coincidir con la feature `llvm20-1` de inkwell en `Cargo.toml`.
const LLVM_MAJOR: u32 = 20;
const PREFIX_VAR: &str = "LLVM_SYS_201_PREFIX";

fn main() {
    println!("cargo:rerun-if-env-changed={PREFIX_VAR}");

    let llvm_config = match locate_llvm_config() {
        Some(path) => path,
        None => fail(
            "no se encontró una instalación de LLVM",
            &format!(
                "no está definida la variable {PREFIX_VAR} y no hay un `llvm-config` en el PATH"
            ),
            &format!("instalá LLVM {LLVM_MAJOR}.1 y definí {PREFIX_VAR}; ver docs/TOOLCHAIN.md"),
        ),
    };

    let version = match llvm_version(&llvm_config) {
        Some(version) => version,
        None => fail(
            "no se pudo determinar la versión de LLVM",
            &format!("`{}` no respondió a --version", llvm_config.display()),
            "verificá que la instalación de LLVM no esté corrupta; ver docs/TOOLCHAIN.md",
        ),
    };

    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok())
        .unwrap_or(0);

    if major != LLVM_MAJOR {
        fail(
            &format!(
                "versión mayor de LLVM incompatible: se encontró {major}, se requiere {LLVM_MAJOR}"
            ),
            &format!(
                "`llvm-sys` enlaza contra la ABI de C++ de una versión mayor concreta; \
                 {} reporta {version}",
                llvm_config.display()
            ),
            &format!(
                "instalá LLVM {LLVM_MAJOR}.1 y apuntá {PREFIX_VAR} a su prefijo; ver docs/TOOLCHAIN.md"
            ),
        );
    }
}

/// Busca `llvm-config` primero en el prefijo declarado y después en el `PATH`.
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

/// Emite el fallo con el formato de `COMPILER_SPEC` sección 8 y aborta el build.
fn fail(message: &str, cause: &str, help: &str) -> ! {
    // `cargo:warning` es el único canal que Cargo muestra sin truncar desde un
    // build script, así que el diagnóstico se emite línea por línea.
    println!("cargo:warning=error[E0001]: {message}");
    println!("cargo:warning=  = causa: {cause}");
    println!("cargo:warning=  = ayuda: {help}");
    panic!("{message}");
}
