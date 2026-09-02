//! The `zirk-check` executable: a frontend-only validator.
//!
//! This binary is intentionally smaller than `zirk` itself: it does not depend
//! on the backend and can be built without LLVM. It reuses the same frontend
//! driver so `zirk-check` and `zirk check` cannot drift.

use zirk_cli::{codes, frontend};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn dispatch(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("zirk-check {VERSION}");
            0
        }
        Some("--help" | "-h") | None => {
            help();
            0
        }
        Some(path) => check(path, args),
    }
}

fn check(path: &str, args: &[String]) -> i32 {
    if path == "--json" || path.starts_with("--color=") {
        return fail(
            codes::INVALID_USAGE,
            "no source file was given",
            "`zirk-check` needs a source file",
            Some("write `zirk-check program` or `zirk-check program.zrk`"),
        );
    }

    let json = args.iter().any(|a| a == "--json");
    let color = frontend::resolve_color(color_choice(args).as_deref());
    let file = frontend::resolve_source_path(path);
    let mut sink = zirk_diagnostics::DiagnosticSink::new();
    let _ = frontend::run_frontend(&file, &mut sink);

    if !sink.is_empty() {
        eprint!("{}", frontend::render(&sink, json, color));
    }

    if sink.has_errors() { 1 } else { 0 }
}

fn color_choice(args: &[String]) -> Option<String> {
    args.iter()
        .find_map(|a| a.strip_prefix("--color=").map(|c| c.to_string()))
}

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
    println!("zirk-check {VERSION}");
    println!();
    println!("Usage: zirk-check <file>");
    println!();
    println!("Validate a `.zrk` file using only the frontend (no LLVM).");
    println!();
    println!("The `.zrk` extension is optional: `main` and `main.zrk` are equivalent.");
    println!();
    println!("Options:");
    println!("      --json         emit diagnostics in structured form");
    println!("      --color=<when> always, never or auto (default: auto)");
    println!("  -V, --version      show the version");
    println!("  -h, --help         show this help");
}
