//! # zirk-cli
//!
//! **Responsabilidad:** el ejecutable `zirk`. Orquesta las etapas del pipeline,
//! invoca el linker y presenta los diagnósticos al usuario.
//!
//! **Límite:** la CLI no implementa lógica de compilación propia. Cada etapa
//! vive en su crate; acá solo se las coordina y se traduce el resultado a
//! salida de terminal y exit codes.
//!
//! # Estado
//!
//! Los subcomandos de `ZIRK_COMPILER_SPEC.md` sección 9 (`run`, `build`,
//! `check`, `test`, ...) llegan a partir de Fase 1. En Fase 0 la CLI solo
//! reporta el estado del toolchain, que es lo único que ya existe.

use zirk_codegen_llvm::{TARGETS, host_triple};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version" | "-V") => println!("zirk {VERSION}"),
        Some("--list-targets") => list_targets(),
        Some("--help" | "-h") | None => help(),
        Some(other) => {
            eprintln!("error[E0000]: subcomando desconocido: `{other}`");
            eprintln!("  = causa: la CLI de Fase 0 no implementa subcomandos de compilación");
            eprintln!("  = ayuda: ejecutá `zirk --help` para ver lo disponible");
            std::process::exit(1);
        }
    }
}

fn help() {
    println!("zirk {VERSION}");
    println!();
    println!("Uso: zirk <opción>");
    println!();
    println!("Opciones:");
    println!("  -V, --version      muestra la versión");
    println!("      --list-targets lista los targets soportados");
    println!("  -h, --help         muestra esta ayuda");
    println!();
    println!("Los subcomandos de compilación (run, build, check, test) llegan en Fase 1.");
}

fn list_targets() {
    println!("host: {}", host_triple());
    println!();
    for target in TARGETS {
        println!("  {:<16} {}", target.name, target.triple);
    }
}
