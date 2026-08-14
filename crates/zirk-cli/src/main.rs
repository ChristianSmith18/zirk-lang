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
//! Phase 1 implements `build` and `run` over a single file. The rest of the
//! subcommands of `ZIRK_COMPILER_SPEC.md` section 9 — `check`, `test`, `bench`,
//! `format`, `lint` — arrive in later phases, and each says so when invoked.

mod driver;
mod modules;

use driver::Action;
use std::path::{Path, PathBuf};

/// Diagnostic codes of the CLI.
pub mod codes {
    use zirk_diagnostics::Code;

    /// The source file could not be read.
    pub const UNREADABLE_FILE: Code = Code::new("E0501");
    /// The output directory could not be prepared.
    pub const OUTPUT_UNAVAILABLE: Code = Code::new("E0502");
    /// The linker failed or could not be invoked.
    pub const LINK_FAILED: Code = Code::new("E0503");
    /// The runtime static library was not found.
    pub const RUNTIME_NOT_FOUND: Code = Code::new("E0504");
    /// The produced executable could not be run.
    pub const EXECUTION_FAILED: Code = Code::new("E0505");
    /// A subcommand that arrives in a later phase.
    pub const NOT_IMPLEMENTED: Code = Code::new("E0506");
    /// Wrong invocation of the CLI.
    pub const INVALID_USAGE: Code = Code::new("E0507");
    /// The compiler produced invalid IR: a compiler bug.
    pub const INTERNAL_ERROR: Code = Code::new("E0508");
    /// The imports of a crate form a cycle.
    pub const IMPORT_CYCLE: Code = Code::new("E0509");
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Directory the artifacts are written to.
///
/// This resolves an open question of the design: `zirk run` leaves the
/// executable on disk instead of deleting it. Someone who ran their program
/// will most likely want to distribute it, and a compiler that hides its output
/// forces a second command to get it back.
const OUTPUT_DIR: &str = "build";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn dispatch(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("build") => compile(&args[1..], Action::Build),
        Some("run") => compile(&args[1..], Action::Run),

        Some("--version" | "-V") => {
            println!("zirk {VERSION}");
            0
        }
        Some("--list-targets") => {
            list_targets();
            0
        }
        Some("--help" | "-h") | None => {
            help();
            0
        }

        // Subcommands of `COMPILER_SPEC` section 9 that arrive later. Naming
        // them beats "unknown subcommand": the user wrote something the language
        // does define.
        Some(later @ ("check" | "test" | "bench" | "format" | "lint" | "new" | "init" | "doc")) => {
            let phase = match later {
                "check" => 2,
                "format" | "lint" => 9,
                "new" | "init" => 6,
                _ => 7,
            };
            fail(
                codes::NOT_IMPLEMENTED,
                &format!("`zirk {later}` is not implemented yet"),
                "the subcommand exists in the specification but arrives later",
                Some(&format!(
                    "it arrives in Phase {phase}; see docs/init/ZIRK_ROADMAP.md"
                )),
            )
        }

        Some(other) => fail(
            codes::INVALID_USAGE,
            &format!("unknown subcommand: `{other}`"),
            "the subcommand is not part of the language CLI",
            Some("run `zirk --help` to see what is available"),
        ),
    }
}

fn compile(args: &[String], action: Action) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let color = driver::resolve_color(color_choice(args).as_deref());
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if files.is_empty() {
        return fail(
            codes::INVALID_USAGE,
            "no source file was given",
            "the subcommand needs a `.zrk` file",
            Some("write `zirk run program.zrk`"),
        );
    }

    if files.len() > 1 {
        return fail(
            codes::NOT_IMPLEMENTED,
            "several source files were given",
            "this version of the compiler processes a single file",
            Some("multi-file projects with `init.zrk` arrive in Phase 6"),
        );
    }

    let path = Path::new(files[0].as_str());
    let output_dir = PathBuf::from(OUTPUT_DIR);
    let compilation = driver::compile(path, &output_dir);

    // Diagnostics go to standard error so they never mix with the output of the
    // compiled program.
    if !compilation.sink.is_empty() {
        eprint!("{}", driver::render(&compilation.sink, json, color));
    }

    let Some(executable) = compilation.executable else {
        return 1;
    };

    match action {
        Action::Build => {
            println!("{}", executable.display());
            0
        }
        Action::Run => match driver::run(&executable) {
            Ok(code) => code,
            Err(diagnostic) => {
                eprint!("{}", diagnostic.render_colored(color));
                1
            }
        },
    }
}

/// Reads `--color=<choice>` from the arguments.
fn color_choice(args: &[String]) -> Option<String> {
    args.iter()
        .find_map(|a| a.strip_prefix("--color=").map(|c| c.to_string()))
}

/// Emits a diagnostic of the CLI itself and returns the exit code.
fn fail(code: zirk_diagnostics::Code, message: &str, cause: &str, help: Option<&str>) -> i32 {
    let mut diagnostic = zirk_diagnostics::Diagnostic::error(code, message).with_cause(cause);
    if let Some(help) = help {
        diagnostic = diagnostic.with_help(help);
    }
    // The CLI's own diagnostics get the same treatment as the compiler's: the
    // reader cannot tell which layer produced them, and should not have to.
    eprint!("{}", diagnostic.render_colored(driver::resolve_color(None)));
    1
}

fn help() {
    println!("zirk {VERSION}");
    println!();
    println!("Usage: zirk <subcommand> [file]");
    println!();
    println!("Subcommands:");
    println!("  build <file.zrk>   compile to a native executable");
    println!("  run <file.zrk>     compile and run");
    println!();
    println!("Options:");
    println!("      --json         emit diagnostics in structured form");
    println!("      --color=<when> always, never or auto (default: auto)");
    println!("  -V, --version      show the version");
    println!("      --list-targets list the supported targets");
    println!("  -h, --help         show this help");
    println!();
    println!("Artifacts are written to `{OUTPUT_DIR}/`.");
}

fn list_targets() {
    println!("host: {}", zirk_codegen_llvm::host_triple());
    println!();
    for target in zirk_codegen_llvm::TARGETS {
        println!("  {:<16} {}", target.name, target.triple);
    }
}
