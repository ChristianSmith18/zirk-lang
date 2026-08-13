//! # zirk-cli
//!
//! **Responsibility:** the `zirk` executable. It orchestrates the pipeline
//! stages, invokes the linker and presents diagnostics to the user.
//!
//! **Boundary:** the CLI implements no compilation logic of its own. Each stage
//! lives in its crate; here they are merely coordinated and their result is
//! translated into terminal output and exit codes.
//!
//! # State
//!
//! The subcommands of `ZIRK_COMPILER_SPEC.md` section 9 (`run`, `build`,
//! `check`, `test`, ...) arrive from Phase 1 onwards. In Phase 0 the CLI only
//! reports toolchain status, which is all that exists so far.

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
