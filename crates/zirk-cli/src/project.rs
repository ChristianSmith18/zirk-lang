//! Project metadata for package-aware frontend loading.
//!
//! `.zkinit` remains a small Zirk-specific declarative surface. This parser is
//! intentionally not the source parser: executable declarations and imports
//! are rejected before module loading begins.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zirk_ast::Program;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Application,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySource {
    Local(PathBuf),
    Registry(String),
    Git(String),
    Vendored(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub constraint: String,
    pub source: DependencySource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifest {
    pub name: String,
    pub kind: ProjectKind,
    pub entry: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub hash: String,
    pub package_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub packages: BTreeMap<String, LockedPackage>,
}

/// Public, deterministic summary used by package inspection and later `.zpkg`
/// writers. It intentionally excludes implementation bodies and private names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageApi {
    pub package: String,
    pub declarations: Vec<String>,
}

pub fn package_api(package: impl Into<String>, program: &Program) -> PackageApi {
    let mut declarations = Vec::new();
    declarations.extend(
        program
            .functions
            .iter()
            .filter(|decl| decl.shared)
            .map(|decl| format!("fn {}", decl.name.name)),
    );
    declarations.extend(
        program
            .classes
            .iter()
            .filter(|decl| decl.shared)
            .map(|decl| format!("class {}", decl.name.name)),
    );
    declarations.extend(
        program
            .enums
            .iter()
            .filter(|decl| decl.shared)
            .map(|decl| format!("enum {}", decl.name.name)),
    );
    declarations.extend(
        program
            .contracts
            .iter()
            .filter(|decl| decl.shared)
            .map(|decl| format!("contract {}", decl.name.name)),
    );
    declarations.extend(
        program
            .type_aliases
            .iter()
            .filter(|decl| decl.shared)
            .map(|decl| format!("type {}", decl.name.name)),
    );
    declarations.extend(
        program
            .module_reexports
            .iter()
            .map(|module| format!("mod {}", module.name.name)),
    );
    declarations.sort();
    PackageApi {
        package: package.into(),
        declarations,
    }
}

/// Minimal inspectable package envelope. Creation/publishing remains disabled
/// until reproducible archives and integrity verification are implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageArtifactPlan {
    pub manifest: ProjectManifest,
    pub api: PackageApi,
    pub publishable: bool,
}

pub fn artifact_plan(manifest: ProjectManifest, api: PackageApi) -> PackageArtifactPlan {
    PackageArtifactPlan {
        manifest,
        api,
        publishable: false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectError(pub String);

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProjectError {}

fn quoted(value: &str) -> Result<String, ProjectError> {
    value
        .trim()
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| ProjectError("expected a quoted value".into()))
}

/// Parses the deliberately line-oriented manifest subset used by the package
/// loader. Every logical field is explicit, which makes failures deterministic
/// and keeps package transport out of source imports.
pub fn parse_manifest(text: &str) -> Result<ProjectManifest, ProjectError> {
    let mut name = None;
    let mut kind = None;
    let mut entry = None;
    let mut source_roots = Vec::new();
    let mut dependencies = BTreeMap::new();
    for raw in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
    {
        let mut pieces = raw.split_whitespace();
        match pieces.next() {
            Some("project") => {
                name =
                    Some(quoted(pieces.next().ok_or_else(|| {
                        ProjectError("project needs a name".into())
                    })?)?)
            }
            Some("kind") => {
                kind = Some(match pieces.next() {
                    Some("application") => ProjectKind::Application,
                    Some("package") => ProjectKind::Package,
                    _ => {
                        return Err(ProjectError(
                            "kind must be `application` or `package`".into(),
                        ));
                    }
                })
            }
            Some("entry") => {
                entry =
                    Some(PathBuf::from(quoted(pieces.next().ok_or_else(|| {
                        ProjectError("entry needs a path".into())
                    })?)?))
            }
            Some("source") => source_roots.push(PathBuf::from(quoted(
                pieces
                    .next()
                    .ok_or_else(|| ProjectError("source needs a path".into()))?,
            )?)),
            Some("dependency") => {
                let package = pieces
                    .next()
                    .ok_or_else(|| ProjectError("dependency needs a package name".into()))?
                    .to_owned();
                let constraint = pieces
                    .next()
                    .ok_or_else(|| ProjectError("dependency needs a version constraint".into()))?
                    .to_owned();
                let source = match pieces.next() {
                    Some("local") => {
                        DependencySource::Local(PathBuf::from(quoted(pieces.next().ok_or_else(
                            || ProjectError("local dependency needs a path".into()),
                        )?)?))
                    }
                    Some("registry") => {
                        DependencySource::Registry(quoted(pieces.next().ok_or_else(|| {
                            ProjectError("registry dependency needs a URL".into())
                        })?)?)
                    }
                    Some("git") => DependencySource::Git(quoted(
                        pieces
                            .next()
                            .ok_or_else(|| ProjectError("git dependency needs a URL".into()))?,
                    )?),
                    Some("vendored") => DependencySource::Vendored(PathBuf::from(quoted(
                        pieces.next().ok_or_else(|| {
                            ProjectError("vendored dependency needs a path".into())
                        })?,
                    )?)),
                    _ => {
                        return Err(ProjectError(
                            "dependency source must be local, registry, git, or vendored".into(),
                        ));
                    }
                };
                dependencies.insert(
                    package.clone(),
                    Dependency {
                        name: package,
                        constraint,
                        source,
                    },
                );
            }
            Some(other) => {
                return Err(ProjectError(format!(
                    "unknown manifest declaration `{other}`"
                )));
            }
            None => {}
        }
    }
    let name = name.ok_or_else(|| ProjectError("manifest is missing `project`".into()))?;
    let kind = kind.ok_or_else(|| ProjectError("manifest is missing `kind`".into()))?;
    let entry = entry.unwrap_or_else(|| {
        PathBuf::from(if kind == ProjectKind::Application {
            "src/main.zrk"
        } else {
            "src/package.zrk"
        })
    });
    if source_roots.is_empty() {
        source_roots.push(PathBuf::from("src"));
    }
    Ok(ProjectManifest {
        name,
        kind,
        entry,
        source_roots,
        dependencies,
    })
}

pub fn parse_lockfile(text: &str) -> Result<Lockfile, ProjectError> {
    let mut packages = BTreeMap::new();
    for raw in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<_> = raw.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(ProjectError(
                "lock entry must contain name, version, source, hash, and package root".into(),
            ));
        }
        if fields[3].is_empty() || fields[3] == "-" {
            return Err(ProjectError(format!(
                "lock entry `{}` has no integrity hash",
                fields[0]
            )));
        }
        let package = LockedPackage {
            name: fields[0].into(),
            version: fields[1].into(),
            source: fields[2].into(),
            hash: fields[3].into(),
            package_root: fields[4].into(),
        };
        packages.insert(package.name.clone(), package);
    }
    Ok(Lockfile { packages })
}

pub fn discover(start: &Path) -> Option<PathBuf> {
    let mut cursor = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };
    loop {
        let manifest = cursor.join(".zkinit");
        if manifest.is_file() {
            return Some(manifest);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

pub fn read_manifest(path: &Path) -> Result<ProjectManifest, ProjectError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| ProjectError(format!("could not read `{}`: {error}", path.display())))?;
    parse_manifest(&text)
}

pub fn read_lockfile(path: &Path) -> Result<Lockfile, ProjectError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| ProjectError(format!("could not read `{}`: {error}", path.display())))?;
    parse_lockfile(&text)
}

/// Resolves the local and vendored portion of a package graph before source
/// loading. Registry/Git entries deliberately remain transport-neutral here.
pub fn resolve_local_roots(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    fn visit(
        root: &Path,
        manifest: &ProjectManifest,
        roots: &mut BTreeMap<String, PathBuf>,
        stack: &mut Vec<String>,
    ) -> Result<(), ProjectError> {
        if let Some(position) = stack.iter().position(|name| name == &manifest.name) {
            let mut cycle = stack[position..].to_vec();
            cycle.push(manifest.name.clone());
            return Err(ProjectError(format!(
                "package dependency cycle: {}",
                cycle.join(" -> ")
            )));
        }
        if roots.contains_key(&manifest.name) {
            return Ok(());
        }
        stack.push(manifest.name.clone());
        roots.insert(manifest.name.clone(), root.join(&manifest.entry));
        for dependency in manifest.dependencies.values() {
            let (DependencySource::Local(path) | DependencySource::Vendored(path)) =
                &dependency.source
            else {
                continue;
            };
            let dependency_root = root.join(path);
            let dependency_manifest = read_manifest(&dependency_root.join(".zkinit"))?;
            visit(&dependency_root, &dependency_manifest, roots, stack)?;
        }
        stack.pop();
        Ok(())
    }
    let mut roots = BTreeMap::new();
    visit(root, manifest, &mut roots, &mut Vec::new())?;
    Ok(roots)
}

/// Resolves every materialized dependency transport to its public entrypoint.
/// Registry and Git acquisition remain separate from graph loading: once a
/// verified artifact is cached, it is loaded exactly like a local package.
pub fn resolve_package_roots(
    root: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    let mut roots = resolve_local_roots(root, manifest)?;
    for dependency in manifest.dependencies.values() {
        let entry = lock.packages.get(&dependency.name).ok_or_else(|| {
            ProjectError(format!(
                "dependency `{}` is missing from `zirk.lock`",
                dependency.name
            ))
        })?;
        let package_root = resolved_dependency_root(root, dependency, entry)
            .expect("every dependency transport has a materialized root");
        roots.insert(
            dependency.name.clone(),
            package_root.join(&entry.package_root),
        );
    }
    Ok(roots)
}

/// Validates the immutable facts that can be checked before source parsing.
///
/// A lockfile is optional for a project without dependencies. Once present,
/// however, every declared dependency needs one matching record.  Transport
/// fetching is intentionally outside the frontend; this prevents a stale
/// lockfile from being silently ignored while keeping imports transport-free.
/// Returns the materialized package root for a dependency. Local and vendored
/// sources are relative to the consuming project; registry and Git sources are
/// resolved from the deterministic project cache populated by acquisition.
pub fn resolved_dependency_root(
    root: &Path,
    dependency: &Dependency,
    lock: &LockedPackage,
) -> Option<PathBuf> {
    match dependency.source {
        DependencySource::Local(ref path) | DependencySource::Vendored(ref path) => {
            Some(root.join(path))
        }
        DependencySource::Registry(_) | DependencySource::Git(_) => Some(
            root.join(".zirk")
                .join("packages")
                .join(&dependency.name)
                .join(&lock.version),
        ),
    }
}

pub fn validate_lockfile(
    root: &Path,
    manifest: &ProjectManifest,
    lock: Option<&Lockfile>,
) -> Result<(), ProjectError> {
    if manifest.dependencies.is_empty() {
        return Ok(());
    }
    let lock =
        lock.ok_or_else(|| ProjectError("project dependencies require `zirk.lock`".into()))?;
    for dependency in manifest.dependencies.values() {
        let entry = lock.packages.get(&dependency.name).ok_or_else(|| {
            ProjectError(format!(
                "dependency `{}` is missing from `zirk.lock`",
                dependency.name
            ))
        })?;
        if entry.version.is_empty() || entry.source.is_empty() || entry.hash.is_empty() {
            return Err(ProjectError(format!(
                "lock entry `{}` is incomplete",
                dependency.name
            )));
        }
        let expected_source = match &dependency.source {
            DependencySource::Local(_) => "local",
            DependencySource::Registry(_) => "registry",
            DependencySource::Git(_) => "git",
            DependencySource::Vendored(_) => "vendored",
        };
        if entry.source != expected_source {
            return Err(ProjectError(format!(
                "lock entry `{}` uses `{}` but the manifest declares `{expected_source}`",
                dependency.name, entry.source
            )));
        }
        if entry.package_root.is_empty() || Path::new(&entry.package_root).is_absolute() {
            return Err(ProjectError(format!(
                "lock entry `{}` has an invalid package root",
                dependency.name
            )));
        }

        if let Some(package_root) = resolved_dependency_root(root, dependency, entry) {
            let package_manifest = read_manifest(&package_root.join(".zkinit"))?;
            if package_manifest.name != dependency.name {
                return Err(ProjectError(format!(
                    "dependency `{}` resolves to package `{}`",
                    dependency.name, package_manifest.name
                )));
            }
            let expected_entry = package_manifest.entry.display().to_string();
            if entry.package_root != expected_entry {
                return Err(ProjectError(format!(
                    "lock entry `{}` points at `{}` but the package entry is `{expected_entry}`",
                    dependency.name, entry.package_root
                )));
            }
            let expected_hash = entry.hash.strip_prefix("sha256:").ok_or_else(|| {
                ProjectError(format!(
                    "lock entry `{}` must use a `sha256:` integrity hash",
                    dependency.name
                ))
            })?;
            if expected_hash.len() != 64
                || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(ProjectError(format!(
                    "lock entry `{}` has an invalid SHA-256 hash",
                    dependency.name
                )));
            }
            let actual_hash = package_integrity_hash(&package_root, &package_manifest)?;
            if expected_hash != actual_hash {
                return Err(ProjectError(format!(
                    "package `{}` does not match the integrity hash in `zirk.lock`",
                    dependency.name
                )));
            }
        }
    }
    Ok(())
}

/// Returns the deterministic SHA-256 of a package's configured source roots.
/// Paths and bytes are length-delimited so different source trees cannot
/// collide by concatenation. Manifest and lock metadata are checked separately.
pub fn package_integrity_hash(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<String, ProjectError> {
    fn collect(
        directory: &Path,
        root: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), ProjectError> {
        let entries = std::fs::read_dir(directory).map_err(|error| {
            ProjectError(format!(
                "could not read package source `{}`: {error}",
                directory.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| ProjectError(error.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                collect(&path, root, files)?;
            } else if path.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                if relative
                    .extension()
                    .is_some_and(|extension| extension == "zrk")
                {
                    files.push(path);
                }
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    for source_root in &manifest.source_roots {
        collect(&root.join(source_root), root, &mut files)?;
    }
    files.sort();
    let mut hasher = Sha256::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        let bytes = std::fs::read(&path).map_err(|error| {
            ProjectError(format!("could not read `{}`: {error}", path.display()))
        })?;
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_models_package_identity_roots_and_dependencies() {
        let manifest = parse_manifest("project \"acme.cli\"\nkind package\nsource \"src\"\ndependency acme.web ^1.0 local \"../web\"").unwrap();
        assert_eq!(manifest.kind, ProjectKind::Package);
        assert_eq!(manifest.entry, PathBuf::from("src/package.zrk"));
        assert!(manifest.dependencies.contains_key("acme.web"));
    }
    #[test]
    fn malformed_lock_integrity_is_rejected_deterministically() {
        assert!(parse_lockfile("acme.web 1.0 local - src/package.zrk").is_err());
        assert!(parse_lockfile("acme.web 1.0 local sha256:ok src/package.zrk").is_ok());
    }

    #[test]
    fn lockfile_must_cover_each_dependency_with_the_declared_transport() {
        let manifest = parse_manifest(
            "project \"app\"\nkind application\ndependency acme.web ^1 local \"dep\"",
        )
        .unwrap();
        let lock = parse_lockfile("acme.web 1.0 git sha256:ok src/package.zrk").unwrap();
        let error = validate_lockfile(Path::new("."), &manifest, Some(&lock)).unwrap_err();
        assert!(error.to_string().contains("manifest declares `local`"));
    }

    #[test]
    fn cached_registry_package_has_the_same_logical_root_as_a_local_package() {
        let root = std::env::temp_dir().join(format!("zirk-registry-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cached = root.join(".zirk/packages/acme.web/1.0");
        std::fs::create_dir_all(cached.join("src")).unwrap();
        std::fs::write(
            root.join(".zkinit"),
            "project \"app\"\nkind application\ndependency acme.web ^1 registry \"https://registry.example\"",
        )
        .unwrap();
        std::fs::write(cached.join(".zkinit"), "project \"acme.web\"\nkind package").unwrap();
        std::fs::write(cached.join("src/package.zrk"), "package acme.web;").unwrap();
        let dependency_manifest = read_manifest(&cached.join(".zkinit")).unwrap();
        let hash = package_integrity_hash(&cached, &dependency_manifest).unwrap();
        let manifest = read_manifest(&root.join(".zkinit")).unwrap();
        let lock = parse_lockfile(&format!(
            "acme.web 1.0 registry sha256:{hash} src/package.zrk"
        ))
        .unwrap();
        validate_lockfile(&root, &manifest, Some(&lock)).unwrap();
        let roots = resolve_package_roots(&root, &manifest, &lock).unwrap();
        assert_eq!(roots["acme.web"], cached.join("src/package.zrk"));
    }

    #[test]
    fn local_package_cycles_report_the_complete_path() {
        let root = std::env::temp_dir().join(format!("zirk-package-cycle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(
            root.join(".zkinit"),
            "project \"a\"\nkind package\ndependency b ^1 local \"b\"",
        )
        .unwrap();
        std::fs::write(
            root.join("b/.zkinit"),
            "project \"b\"\nkind package\ndependency a ^1 local \"..\"",
        )
        .unwrap();
        let error =
            resolve_local_roots(&root, &read_manifest(&root.join(".zkinit")).unwrap()).unwrap_err();
        assert!(error.to_string().contains("a -> b -> a"));
    }

    #[test]
    fn public_api_omits_private_declarations() {
        let source = zirk_diagnostics::SourceFile::new(
            "api.zrk",
            "share fn visible(): Void { }\nfn private_helper(): Void { }",
        );
        let mut sink = zirk_diagnostics::DiagnosticSink::new();
        let tokens = zirk_lexer::tokenize(&source, &mut sink);
        let program = zirk_parser::parse(&source, &tokens, &mut sink);
        let api = package_api("acme.api", &program);
        assert_eq!(api.declarations, vec!["fn visible"]);
    }

    #[test]
    fn package_artifact_plans_are_inspectable_but_not_publishable() {
        let manifest = parse_manifest("project \"acme.api\"\nkind package").unwrap();
        let plan = artifact_plan(
            manifest,
            PackageApi {
                package: "acme.api".into(),
                declarations: vec!["fn visible".into()],
            },
        );
        assert!(!plan.publishable);
        assert_eq!(plan.api.declarations, vec!["fn visible"]);
    }
}
