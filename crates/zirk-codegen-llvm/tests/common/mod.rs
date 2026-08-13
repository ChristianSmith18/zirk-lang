//! Helpers shared by the backend integration tests.
//!
//! Cargo compiles this module into each test binary separately, so each one
//! sees whatever only the other needs as unused.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes access to LLVM across tests in the same binary.
///
/// LLVM keeps global process state —target registry, error handling— that does
/// not tolerate concurrent use, even when each thread has its own `Context`.
/// The Rust harness runs tests in parallel, and without this serialization the
/// binary aborts with an access violation on Windows.
///
/// Mutex poisoning is recovered from on purpose: if a test fails while holding
/// it, the others must be able to continue and report their own result rather
/// than failing in cascade over someone else's panic.
pub fn llvm_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Container format detected from the bytes of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedContainer {
    MachO,
    Elf,
    Coff,
    Unknown,
}

/// Identifies the container by reading the object file header.
///
/// Bytes are inspected rather than invoking `file(1)` so the test behaves the
/// same on all three platforms.
pub fn detect_container(bytes: &[u8]) -> DetectedContainer {
    if bytes.len() < 4 {
        return DetectedContainer::Unknown;
    }

    // ELF: 0x7F 'E' 'L' 'F'
    if bytes[..4] == [0x7F, b'E', b'L', b'F'] {
        return DetectedContainer::Elf;
    }

    // Mach-O: MH_MAGIC / MH_MAGIC_64 y sus variantes big-endian.
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if matches!(magic, 0xFEED_FACE | 0xFEED_FACF | 0xCEFA_EDFE | 0xCFFA_EDFE) {
        return DetectedContainer::MachO;
    }

    // COFF has no magic: it starts with the machine type. It is validated
    // against the set produced by the Zirk target matrix.
    let machine = u16::from_le_bytes([bytes[0], bytes[1]]);
    if matches!(machine, 0x014C | 0x8664 | 0xAA64) {
        return DetectedContainer::Coff;
    }

    DetectedContainer::Unknown
}

/// Architecture declared in the header, for containers that expose it at a
/// fixed position.
pub fn detect_machine(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 2 {
        return None;
    }
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Temporary directory owned by these tests, provided by Cargo.
pub fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).expect("could not create the temporary directory");
    dir
}

/// Path to `clang`, used as the link driver.
///
/// The `clang` from the pinned LLVM installation is preferred: the project
/// already requires that installation, and using it avoids depending on
/// whatever C compiler each host has, which differs across the three platforms.
pub fn linker_driver() -> PathBuf {
    if let Ok(prefix) = std::env::var("LLVM_SYS_201_PREFIX") {
        let candidate = Path::new(&prefix).join("bin").join(exe("clang"));
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(exe("clang"))
}

pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}
