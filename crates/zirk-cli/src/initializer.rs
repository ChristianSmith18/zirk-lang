//! Creation of a new canonical `.zkinit` manifest.

use std::fs::OpenOptions;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::project::{
    EnvironmentPermissions, NetworkPermissions, ProjectKind, ProjectManifest, format_manifest,
    parse_manifest,
};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitMode {
    Interactive,
    Yes,
}

/// Validates and writes `.zkinit` in `root`. No source or consent files are
/// created. Existing manifests are never replaced.
pub fn initialize<R: BufRead, W: Write>(
    root: &Path,
    mode: InitMode,
    input: &mut R,
    output: &mut W,
) -> Result<PathBuf, String> {
    let manifest_path = root.join(".zkinit");
    if std::fs::symlink_metadata(&manifest_path).is_ok() {
        return Err(format!(
            "`{}` already exists; `zirk init` never overwrites a manifest",
            manifest_path.display()
        ));
    }

    let manifest = match mode {
        InitMode::Yes => default_manifest(root),
        InitMode::Interactive => prompt_manifest(root, input, output)?,
    };
    let formatted = format_manifest(&manifest);
    let parsed = parse_manifest(&formatted)
        .map_err(|error| format!("the proposed `.zkinit` is invalid: {error}"))?;
    let canonical = format_manifest(&parsed);
    if canonical != formatted {
        return Err("the proposed `.zkinit` did not round-trip canonically".into());
    }
    atomic_create(&manifest_path, canonical.as_bytes())?;
    Ok(manifest_path)
}

fn default_manifest(root: &Path) -> ProjectManifest {
    application_manifest(
        project_name(root),
        "0.1.0".into(),
        PathBuf::from("src/main.zrk"),
    )
}

fn prompt_manifest<R: BufRead, W: Write>(
    root: &Path,
    input: &mut R,
    output: &mut W,
) -> Result<ProjectManifest, String> {
    let kind = loop {
        let answer = ask_with_default(
            input,
            output,
            "Project type (application/package)",
            "application",
        )?;
        match answer.as_str() {
            "application" | "app" => break ProjectKind::Application,
            "package" | "library" => break ProjectKind::Package,
            _ => writeln!(output, "Choose `application` or `package`.")
                .map_err(|error| error.to_string())?,
        }
    };

    let name = ask_with_default(input, output, "Project name", &project_name(root))?;
    let version = ask_with_default(input, output, "Version", "0.1.0")?;
    let path_label = match kind {
        ProjectKind::Application => "Entry file",
        ProjectKind::Package => "Package root",
    };
    let default_entry = match kind {
        ProjectKind::Application => "src/main.zrk",
        ProjectKind::Package => "src/package.zrk",
    };
    let entry = PathBuf::from(ask_with_default(
        input,
        output,
        &format!("{path_label} (project-relative)"),
        default_entry,
    )?);
    let authors = ask_optional_list(input, output, "Authors (comma-separated, optional)")?;
    let description = ask_optional(input, output, "Description (optional)")?;
    let license = ask_optional(input, output, "SPDX license (optional)")?;
    let source_roots =
        ask_list_with_default(input, output, "Source roots (comma-separated)", "src")?
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
    let build_targets = if kind == ProjectKind::Application {
        ask_optional_list(input, output, "Build targets (comma-separated, optional)")?
    } else {
        Vec::new()
    };

    Ok(ProjectManifest {
        name,
        version,
        kind,
        authors,
        description,
        license,
        entry: entry.clone(),
        package_root: (kind == ProjectKind::Package).then_some(entry),
        source_roots,
        build_targets,
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        globals: Default::default(),
        requires: Default::default(),
        spans: Default::default(),
        environment_permissions: EnvironmentPermissions::default(),
        network_permissions: NetworkPermissions::default(),
    })
}

fn ask_with_default<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    default: &str,
) -> Result<String, String> {
    let answer = ask(input, output, &format!("{label} [{default}]"))?;
    Ok(if answer.is_empty() {
        default.to_owned()
    } else {
        answer
    })
}

fn ask_optional<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
) -> Result<Option<String>, String> {
    ask(input, output, &format!("{label} [omit]"))
        .map(|answer| (!answer.is_empty()).then_some(answer))
}

fn ask_optional_list<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
) -> Result<Vec<String>, String> {
    let answer = ask(input, output, &format!("{label} [omit]"))?;
    Ok(answer
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect())
}

fn ask_list_with_default<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    default: &str,
) -> Result<Vec<String>, String> {
    let answer = ask(input, output, &format!("{label} [{default}]"))?;
    let selected = if answer.is_empty() { default } else { &answer };
    Ok(selected
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect())
}

fn ask<R: BufRead, W: Write>(input: &mut R, output: &mut W, label: &str) -> Result<String, String> {
    write!(output, "{label}: ").map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    let mut answer = String::new();
    let read = input
        .read_line(&mut answer)
        .map_err(|error| format!("could not read initializer answer: {error}"))?;
    if read == 0 {
        return Err("initialization cancelled before `.zkinit` was written".into());
    }
    Ok(answer.trim().to_owned())
}

fn application_manifest(name: String, version: String, entry: PathBuf) -> ProjectManifest {
    ProjectManifest {
        name,
        version,
        kind: ProjectKind::Application,
        authors: Vec::new(),
        description: None,
        license: None,
        entry,
        package_root: None,
        source_roots: vec![PathBuf::from("src")],
        build_targets: Vec::new(),
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        globals: Default::default(),
        requires: Default::default(),
        spans: Default::default(),
        environment_permissions: EnvironmentPermissions::default(),
        network_permissions: NetworkPermissions::default(),
    }
}

fn project_name(root: &Path) -> String {
    let directory = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut slug = String::new();
    let mut previous_separator = false;
    for character in directory.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_separator = false;
        } else if !slug.is_empty() && !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "zirk-project".into()
    } else {
        slug.into()
    }
}

fn atomic_create(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("manifest path has no parent directory")?;
    let counter = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".zkinit-{}-{counter}.tmp", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o644);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("could not create temporary `.zkinit`: {error}"))?;
        file.write_all(contents)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("could not write temporary `.zkinit`: {error}"))?;
        std::fs::hard_link(&temporary, path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!(
                    "`{}` already exists; it was not overwritten",
                    path.display()
                )
            } else {
                format!("could not atomically create `{}`: {error}", path.display())
            }
        })?;
        std::fs::remove_file(&temporary)
            .map_err(|error| format!("could not clean up temporary `.zkinit`: {error}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{InitMode, initialize, project_name};
    use crate::project::{ProjectKind, parse_manifest};
    use std::io::Cursor;

    fn workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("zirk-init-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn yes_uses_application_defaults_without_creating_source_or_consent() {
        let root = workspace("yes-defaults");
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let path = initialize(&root, InitMode::Yes, &mut input, &mut output).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        let manifest = parse_manifest(&source).unwrap();
        assert_eq!(manifest.name, root.file_name().unwrap().to_string_lossy());
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.kind, ProjectKind::Application);
        assert_eq!(manifest.entry, std::path::PathBuf::from("src/main.zrk"));
        assert_eq!(manifest.source_roots, [std::path::PathBuf::from("src")]);
        assert!(manifest.authors.is_empty());
        assert!(manifest.description.is_none());
        assert!(manifest.license.is_none());
        assert!(manifest.build_targets.is_empty());
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.dev_dependencies.is_empty());
        assert!(manifest.globals.is_empty());
        assert!(manifest.requires.is_empty());
        assert!(!root.join("src").exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interactive_package_prompts_use_package_root_and_optional_metadata() {
        let root = workspace("interactive-package");
        let answers =
            b"package\nmy-lib\n1.2.0\nsrc/lib.zrk\nAda Lovelace\nA library\nMIT\nsrc, tests\n";
        let mut input = Cursor::new(answers);
        let mut output = Vec::new();
        let path = initialize(&root, InitMode::Interactive, &mut input, &mut output).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        let manifest = parse_manifest(&source).unwrap();
        assert_eq!(manifest.kind, ProjectKind::Package);
        assert_eq!(manifest.entry, std::path::PathBuf::from("src/lib.zrk"));
        assert_eq!(manifest.package_root, Some("src/lib.zrk".into()));
        assert_eq!(manifest.authors, ["Ada Lovelace"]);
        assert_eq!(manifest.description.as_deref(), Some("A library"));
        assert_eq!(manifest.license.as_deref(), Some("MIT"));
        assert_eq!(manifest.source_roots.len(), 2);
        assert!(!source.contains("build_targets"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interactive_blank_answers_accept_displayed_defaults() {
        let root = workspace("interactive-defaults");
        let mut input = Cursor::new(b"\n\n\n\n\n\n\n\n\n".to_vec());
        let mut output = Vec::new();
        let path = initialize(&root, InitMode::Interactive, &mut input, &mut output).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        let manifest = parse_manifest(&source).unwrap();
        let prompts = String::from_utf8(output).unwrap();

        assert_eq!(manifest.kind, ProjectKind::Application);
        assert_eq!(manifest.name, project_name(&root));
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, std::path::PathBuf::from("src/main.zrk"));
        assert_eq!(manifest.source_roots, [std::path::PathBuf::from("src")]);
        assert!(manifest.authors.is_empty());
        assert!(manifest.description.is_none());
        assert!(manifest.license.is_none());
        assert!(manifest.build_targets.is_empty());
        assert!(prompts.contains("Project type (application/package) [application]"));
        assert!(prompts.contains("Entry file (project-relative) [src/main.zrk]"));
        assert!(prompts.contains("Build targets (comma-separated, optional) [omit]"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interactive_package_accepts_the_default_public_root() {
        let root = workspace("interactive-package-default-root");
        let answers = b"package\nmy-lib\n1.2.0\n\n\n\n\n\n";
        let mut input = Cursor::new(answers);
        let mut output = Vec::new();
        let path = initialize(&root, InitMode::Interactive, &mut input, &mut output).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        let manifest = parse_manifest(&source).unwrap();

        assert_eq!(manifest.kind, ProjectKind::Package);
        assert_eq!(manifest.package_root, Some("src/package.zrk".into()));
        assert!(!source.contains("build_targets"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn yes_uses_a_fallback_name_when_directory_has_no_ascii_slug() {
        let parent =
            std::env::temp_dir().join(format!("zirk-init-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        let root = parent.join("🪐");
        std::fs::create_dir_all(&root).unwrap();
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let path = initialize(&root, InitMode::Yes, &mut input, &mut output).unwrap();
        let manifest = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(manifest.name, "zirk-project");
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn cancellation_invalid_input_and_existing_manifest_write_nothing() {
        let root = workspace("failure-paths");
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        assert!(initialize(&root, InitMode::Interactive, &mut input, &mut output).is_err());
        assert!(!root.join(".zkinit").exists());

        let answers = b"application\nmy-app\nnot-a-version\n";
        let mut input = Cursor::new(answers);
        assert!(initialize(&root, InitMode::Interactive, &mut input, &mut output).is_err());
        assert!(!root.join(".zkinit").exists());

        std::fs::write(root.join(".zkinit"), "keep me\n").unwrap();
        let original = std::fs::read(root.join(".zkinit")).unwrap();
        assert!(initialize(&root, InitMode::Yes, &mut input, &mut output).is_err());
        assert_eq!(std::fs::read(root.join(".zkinit")).unwrap(), original);
        std::fs::remove_dir_all(root).unwrap();
    }
}
