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
            eprintln!("error[E0000]: unknown subcommand: `{other}`");
            eprintln!("  = cause: the Phase 0 CLI implements no compilation subcommands");
            eprintln!("  = help: run `zirk --help` to see what is available");
            std::process::exit(1);
        }
    }
}

fn help() {
    println!("zirk {VERSION}");
    println!();
    println!("Usage: zirk <option>");
    println!();
    println!("Options:");
    println!("  -V, --version      show the version");
    println!("      --list-targets list the supported targets");
    println!("  -h, --help         show this help");
    println!();
    println!("Compilation subcommands (run, build, check, test) arrive in Phase 1.");
}

fn list_targets() {
    println!("host: {}", host_triple());
    println!();
    for target in TARGETS {
        println!("  {:<16} {}", target.name, target.triple);
    }
}
