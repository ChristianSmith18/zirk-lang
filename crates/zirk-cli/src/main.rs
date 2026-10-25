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

use std::io::IsTerminal;
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
        Some("migrate") => migrate(&args[1..]),
        Some("init") => init(&args[1..]),
        Some("install") if args.len() == 1 => install(false),
        Some("update") if args.len() == 1 => install(true),
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
        Some(later @ ("test" | "bench" | "lint" | "new" | "doc")) => {
            let phase = match later {
                "test" | "new" => Phase::SIX,
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

fn migrate(args: &[String]) -> i32 {
    let apply = args.iter().any(|arg| arg == "--apply");
    let check = args.iter().any(|arg| arg == "--check");
    let paths: Vec<_> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if (apply && check)
        || paths.len() > 1
        || args
            .iter()
            .any(|arg| arg.starts_with("--") && arg != "--apply" && arg != "--check")
    {
        return fail(
            codes::INVALID_USAGE,
            "invalid `zirk migrate` arguments",
            "migration accepts one manifest path and either `--check` or `--apply`",
            Some("run `zirk migrate --check [project-or-.zkinit]` before `zirk migrate --apply`"),
        );
    }

    let provided = paths
        .first()
        .map(|value| std::path::Path::new(value.as_str()));
    let manifest = match provided {
        Some(path) if path.is_dir() => path.join(".zkinit"),
        Some(path) => path.to_path_buf(),
        None => std::path::PathBuf::from(".zkinit"),
    };
    match zirk_cli::project::migrate_manifest_file(&manifest, apply) {
        Ok((original, _converted)) if apply => {
            println!("Migrated {}", manifest.display());
            println!("Recoverable original: {}.pre-migrate", manifest.display());
            if legacy_entry_defines_main(&manifest, &original) {
                println!(
                    "Source note: the legacy entry defines `fn main`; migration did not rewrite or invoke it. Review the new top-level entry behavior separately."
                );
            }
            0
        }
        Ok((original, converted)) => {
            print_manifest_diff(&original, &converted);
            println!(
                "Preview only. Run `zirk migrate --apply {}` to write this conversion.",
                manifest.display()
            );
            if legacy_entry_defines_main(&manifest, &original) {
                println!(
                    "Source note: the legacy entry defines `fn main`; migration will not rewrite or invoke it."
                );
            }
            0
        }
        Err(error) => fail(
            codes::INVALID_USAGE,
            "could not migrate `.zkinit`",
            &error.to_string(),
            Some("run `zirk migrate --check` and resolve the reported legacy fields"),
        ),
    }
}

fn print_manifest_diff(original: &str, converted: &str) {
    println!("--- .zkinit (legacy)");
    println!("+++ .zkinit (canonical preview)");
    for line in original.lines() {
        println!("-{line}");
    }
    for line in converted.lines() {
        println!("+{line}");
    }
}

fn legacy_entry_defines_main(manifest_path: &std::path::Path, source: &str) -> bool {
    let Some(parent) = manifest_path.parent() else {
        return false;
    };
    let Ok(manifest) = zirk_cli::project::parse_legacy_manifest(source) else {
        return false;
    };
    if manifest.kind != zirk_cli::project::ProjectKind::Application {
        return false;
    }
    let Ok(entry) = std::fs::read_to_string(parent.join(manifest.entry)) else {
        return false;
    };
    let entry_path = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("<legacy-entry>");
    let source_file = zirk_diagnostics::SourceFile::new(entry_path.to_string_lossy(), &entry);
    let mut sink = zirk_diagnostics::DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(&source_file, &mut sink);
    let program = zirk_parser::parse(&source_file, &tokens, &mut sink);
    program
        .functions
        .iter()
        .any(|function| function.name.name == "main")
}

fn init(args: &[String]) -> i32 {
    if args.iter().any(|argument| argument != "-y") || args.len() > 1 {
        return fail(
            codes::INVALID_USAGE,
            "invalid `zirk init` arguments",
            "the initializer accepts no arguments or `-y`",
            Some("run `zirk init` for prompts or `zirk init -y` for canonical defaults"),
        );
    }
    let mode = if args.first().is_some_and(|argument| argument == "-y") {
        zirk_cli::initializer::InitMode::Yes
    } else {
        if !std::io::stdin().is_terminal() {
            return fail(
                codes::INVALID_USAGE,
                "interactive `zirk init` needs a terminal",
                "stdin is not attached to a terminal, so no answers can be collected",
                Some("run `zirk init -y` to create a manifest with the documented defaults"),
            );
        }
        zirk_cli::initializer::InitMode::Interactive
    };
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            return fail(
                codes::INVALID_USAGE,
                "could not locate the current directory",
                &error.to_string(),
                None,
            );
        }
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    match zirk_cli::initializer::initialize(&root, mode, &mut stdin.lock(), &mut stdout.lock()) {
        Ok(path) => {
            println!("Created {}", path.display());
            0
        }
        Err(error) => fail(
            codes::INVALID_PROJECT,
            "could not initialize the project",
            &error,
            Some("review the answers and run `zirk init` again; no existing manifest was changed"),
        ),
    }
}

fn install(update: bool) -> i32 {
    match zirk_cli::packages::install_current_project(update) {
        Ok(lock) => {
            let action = if update { "Updated" } else { "Installed" };
            println!("{action} {} locked dependencies.", lock.packages.len());
            0
        }
        Err(error) => fail(
            codes::INVALID_PROJECT,
            if update {
                "could not update project dependencies"
            } else {
                "could not install project dependencies"
            },
            &error.to_string(),
            Some("check `.zkinit`, the configured registry, and the requested source revisions"),
        ),
    }
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
    let debug_info = args.iter().any(|a| a == "--debug-info");
    let color = frontend::resolve_color(color_choice(args).as_deref());
    let mut files = Vec::new();
    let mut explicit_target = None;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        if argument == "--target" {
            if explicit_target.is_some() || index + 1 >= args.len() {
                return fail(
                    codes::INVALID_USAGE,
                    "invalid `--target` option",
                    "the option requires one target name and may appear only once",
                    Some("run `zirk --list-targets` to see supported targets"),
                );
            }
            index += 1;
            explicit_target = Some(args[index].clone());
        } else if let Some(target) = argument.strip_prefix("--target=") {
            if explicit_target.replace(target.to_string()).is_some() || target.is_empty() {
                return fail(
                    codes::INVALID_USAGE,
                    "invalid `--target` option",
                    "the option requires one target name and may appear only once",
                    Some("run `zirk --list-targets` to see supported targets"),
                );
            }
        } else if !argument.starts_with("--") {
            files.push(&args[index]);
        }
        index += 1;
    }

    if action == Action::Run && explicit_target.is_some() {
        return fail(
            codes::INVALID_USAGE,
            "`zirk run` does not accept `--target`",
            "run always uses the compiler host target",
            Some("remove `--target` or use `zirk build --target <target>`"),
        );
    }

    let directory_argument = files.first().is_some_and(|file| {
        let path = std::path::Path::new(file);
        path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit")
    });
    let pathless_run = action == Action::Run && (files.is_empty() || directory_argument);

    if files.is_empty() && !pathless_run {
        return fail(
            codes::INVALID_USAGE,
            "no source file was given",
            "`zirk build` needs a source file or project entry",
            Some("write `zirk build program` or `zirk build program.zrk`"),
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

    let project_start = if let Some(file) = files.first() {
        let path = std::path::Path::new(file);
        if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
            path
        } else {
            std::path::Path::new(".")
        }
    } else {
        std::path::Path::new(".")
    };
    let path = if pathless_run {
        let Some(manifest_path) = zirk_cli::project::discover(project_start) else {
            return fail(
                codes::INVALID_PROJECT,
                "pathless `zirk run` requires an application `.zkinit` entry",
                "no project manifest was found from the current directory",
                Some("run `zirk init` or provide an explicit source path"),
            );
        };
        match zirk_cli::project::read_manifest(&manifest_path) {
            Ok(manifest) if manifest.kind == zirk_cli::project::ProjectKind::Application => {}
            Ok(_) => {
                return fail(
                    codes::INVALID_PROJECT,
                    "pathless `zirk run` cannot start a package project",
                    "a package has a reusable root, not an application entry",
                    Some("provide an explicit source file to run within the package context"),
                );
            }
            Err(error) => {
                return fail(
                    codes::INVALID_PROJECT,
                    "could not read the project manifest",
                    &error.to_string(),
                    Some("repair `.zkinit` or run `zirk migrate --check`"),
                );
            }
        }
        manifest_path
    } else {
        let raw = std::path::Path::new(files[0]);
        if raw.is_dir() || raw.file_name().is_some_and(|name| name == ".zkinit") {
            raw.to_path_buf()
        } else {
            frontend::resolve_source_path(files[0].as_str())
        }
    };

    if action == Action::Build {
        let manifest = zirk_cli::project::discover(&path)
            .map(|manifest_path| zirk_cli::project::read_manifest(&manifest_path))
            .transpose();
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                return fail(
                    codes::INVALID_PROJECT,
                    "could not read the project manifest",
                    &error.to_string(),
                    Some("repair `.zkinit` or run `zirk migrate --check`"),
                );
            }
        };
        if let Some(manifest) = &manifest {
            if manifest.kind == zirk_cli::project::ProjectKind::Package
                && !manifest.build_targets.is_empty()
            {
                return fail(
                    codes::INVALID_PROJECT,
                    "a package cannot select application build targets",
                    "`build_targets` is an application-only manifest field",
                    Some("remove `build_targets` from the package `.zkinit`"),
                );
            }
        }
        let host_target = host_target_name();
        let selected = select_build_targets(
            explicit_target.as_deref(),
            manifest
                .as_ref()
                .map(|manifest| manifest.build_targets.as_slice())
                .unwrap_or_default(),
            host_target,
        );
        let selected = match selected {
            Ok(selected) => selected,
            Err(error) => {
                return fail(
                    codes::INVALID_USAGE,
                    "unsupported build target selection",
                    &error,
                    Some("run `zirk --list-targets` to see supported targets"),
                );
            }
        };
        if let Some(target) = selected
            .iter()
            .find(|target| target.as_str() != host_target)
        {
            return fail(
                codes::NOT_IMPLEMENTED,
                &format!("native linking for `{target}` is not available in this installation"),
                "the compiler can emit objects for this target, but a matching Zirk runtime archive and linker sysroot are not configured",
                Some(
                    "build for the host target or install the matching cross-compilation toolchain and runtime",
                ),
            );
        }
    }

    let output_dir = PathBuf::from(OUTPUT_DIR);
    let compilation = if debug_info {
        driver::compile_with_debug_info(&path, &output_dir)
    } else {
        driver::compile(&path, &output_dir)
    };

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
            let runtime_environment = match zirk_cli::approvals::runtime_environment_for_path(&path)
            {
                Ok(environment) => environment,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    return 1;
                }
            };
            match driver::run_with_runtime_permissions(
                &executable,
                grant.as_deref(),
                json,
                runtime_environment.as_ref(),
            ) {
                Ok(code) => code,
                Err(diagnostic) => {
                    eprint!("{}", diagnostic.render_colored(color));
                    1
                }
            }
        }
    }
}

#[cfg(feature = "backend")]
fn select_build_targets(
    explicit: Option<&str>,
    manifest_targets: &[String],
    host: &str,
) -> Result<Vec<String>, String> {
    if let Some(target) = explicit {
        if zirk_codegen_llvm::target_by_name(target).is_none() {
            return Err(format!("`{target}` is not a supported Zirk target"));
        }
        return Ok(vec![target.to_string()]);
    }
    if !manifest_targets.is_empty() {
        return Ok(manifest_targets.to_vec());
    }
    if zirk_codegen_llvm::target_by_name(host).is_none() {
        return Err(format!("host target `{host}` is not supported by Zirk"));
    }
    Ok(vec![host.to_string()])
}

#[cfg(feature = "backend")]
fn host_target_name() -> &'static str {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86", "windows") => "x86-windows",
        ("x86_64", "windows") => "x86_64-windows",
        ("aarch64", "windows") => "aarch64-windows",
        ("x86", "linux") => "x86-linux",
        ("x86_64", "linux") => "x86_64-linux",
        ("arm", "linux") => "armv7-linux",
        ("aarch64", "linux") => "aarch64-linux",
        ("x86_64", "macos") => "x86_64-macos",
        ("aarch64", "macos") => "aarch64-macos",
        _ => "unsupported-host",
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
        [command, project] if command == "inspect" => {
            match zirk_cli::approvals::inspect(std::path::Path::new(project)) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    1
                }
            }
        }
        [command, project] if command == "diff" => {
            match zirk_cli::approvals::diff(std::path::Path::new(project)) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    1
                }
            }
        }
        [command, project] if command == "history" => {
            match zirk_cli::approvals::history(std::path::Path::new(project)) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ERROR: {error}");
                    1
                }
            }
        }
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
            "requirements and exact consent can be inspected or diffed; approval is requested at protected operations and recorded in history",
            Some(
                "write `zirk permissions inspect|diff|history|revoke <project-directory-or-.zkinit>`; exact consent is requested by `zirk run` at the protected operation",
            ),
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
    println!(
        "  migrate --check|--apply [project-or-.zkinit]  preview or apply legacy manifest migration"
    );
    println!("  init [-y]          create a canonical `.zkinit` manifest");
    println!("  install            resolve dependencies and write `zirk.lock`");
    println!("  update             refresh locked dependency selections");
    #[cfg(not(target_arch = "wasm32"))]
    println!(
        "  permissions inspect|diff|history|revoke <project>  review requirements and saved consent"
    );
    println!("  uninstall          remove the managed per-user Zirk installation");
    #[cfg(feature = "backend")]
    {
        println!("  build <file>       compile to a native executable");
        println!("      --debug-info   include source-level DWARF for native debugging");
        println!("      --target NAME  select one supported build target");
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

#[cfg(test)]
mod tests {
    use super::dispatch;

    #[cfg(feature = "backend")]
    #[test]
    fn build_target_selection_obeys_cli_manifest_host_precedence() {
        let manifest = vec!["x86_64-macos".to_string(), "aarch64-macos".to_string()];
        assert_eq!(
            super::select_build_targets(None, &manifest, "aarch64-macos").unwrap(),
            manifest
        );
        assert_eq!(
            super::select_build_targets(Some("aarch64-macos"), &manifest, "x86_64-macos").unwrap(),
            ["aarch64-macos"]
        );
        assert_eq!(
            super::select_build_targets(None, &[], "x86_64-macos").unwrap(),
            ["x86_64-macos"]
        );
    }

    #[test]
    fn migrate_check_and_apply_have_explicit_write_boundary() {
        let root = std::env::temp_dir().join(format!("zirk-cli-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let manifest = root.join(".zkinit");
        let legacy = "project \"cli.migrate\"\nkind application\n";
        std::fs::write(&manifest, legacy).unwrap();
        let path = manifest.to_string_lossy().to_string();

        assert_eq!(
            dispatch(&["migrate".into(), "--check".into(), path.clone()]),
            0
        );
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), legacy);
        assert_eq!(dispatch(&["migrate".into(), "--apply".into(), path]), 0);
        let converted = std::fs::read_to_string(&manifest).unwrap();
        assert!(converted.contains("type: application"));
        assert_eq!(
            std::fs::read_to_string(root.join(".zkinit.pre-migrate")).unwrap(),
            legacy
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(feature = "backend")]
fn list_targets() {
    println!("host: {}", zirk_codegen_llvm::host_triple());
    println!();
    for target in zirk_codegen_llvm::TARGETS {
        println!("  {:<16} {}", target.name, target.triple);
    }
}
