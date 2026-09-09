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
//! Phase 1 implements `build` and `run` over a single file. `check` is now
//! available as a frontend-only validation pass. The rest of the subcommands of
//! `ZIRK_COMPILER_SPEC.md` section 9 — `test`, `bench`, `format`, `lint` —
//! arrive in later phases, and each says so when invoked.

#[cfg(feature = "backend")]
use std::path::PathBuf;
use zirk_cli::{codes, frontend};
use zirk_diagnostics::Phase;

#[cfg(feature = "backend")]
use zirk_cli::driver::{self, Action};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Directory the artifacts are written to.
const OUTPUT_DIR: &str = "build";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn dispatch(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("check") => check(&args[1..]),

        #[cfg(feature = "backend")]
        Some("build") => compile(&args[1..], Action::Build),
        #[cfg(feature = "backend")]
        Some("run") => compile(&args[1..], Action::Run),

        Some("--version" | "-V") => {
            println!("zirk {VERSION}");
            0
        }
        Some("--list-targets") => {
            #[cfg(feature = "backend")]
            {
                list_targets();
                0
            }
            #[cfg(not(feature = "backend"))]
            fail(
                codes::NOT_IMPLEMENTED,
                "`zirk --list-targets` is not available in this build",
                "this build of the compiler was compiled without the backend",
                Some("build with the `backend` feature enabled"),
            )
        }
        Some("--help" | "-h") | None => {
            help();
            0
        }

        // Subcommands of `COMPILER_SPEC` section 9 that arrive later.
        Some(later @ ("test" | "bench" | "format" | "lint" | "new" | "init" | "doc")) => {
            let phase = match later {
                "test" | "new" | "init" => Phase::SIX,
                "format" | "lint" => Phase::NINE,
                _ => Phase::SEVEN,
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

fn check(args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let color = frontend::resolve_color(color_choice(args).as_deref());
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if files.is_empty() {
        return fail(
            codes::INVALID_USAGE,
            "no source file was given",
            "`zirk check` needs a source file",
            Some("write `zirk check program` or `zirk check program.zrk`"),
        );
    }

    if files.len() > 1 {
        return fail(
            codes::NOT_IMPLEMENTED,
            "several source files were given",
            "this version of the compiler checks a single file",
            Some("multi-file projects with `.zkinit` arrive in Phase 6"),
        );
    }

    let path = frontend::resolve_source_path(files[0].as_str());
    let mut sink = zirk_diagnostics::DiagnosticSink::new();
    let _ = frontend::run_frontend(&path, &mut sink);

    if !sink.is_empty() {
        eprint!("{}", frontend::render(&sink, json, color));
    }

    if sink.has_errors() { 1 } else { 0 }
}

#[cfg(feature = "backend")]
fn compile(args: &[String], action: Action) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let color = frontend::resolve_color(color_choice(args).as_deref());
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if files.is_empty() {
        return fail(
            codes::INVALID_USAGE,
            "no source file was given",
            "the subcommand needs a source file",
            Some("write `zirk run program` or `zirk run program.zrk`"),
        );
    }

    if files.len() > 1 {
        return fail(
            codes::NOT_IMPLEMENTED,
            "several source files were given",
            "this version of the compiler processes a single file",
            Some("multi-file projects with `.zkinit` arrive in Phase 6"),
        );
    }

    let path = frontend::resolve_source_path(files[0].as_str());
    let output_dir = PathBuf::from(OUTPUT_DIR);
    let compilation = driver::compile(&path, &output_dir);

    if !compilation.sink.is_empty() {
        eprint!("{}", frontend::render(&compilation.sink, json, color));
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

#[cfg(not(feature = "backend"))]
#[allow(dead_code)]
fn compile(_args: &[String], _action: &str) -> i32 {
    fail(
        codes::NOT_IMPLEMENTED,
        "`zirk build` and `zirk run` are not available in this build",
        "this build of the compiler was compiled without the backend",
        Some("build with the `backend` feature enabled"),
    )
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
    eprint!(
        "{}",
        diagnostic.render_colored(frontend::resolve_color(None))
    );
    1
}

fn help() {
    println!("zirk {VERSION}");
    println!();
    println!("Usage: zirk <subcommand> [file]");
    println!();
    println!("Subcommands:");
    println!("  check <file>       validate the frontend only");
    #[cfg(feature = "backend")]
    {
        println!("  build <file>       compile to a native executable");
        println!("  run <file>         compile and run");
    }
    println!();
    println!("Options:");
    println!("      --json         emit diagnostics in structured form");
    println!("      --color=<when> always, never or auto (default: auto)");
    println!("  -V, --version      show the version");
    println!("      --list-targets list the supported targets");
    println!("  -h, --help         show this help");
    println!();
    println!("The `.zrk` extension is optional: `main` and `main.zrk` are equivalent.");
    println!();
    println!("Artifacts are written to `{OUTPUT_DIR}/`.");
}

#[cfg(feature = "backend")]
fn list_targets() {
    println!("host: {}", zirk_codegen_llvm::host_triple());
    println!();
    for target in zirk_codegen_llvm::TARGETS {
        println!("  {:<16} {}", target.name, target.triple);
    }
}
