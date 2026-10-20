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
        Some("format") => format(&args[1..]),
        #[cfg(not(target_arch = "wasm32"))]
        Some("permissions") => permissions(&args[1..]),
        Some("uninstall") => uninstall(&args[1..]),

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
        Some(later @ ("test" | "bench" | "lint" | "new" | "init" | "doc")) => {
            let phase = match later {
                "test" | "new" | "init" => Phase::SIX,
                "lint" => Phase::NINE,
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

fn format(args: &[String]) -> i32 {
    let _json = args.iter().any(|arg| arg == "--json");
    let check = args.iter().any(|arg| arg == "--check");
    let diff = args.iter().any(|arg| arg == "--diff");
    let files: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if files.is_empty() {
        return fail(
            codes::INVALID_USAGE,
            "`zirk format` needs at least one source file or directory",
            "formatting works independently for every target",
            Some("write `zirk format [--check] [--diff] program.zrk src/`"),
        );
    }
    let mut paths = Vec::new();
    for file in files {
        let raw = std::path::Path::new(file);
        let path = if raw.exists() {
            raw.to_path_buf()
        } else {
            frontend::resolve_source_path(file)
        };
        collect_format_targets(&path, &mut paths);
    }
    let mut failed = false;
    let mut drift = false;
    for path in paths {
        match zirk_cli::format::format_path_with_diff(&path, check, diff) {
            Ok(zirk_cli::format::Outcome::Unchanged) => {}
            Ok(zirk_cli::format::Outcome::Changed) if check => {
                eprintln!("{} needs formatting", path.display());
                drift = true;
            }
            Ok(zirk_cli::format::Outcome::Changed) => {}
            Ok(zirk_cli::format::Outcome::Invalid(diagnostic)) | Err(diagnostic) => {
                eprint!("{diagnostic}");
                failed = true;
            }
        }
    }
    i32::from(failed || drift)
}

fn collect_format_targets(path: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
    if path.is_file() {
        output.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        output.push(path.to_path_buf());
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            collect_format_targets(&child, output);
        } else if child
            .extension()
            .is_some_and(|extension| extension == "zrk")
        {
            output.push(child);
        }
    }
}

fn check(args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let types = args.iter().any(|a| a == "--types");
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

    let raw = std::path::Path::new(files[0]);
    let path = if raw.is_dir() || raw.file_name().is_some_and(|name| name == ".zkinit") {
        raw.to_path_buf()
    } else {
        frontend::resolve_source_path(files[0].as_str())
    };
    let mut sink = zirk_diagnostics::DiagnosticSink::new();
    let result = if types {
        // `--types` keeps the checked program even on errors: the map
        // reports what the checker managed to resolve.
        frontend::run_frontend_lenient(&path, &mut sink)
    } else {
        frontend::run_frontend(&path, &mut sink)
    };

    if !sink.is_empty() {
        eprint!("{}", frontend::render(&sink, json, color));
    }

    if types && let Some(result) = &result {
        println!(
            "{}",
            frontend::render_type_map(&result.checked, &result.sources)
        );
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

    let raw = std::path::Path::new(files[0]);
    let path = if raw.is_dir() || raw.file_name().is_some_and(|name| name == ".zkinit") {
        raw.to_path_buf()
    } else {
        frontend::resolve_source_path(files[0].as_str())
    };
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
        Action::Run => {
            let grant = match zirk_cli::approvals::runtime_token_for_path(&path) {
                Ok(grant) => grant,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    return 1;
                }
            };
            match driver::run_with_runtime_grant(&executable, grant.as_deref(), json) {
                Ok(code) => code,
                Err(diagnostic) => {
                    eprint!("{}", diagnostic.render_colored(color));
                    1
                }
            }
        }
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

#[cfg(not(target_arch = "wasm32"))]
fn permissions(args: &[String]) -> i32 {
    match args {
        [command, project] if command == "approve" || command == "revoke" => {
            let project = std::path::Path::new(project);
            let result = if command == "approve" {
                zirk_cli::approvals::approve(project)
            } else {
                zirk_cli::approvals::revoke(project)
            };
            match result {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    1
                }
            }
        }
        _ => fail(
            codes::INVALID_USAGE,
            "invalid permissions command",
            "permission grants are approved or revoked explicitly for a project manifest",
            Some("write `zirk permissions approve|revoke <project-directory-or-.zkinit>`"),
        ),
    }
}

fn uninstall(args: &[String]) -> i32 {
    if !args.is_empty() {
        return fail(
            codes::INVALID_USAGE,
            "`zirk uninstall` does not accept arguments",
            "uninstallation removes the current user's managed Zirk toolchain",
            Some("run `zirk uninstall` without arguments"),
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        fail(
            codes::NOT_IMPLEMENTED,
            "`zirk uninstall` is currently available only for macOS user installations",
            "this release manages the macOS `~/.local` installation layout",
            Some("remove this installation using its platform-specific package instructions"),
        )
    }

    #[cfg(target_os = "macos")]
    {
        match uninstall_macos_user_installation() {
            Ok(()) => 0,
            Err(message) => {
                eprintln!("ERROR: {message}");
                1
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn uninstall_macos_user_installation() -> Result<(), String> {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn is_zirk_version(path: &Path) -> bool {
        let manifest = path.join("manifest.json");
        let Ok(contents) = fs::read_to_string(manifest) else {
            return false;
        };
        contents.contains("\"name\": \"Zirk\"")
            && contents.contains("\"compiler\": \"bin/zirk\"")
            && path.join("bin/zirk").is_file()
    }

    let home =
        std::env::var_os("HOME").ok_or("HOME is not set; cannot locate the user installation")?;
    let home = PathBuf::from(home);
    let data_root = home.join(".local/share/zirk");
    let versions_root = data_root.join("versions");
    let current_link = data_root.join("current");
    let command_link = home.join(".local/bin/zirk");
    let expected_command_target = Path::new("../share/zirk/current/bin/zirk");

    println!("[1/3] Checking the active per-user Zirk installation...");
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the running compiler: {error}"))?
        .canonicalize()
        .map_err(|error| format!("could not resolve the running compiler path: {error}"))?;
    let versions_root_canonical = versions_root.canonicalize().map_err(|_| {
        "no managed Zirk installation was found under ~/.local/share/zirk/versions".to_string()
    })?;
    if !executable.starts_with(&versions_root_canonical) {
        return Err(
            "this compiler is not running from the managed ~/.local Zirk installation".into(),
        );
    }

    let current_target = current_link
        .canonicalize()
        .map_err(|_| "the active Zirk version link is missing or invalid".to_string())?;
    if !current_target.starts_with(&versions_root_canonical) || !is_zirk_version(&current_target) {
        return Err("the active version link does not point to a managed Zirk installation".into());
    }
    if !fs::read_link(&command_link)
        .map(|target| target == expected_command_target)
        .unwrap_or(false)
    {
        return Err("~/.local/bin/zirk is not the managed Zirk command link; refusing to remove an unrelated command".into());
    }

    let versions = fs::read_dir(&versions_root)
        .map_err(|error| format!("could not list installed Zirk versions: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_zirk_version(path))
        .collect::<Vec<_>>();
    if versions.is_empty() {
        return Err("no managed Zirk version directories were found; nothing was removed".into());
    }

    println!("      Found {} managed version(s).", versions.len());
    println!("[2/3] Removing the active command and version links...");
    fs::remove_file(&command_link)
        .map_err(|error| format!("could not remove ~/.local/bin/zirk: {error}"))?;
    println!("      Removed ~/.local/bin/zirk");
    fs::remove_file(&current_link)
        .map_err(|error| format!("could not remove ~/.local/share/zirk/current: {error}"))?;
    println!("      Removed ~/.local/share/zirk/current");

    println!("[3/3] Removing managed compiler and runtime versions...");
    for version in versions {
        fs::remove_dir_all(&version)
            .map_err(|error| format!("could not remove {}: {error}", version.display()))?;
        println!("      Removed {}", version.display());
    }
    if fs::read_dir(&versions_root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        fs::remove_dir(&versions_root)
            .map_err(|error| format!("could not remove the empty versions directory: {error}"))?;
    }
    let security_root = data_root.join("security");
    if let Ok(metadata) = fs::symlink_metadata(&security_root) {
        if metadata.file_type().is_dir() {
            fs::remove_dir_all(&security_root).map_err(|error| {
                format!("could not remove the Zirk permission approvals: {error}")
            })?;
        } else {
            fs::remove_file(&security_root).map_err(|error| {
                format!("could not remove the Zirk permission approval link: {error}")
            })?;
        }
        println!("      Removed ~/.local/share/zirk/security (approval key and grants)");
    }

    println!("Zirk has been uninstalled from this user's ~/.local directory.");
    println!("Release files in the Zirk repository were left untouched.");
    Ok(())
}

fn help() {
    println!("zirk {VERSION}");
    println!();
    println!("Usage: zirk <subcommand> [file]");
    println!();
    println!("Subcommands:");
    println!("  check <file>       validate the frontend only");
    println!("  format [--check] [--diff] <target>...  canonicalize Zirk source files");
    #[cfg(not(target_arch = "wasm32"))]
    println!("  permissions approve|revoke <project>  manage exact .zkinit runtime scopes");
    println!("  uninstall          remove the managed per-user Zirk installation");
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
