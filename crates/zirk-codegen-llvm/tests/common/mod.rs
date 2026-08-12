//! Utilidades compartidas por los tests de integración del backend.
//!
//! Cargo compila este módulo dentro de cada binario de test por separado, así
//! que cada uno ve como no usado lo que solo necesita el otro.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Formato de contenedor detectado a partir de los bytes del archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedContainer {
    MachO,
    Elf,
    Coff,
    Desconocido,
}

/// Identifica el contenedor leyendo la cabecera del archivo objeto.
///
/// Se inspeccionan los bytes en vez de invocar `file(1)` para que el test
/// funcione igual en las tres plataformas.
pub fn detect_container(bytes: &[u8]) -> DetectedContainer {
    if bytes.len() < 4 {
        return DetectedContainer::Desconocido;
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

    // COFF no tiene magic: arranca con el tipo de máquina. Se valida contra el
    // conjunto que produce la matriz de targets de Zirk.
    let machine = u16::from_le_bytes([bytes[0], bytes[1]]);
    if matches!(machine, 0x014C | 0x8664 | 0xAA64) {
        return DetectedContainer::Coff;
    }

    DetectedContainer::Desconocido
}

/// Arquitectura declarada en la cabecera, para los contenedores que la exponen
/// en una posición fija.
pub fn detect_machine(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 2 {
        return None;
    }
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Directorio temporal propio de estos tests, provisto por Cargo.
pub fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).expect("no se pudo crear el directorio temporal");
    dir
}

/// Ruta a `clang`, usado como driver de enlace.
///
/// Se prefiere el `clang` de la instalación de LLVM pineada: el proyecto ya
/// exige esa instalación, y usarla evita depender del compilador de C que haya
/// en cada host, que es distinto en las tres plataformas.
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
