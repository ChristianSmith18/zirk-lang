//! # zirk-codegen-llvm
//!
//! **Responsabilidad:** traducir la IR de `zirk-ir` a LLVM IR y emitir archivos
//! objeto para los targets de `ZIRK_COMPILER_SPEC.md` sección 6.
//!
//! **Límite:** es el único crate que conoce LLVM. Esa contención es lo que hace
//! posible añadir otro backend más adelante sin tocar la semántica pública, como
//! anticipa `ZIRK_COMPILER_SPEC.md` sección 5.
//!
//! El enlace final del ejecutable **no** ocurre acá: este crate produce objetos.
//! Invocar el linker es responsabilidad de `zirk-cli`.
//!
//! # Estado
//!
//! En Fase 0 existe la infraestructura de targets y emisión, verificada de punta
//! a punta, pero no la traducción desde `zirk-ir` — esa IR todavía está vacía.

use inkwell::OptimizationLevel;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use std::path::Path;
use std::sync::Once;
use zirk_diagnostics::{Diagnostic, DiagnosticResult};

/// Códigos de diagnóstico de este crate.
pub mod codes {
    use zirk_diagnostics::Code;

    /// El triple solicitado no es reconocido por el LLVM enlazado.
    pub const TARGET_DESCONOCIDO: Code = Code::new("E0100");
    /// LLVM no pudo construir una máquina de destino para el triple.
    pub const TARGET_MACHINE_NO_DISPONIBLE: Code = Code::new("E0101");
    /// Fallo al escribir el archivo objeto.
    pub const EMISION_FALLIDA: Code = Code::new("E0102");
}

/// Formato de contenedor del archivo objeto producido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    MachO,
    Elf,
    Coff,
}

/// Un target soportado por Zirk.
///
/// El nombre canónico es el de `ZIRK_COMPILER_SPEC.md` sección 6; el triple es
/// su equivalente para LLVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZirkTarget {
    pub name: &'static str,
    pub triple: &'static str,
    pub container: Container,
}

/// Los targets del spec.
///
/// La validez real depende del sistema operativo, del linker, del SDK y de las
/// dependencias nativas; esta tabla describe lo que el backend debe **emitir**,
/// no lo que se puede enlazar sin sysroot.
///
/// Se preserva la alineación en columnas: es una tabla de datos y expandirla a
/// un campo por línea la vuelve considerablemente menos legible.
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

/// Emite una traza de progreso del backend cuando `ZIRK_TRACE` está definida.
///
/// Existe porque un fallo dentro de LLVM puede abortar el proceso sin dejar
/// mensaje ni backtrace, y en esa situación la única forma de ubicar el punto
/// exacto es haber impreso antes de llegar. Ver el issue #2.
fn traza(paso: &str) {
    if std::env::var_os("ZIRK_TRACE").is_some() {
        eprintln!("[zirk-codegen] {paso}");
    }
}

/// Busca un target por su nombre canónico de Zirk.
pub fn target_by_name(name: &str) -> Option<&'static ZirkTarget> {
    TARGETS.iter().find(|t| t.name == name)
}

/// Inicializa los targets de LLVM. Idempotente y seguro entre threads.
///
/// La inicialización de targets de LLVM **no** es thread-safe: registra
/// estructuras globales del proceso. Sin esta guarda, dos llamadas simultáneas
/// corrompen ese estado. El harness de tests de Rust ejecuta en paralelo, así
/// que la condición se da con facilidad.
pub fn initialize_targets() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Target::initialize_all(&InitializationConfig::default());
    });
}

/// Triple del host donde corre el compilador.
pub fn host_triple() -> String {
    traza("consultando el triple por defecto");
    TargetMachine::get_default_triple()
        .as_str()
        .to_string_lossy()
        .into_owned()
}

/// Construye la máquina de destino para un triple arbitrario.
///
/// Falla con diagnóstico si el triple no es reconocido, en vez de producir un
/// objeto inválido.
fn target_machine(triple: &str) -> DiagnosticResult<TargetMachine> {
    traza("inicializando targets");
    initialize_targets();

    traza("creando triple");
    let target_triple = TargetTriple::create(triple);

    traza("buscando target para el triple");
    let target = Target::from_triple(&target_triple).map_err(|error| {
        Diagnostic::error(
            codes::TARGET_DESCONOCIDO,
            format!("target no soportado: `{triple}`"),
        )
        .with_cause(format!("LLVM no reconoce el triple: {error}"))
        .with_help("consultá `zirk build --list-targets` para ver los targets disponibles")
        .boxed()
    })?;

    // RelocMode::Default deja que LLVM elija el modelo correcto para cada
    // target. Forzar PIC es un concepto de Unix que no corresponde a COFF y
    // provocaba una violación de acceso al emitir para Windows.
    traza("creando target machine");
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
                codes::TARGET_MACHINE_NO_DISPONIBLE,
                format!("no se pudo construir la máquina de destino para `{triple}`"),
            )
            .with_cause("el LLVM enlazado no incluye soporte para esta arquitectura")
            .with_help("verificá que la instalación de LLVM incluya el backend correspondiente")
            .boxed()
        })
}

/// Emite un archivo objeto para el triple indicado.
pub fn emit_object_for_triple(
    module: &Module<'_>,
    triple: &str,
    output: &Path,
) -> DiagnosticResult<()> {
    let machine = target_machine(triple)?;

    traza("escribiendo el objeto a disco");
    machine
        .write_to_file(module, FileType::Object, output)
        .map_err(|error| {
            Diagnostic::error(
                codes::EMISION_FALLIDA,
                format!("no se pudo emitir el objeto en `{}`", output.display()),
            )
            .with_cause(error.to_string())
            .with_help("verificá permisos de escritura sobre el directorio de salida")
            .boxed()
        })
}

/// Emite un archivo objeto para un target del spec.
pub fn emit_object(
    module: &Module<'_>,
    target: &ZirkTarget,
    output: &Path,
) -> DiagnosticResult<()> {
    emit_object_for_triple(module, target.triple, output)
}

/// Emite un archivo objeto para el host.
///
/// No inicializa el target nativo por separado: `initialize_targets` ya
/// registra todos los targets, incluido el del host, y hacerlo por dos caminos
/// distintos reintroduce la condición de carrera que la guarda evita.
pub fn emit_object_for_host(module: &Module<'_>, output: &Path) -> DiagnosticResult<()> {
    emit_object_for_triple(module, &host_triple(), output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_tabla_cubre_los_nueve_targets_del_spec() {
        assert_eq!(TARGETS.len(), 9);
    }

    #[test]
    fn los_nombres_de_target_son_unicos() {
        let mut nombres: Vec<_> = TARGETS.iter().map(|t| t.name).collect();
        nombres.sort_unstable();
        let total = nombres.len();
        nombres.dedup();
        assert_eq!(nombres.len(), total);
    }

    #[test]
    fn se_puede_buscar_un_target_por_nombre() {
        let target = target_by_name("aarch64-macos").expect("target del spec");
        assert_eq!(target.triple, "aarch64-apple-darwin");
        assert_eq!(target.container, Container::MachO);
    }

    #[test]
    fn un_nombre_desconocido_no_resuelve() {
        assert!(target_by_name("riscv64-plan9").is_none());
    }
}
