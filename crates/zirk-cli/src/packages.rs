//! Dependency acquisition and lockfile updates.
//!
//! Package directories contain source and manifests only. Installation reads
//! metadata and files; it never invokes Zirk package code.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::project::{
    self, Dependency, DependencySource, LockedPackage, Lockfile, ProjectError, ProjectKind,
    ProjectManifest,
};

#[derive(Default)]
struct Resolver {
    project_root: PathBuf,
    old_lock: BTreeMap<String, LockedPackage>,
    new_lock: BTreeMap<String, LockedPackage>,
    visited: BTreeSet<String>,
    registries: BTreeMap<String, String>,
    update: bool,
}

/// Resolves runtime and root development dependencies, then atomically writes
/// `zirk.lock`. Dependencies declared by packages are runtime-only; their
/// `dev_dependencies` are never inherited by consumers.
pub fn install_current_project(update: bool) -> Result<Lockfile, ProjectError> {
    let manifest_path = project::discover(Path::new("."))
        .ok_or_else(|| ProjectError("could not find a `.zkinit` project manifest".into()))?;
    let root = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    install_project(root, update)
}

pub fn install_project(root: &Path, update: bool) -> Result<Lockfile, ProjectError> {
    let root = root.canonicalize().map_err(|error| {
        ProjectError(format!(
            "could not resolve project root `{}`: {error}",
            root.display()
        ))
    })?;
    let manifest_path = root.join(".zkinit");
    let manifest = project::read_manifest(&manifest_path)?;
    let lock_path = root.join("zirk.lock");
    let old_lock = if lock_path.is_file() {
        project::read_lockfile(&lock_path)?.packages
    } else {
        BTreeMap::new()
    };
    let mut resolver = Resolver {
        project_root: root.clone(),
        old_lock,
        new_lock: BTreeMap::new(),
        visited: BTreeSet::new(),
        registries: read_registry_config()?,
        update,
    };

    resolver.resolve_dependencies(&root, &manifest, true)?;
    let lock = Lockfile {
        packages: resolver.new_lock,
    };
    let encoded = project::format_lockfile(&lock);
    if !lock_path.exists() || fs::read_to_string(&lock_path).ok().as_deref() != Some(&encoded) {
        atomic_write(&lock_path, encoded.as_bytes())?;
    }
    Ok(lock)
}

impl Resolver {
    fn resolve_dependencies(
        &mut self,
        importer_root: &Path,
        manifest: &ProjectManifest,
        include_root_dev_dependencies: bool,
    ) -> Result<(), ProjectError> {
        for dependency in manifest.dependencies.values() {
            self.resolve_one(importer_root, dependency)?;
        }
        if include_root_dev_dependencies {
            for dependency in manifest.dev_dependencies.values() {
                self.resolve_one(importer_root, dependency)?;
            }
        }
        Ok(())
    }

    fn resolve_one(
        &mut self,
        importer_root: &Path,
        dependency: &Dependency,
    ) -> Result<(), ProjectError> {
        let locator = project::dependency_locator(&dependency.source);
        if let Some(existing) = self.new_lock.get(&dependency.name) {
            if existing.locator != locator
                || !project::lock_version_satisfies(&dependency.constraint, &existing.version)?
            {
                return Err(ProjectError(format!(
                    "dependency `{}` is required with incompatible sources or versions in the locked graph",
                    dependency.name
                )));
            }
            return Ok(());
        }

        let (package_root, revision) = self.materialize(importer_root, dependency)?;
        let package_manifest = project::read_manifest(&package_root.join(".zkinit"))?;
        if package_manifest.kind != ProjectKind::Package {
            return Err(ProjectError(format!(
                "dependency `{}` resolves to an application; dependency manifests must use `type: package`",
                dependency.name
            )));
        }
        if package_manifest.name != dependency.name {
            return Err(ProjectError(format!(
                "dependency `{}` resolves to package `{}`",
                dependency.name, package_manifest.name
            )));
        }
        if !project::lock_version_satisfies(&dependency.constraint, &package_manifest.version)? {
            return Err(ProjectError(format!(
                "package `{}` version `{}` does not satisfy `{}`",
                dependency.name, package_manifest.version, dependency.constraint
            )));
        }
        let integrity = project::package_integrity_hash(&package_root, &package_manifest)?;
        if let Some(expected) = &dependency.integrity
            && expected != &format!("sha256:{integrity}")
        {
            return Err(ProjectError(format!(
                "dependency `{}` does not match its declared integrity value",
                dependency.name
            )));
        }
        let package = LockedPackage {
            name: dependency.name.clone(),
            version: package_manifest.version.clone(),
            source: source_kind(&dependency.source).into(),
            locator,
            revision,
            hash: format!("sha256:{integrity}"),
            package_root: package_manifest.entry.to_string_lossy().into_owned(),
        };
        self.new_lock
            .insert(dependency.name.clone(), package.clone());

        if self.visited.insert(dependency.name.clone()) {
            self.resolve_dependencies(&package_root, &package_manifest, false)?;
        }
        Ok(())
    }

    fn materialize(
        &mut self,
        importer_root: &Path,
        dependency: &Dependency,
    ) -> Result<(PathBuf, Option<String>), ProjectError> {
        match &dependency.source {
            DependencySource::Local(path) | DependencySource::Vendored(path) => {
                let root = resolve_local_path(importer_root, path, &dependency.name)?;
                Ok((root, None))
            }
            DependencySource::Git(repository) => self.materialize_git(dependency, repository),
            DependencySource::Registry(registry) => self.materialize_registry(dependency, registry),
        }
    }

    fn materialize_git(
        &mut self,
        dependency: &Dependency,
        repository: &str,
    ) -> Result<(PathBuf, Option<String>), ProjectError> {
        if !self.update
            && let Some(old) = self.old_lock.get(&dependency.name)
            && old.locator == project::dependency_locator(&dependency.source)
            && project::lock_version_satisfies(&dependency.constraint, &old.version)?
            && let Some(revision) = old.revision.as_deref()
        {
            let cached =
                package_cache_path(&self.project_root, &dependency.name, &old.version, revision);
            if verify_cached_package(&cached, dependency, old).is_ok() {
                return Ok((cached, Some(revision.into())));
            }
            return self.clone_git_revision(dependency, repository, revision);
        }

        let revision = dependency.revision.as_deref().ok_or_else(|| {
            ProjectError(format!(
                "Git dependency `{}` needs an explicit `rev` before `zirk install`",
                dependency.name
            ))
        })?;
        self.clone_git_revision(dependency, repository, revision)
    }

    fn clone_git_revision(
        &self,
        dependency: &Dependency,
        repository: &str,
        requested_revision: &str,
    ) -> Result<(PathBuf, Option<String>), ProjectError> {
        let root = self.project_root.clone();
        let cache_parent = root.join(".zirk/git").join(stable_hash(repository));
        fs::create_dir_all(&cache_parent).map_err(|error| {
            ProjectError(format!("could not create Git cache directory: {error}"))
        })?;
        let staged_repo = cache_parent.join(format!("clone-{}", std::process::id()));
        if staged_repo.exists() {
            fs::remove_dir_all(&staged_repo).map_err(|error| {
                ProjectError(format!("could not clear temporary Git checkout: {error}"))
            })?;
        }
        git_command()
            .args(["clone", "--quiet", "--no-checkout", "--", repository])
            .arg(&staged_repo)
            .stdout(Stdio::null())
            .output()
            .map_err(|error| ProjectError(format!("could not start Git: {error}")))
            .and_then(|output| git_result(output, "clone dependency repository"))?;
        git_command()
            .arg("-C")
            .arg(&staged_repo)
            .args(["checkout", "--quiet", "--detach", requested_revision])
            .stdout(Stdio::null())
            .output()
            .map_err(|error| ProjectError(format!("could not start Git: {error}")))
            .and_then(|output| git_result(output, "checkout requested dependency revision"))?;
        let revision = git_command()
            .arg("-C")
            .arg(&staged_repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|error| ProjectError(format!("could not start Git: {error}")))
            .and_then(|output| git_stdout(output, "resolve immutable dependency revision"))?;
        let git_manifest = project::read_manifest(&staged_repo.join(".zkinit"))?;
        if git_manifest.kind != ProjectKind::Package || git_manifest.name != dependency.name {
            return Err(ProjectError(format!(
                "Git repository for `{}` must contain a package manifest with the same logical name",
                dependency.name
            )));
        }
        if !project::lock_version_satisfies(&dependency.constraint, &git_manifest.version)? {
            return Err(ProjectError(format!(
                "Git package `{}` version `{}` does not satisfy `{}`",
                dependency.name, git_manifest.version, dependency.constraint
            )));
        }
        let destination =
            package_cache_path(&root, &dependency.name, &git_manifest.version, &revision);
        let package_root = install_cached_tree(&staged_repo, &destination, &dependency.name)?;
        let _ = fs::remove_dir_all(&staged_repo);
        Ok((package_root, Some(revision)))
    }

    fn materialize_registry(
        &mut self,
        dependency: &Dependency,
        registry: &str,
    ) -> Result<(PathBuf, Option<String>), ProjectError> {
        if !self.update
            && let Some(old) = self.old_lock.get(&dependency.name)
            && old.locator == project::dependency_locator(&dependency.source)
            && project::lock_version_satisfies(&dependency.constraint, &old.version)?
        {
            let identity = old.hash.strip_prefix("sha256:").ok_or_else(|| {
                ProjectError(format!(
                    "registry lock entry `{}` lacks its content identity",
                    dependency.name
                ))
            })?;
            let cached =
                package_cache_path(&self.project_root, &dependency.name, &old.version, identity);
            if verify_cached_package(&cached, dependency, old).is_ok() {
                return Ok((cached, old.revision.clone()));
            }
        }

        let registry_root = self.registry_root(registry)?;
        let package_versions = registry_root.join(&dependency.name);
        let entries = fs::read_dir(&package_versions).map_err(|error| {
            ProjectError(format!(
                "registry `{registry}` has no package `{}`: {error}",
                dependency.name
            ))
        })?;
        let mut candidates = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| ProjectError(error.to_string()))?;
            if !entry
                .file_type()
                .map_err(|error| ProjectError(error.to_string()))?
                .is_dir()
            {
                continue;
            }
            let version = entry.file_name().to_string_lossy().into_owned();
            if project::lock_version_satisfies(&dependency.constraint, &version)? {
                candidates.push((
                    semver::Version::parse(&version).map_err(|error| {
                        ProjectError(format!(
                            "registry has invalid package version `{version}`: {error}"
                        ))
                    })?,
                    entry.path(),
                ));
            }
        }
        candidates.sort_by(|left, right| right.0.cmp(&left.0));
        let (version, source) = candidates.into_iter().next().ok_or_else(|| {
            ProjectError(format!(
                "registry `{registry}` has no version of `{}` satisfying `{}`",
                dependency.name, dependency.constraint
            ))
        })?;
        let package_manifest = project::read_manifest(&source.join(".zkinit"))?;
        if package_manifest.version != version.to_string() {
            return Err(ProjectError(format!(
                "registry package `{}` directory version `{version}` does not match its manifest `{}`",
                dependency.name, package_manifest.version
            )));
        }
        let identity = project::package_integrity_hash(&source, &package_manifest)?;
        let destination = package_cache_path(
            &self.project_root,
            &dependency.name,
            &version.to_string(),
            &identity,
        );
        let package_root = install_cached_tree(&source, &destination, &dependency.name)?;
        Ok((package_root, None))
    }

    fn registry_root(&mut self, registry: &str) -> Result<PathBuf, ProjectError> {
        let configured = self.registries.get(registry).ok_or_else(|| {
            ProjectError(format!(
                "registry `{registry}` is not configured; add it to `~/.config/zirk/registries.conf`"
            ))
        })?;
        let configured_path = configured.strip_prefix("file://").unwrap_or(configured);
        let local_path = PathBuf::from(configured_path);
        if local_path.is_dir() {
            return local_path.canonicalize().map_err(|error| {
                ProjectError(format!("could not resolve registry `{registry}`: {error}"))
            });
        }
        let cache = self
            .project_root
            .join(".zirk/registries")
            .join(registry)
            .join(stable_hash(configured));
        if self.update && cache.exists() {
            let suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let refreshed = cache.with_file_name(format!("refresh-{suffix}"));
            let _ = fs::remove_dir_all(&refreshed);
            clone_repository(configured, &refreshed, "refresh registry")?;
            let old = cache.with_file_name(format!("old-{suffix}"));
            fs::rename(&cache, &old).map_err(|error| {
                ProjectError(format!("could not rotate registry cache: {error}"))
            })?;
            fs::rename(&refreshed, &cache).map_err(|error| {
                ProjectError(format!(
                    "could not install refreshed registry cache: {error}"
                ))
            })?;
        } else if !cache.exists() {
            if let Some(parent) = cache.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    ProjectError(format!("could not create registry cache: {error}"))
                })?;
            }
            clone_repository(configured, &cache, "clone registry")?;
        }
        cache.canonicalize().map_err(|error| {
            ProjectError(format!(
                "could not resolve registry cache `{}`: {error}",
                cache.display()
            ))
        })
    }
}

fn resolve_local_path(base: &Path, relative: &Path, name: &str) -> Result<PathBuf, ProjectError> {
    project::canonical_child_without_symlinks(base, relative, &format!("local dependency `{name}`"))
}

fn source_kind(source: &DependencySource) -> &'static str {
    match source {
        DependencySource::Local(_) => "local",
        DependencySource::Registry(_) => "registry",
        DependencySource::Git(_) => "git",
        DependencySource::Vendored(_) => "vendored",
    }
}

fn package_cache_path(root: &Path, name: &str, version: &str, identity: &str) -> PathBuf {
    root.join(".zirk/packages")
        .join(name)
        .join(version)
        .join(identity)
}

fn verify_cached_package(
    root: &Path,
    dependency: &Dependency,
    lock: &LockedPackage,
) -> Result<(), ProjectError> {
    let manifest = project::read_manifest(&root.join(".zkinit"))?;
    if manifest.name != dependency.name || manifest.version != lock.version {
        return Err(ProjectError(format!(
            "cached package `{}` does not match the lock identity",
            dependency.name
        )));
    }
    let hash = project::package_integrity_hash(root, &manifest)?;
    if lock.hash != format!("sha256:{hash}") {
        return Err(ProjectError(format!(
            "cached package `{}` does not match the locked integrity value",
            dependency.name
        )));
    }
    Ok(())
}

fn install_cached_tree(
    source: &Path,
    destination: &Path,
    package: &str,
) -> Result<PathBuf, ProjectError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ProjectError(format!("could not create package cache: {error}")))?;
    }
    let stage = destination.with_file_name(format!("stage-{}-{}", package, std::process::id()));
    if stage.exists() {
        return Err(ProjectError(format!(
            "staging path `{}` already exists; remove that incomplete cache entry before installing",
            stage.display()
        )));
    }
    copy_tree_without_symlinks(source, &stage)?;
    if destination.exists() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let backup = destination.with_file_name(format!(
            "backup-{}-{}-{suffix}",
            package,
            std::process::id()
        ));
        fs::rename(destination, &backup).map_err(|error| {
            ProjectError(format!("could not preserve stale package cache: {error}"))
        })?;
    }
    fs::rename(&stage, destination)
        .map_err(|error| ProjectError(format!("could not commit package cache: {error}")))?;
    destination.canonicalize().map_err(|error| {
        ProjectError(format!(
            "could not resolve installed package cache: {error}"
        ))
    })
}

fn copy_tree_without_symlinks(source: &Path, destination: &Path) -> Result<(), ProjectError> {
    fs::create_dir_all(destination).map_err(|error| {
        ProjectError(format!(
            "could not create package staging directory: {error}"
        ))
    })?;
    for entry in fs::read_dir(source)
        .map_err(|error| ProjectError(format!("could not read package source: {error}")))?
    {
        let entry = entry.map_err(|error| ProjectError(error.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| ProjectError(error.to_string()))?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let from = entry.path();
        let to = destination.join(name);
        if file_type.is_symlink() {
            return Err(ProjectError(format!(
                "package source contains symbolic link `{}`",
                from.display()
            )));
        }
        if file_type.is_dir() {
            copy_tree_without_symlinks(&from, &to)?;
        } else if file_type.is_file() {
            fs::copy(&from, &to).map_err(|error| {
                ProjectError(format!(
                    "could not copy package file `{}`: {error}",
                    from.display()
                ))
            })?;
        }
    }
    Ok(())
}

fn read_registry_config() -> Result<BTreeMap<String, String>, ProjectError> {
    let Some(config_root) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    else {
        return Ok(BTreeMap::new());
    };
    let path = config_root.join("zirk/registries.conf");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(ProjectError(format!(
                "could not read registry configuration `{}`: {error}",
                path.display()
            )));
        }
    };
    let mut registries = BTreeMap::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or_else(|| {
            ProjectError(format!(
                "invalid registry configuration at {}:{}; expected `name = \"url\"`",
                path.display(),
                line_number + 1
            ))
        })?;
        let name = name.trim();
        let value = value.trim().trim_matches('"');
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || value.is_empty()
            || registries
                .insert(name.to_owned(), value.to_owned())
                .is_some()
        {
            return Err(ProjectError(format!(
                "invalid or duplicate registry entry `{name}` at {}:{}",
                path.display(),
                line_number + 1
            )));
        }
    }
    Ok(registries)
}

fn clone_repository(url: &str, destination: &Path, action: &str) -> Result<(), ProjectError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ProjectError(format!("could not create Git cache: {error}")))?;
    }
    let output = git_command()
        .args(["clone", "--quiet", "--", url])
        .arg(destination)
        .stdout(Stdio::null())
        .output()
        .map_err(|error| ProjectError(format!("could not start Git: {error}")))?;
    git_result(output, action)
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    #[cfg(unix)]
    command.env("GIT_CONFIG_GLOBAL", "/dev/null");
    command
}

fn git_result(output: std::process::Output, action: &str) -> Result<(), ProjectError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(ProjectError(format!(
            "Git could not {action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn git_stdout(output: std::process::Output, action: &str) -> Result<String, ProjectError> {
    if !output.status.success() {
        return Err(ProjectError(format!(
            "Git could not {action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| ProjectError(format!("Git returned an invalid revision: {error}")))
}

fn stable_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), ProjectError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| ProjectError(format!("could not create lockfile directory: {error}")))?;
    let temp = path.with_file_name(format!("zirk.lock.tmp.{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| ProjectError(format!("could not create temporary lockfile: {error}")))?;
    use std::io::Write;
    file.write_all(contents)
        .and_then(|_| file.sync_all())
        .map_err(|error| ProjectError(format!("could not write temporary lockfile: {error}")))?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        ProjectError(format!("could not atomically update `zirk.lock`: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(root: &Path, name: &str, version: &str) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join(".zkinit"),
            format!("project {{ name: \"{name}\"; version: \"{version}\"; type: package; }}\n"),
        )
        .unwrap();
        fs::write(root.join("src/package.zrk"), "share class Api { }\n").unwrap();
    }

    fn scratch(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zirk-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn install_locks_path_and_root_dev_dependencies_without_rewriting_unchanged_lock() {
        let root = scratch("install-path");
        package(&root.join("vendor/lib"), "acme.lib", "1.2.0");
        package(&root.join("dev/testkit"), "acme.testkit", "0.8.0");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.lib: { from: \"path:vendor/lib\"; version: \"^1.0\"; } }\ndev_dependencies { acme.testkit: { from: \"path:dev/testkit\"; version: \"^0.8\"; } }\n",
        )
        .unwrap();

        let lock = install_project(&root, false).unwrap();
        assert_eq!(lock.packages.len(), 2);
        assert_eq!(lock.packages["acme.lib"].locator, "path:vendor/lib");
        assert_eq!(lock.packages["acme.testkit"].version, "0.8.0");
        let bytes = fs::read(root.join("zirk.lock")).unwrap();
        let manifest = project::read_manifest(&root.join(".zkinit")).unwrap();
        project::validate_lockfile(&root, &manifest, Some(&lock)).unwrap();
        install_project(&root, false).unwrap();
        assert_eq!(fs::read(root.join("zirk.lock")).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn development_graph_includes_only_root_dev_dependencies() {
        let root = scratch("dev-graph");
        package(&root.join("vendor/runtime"), "acme.runtime", "1.0.0");
        package(&root.join("vendor/hidden"), "acme.hidden", "1.0.0");
        package(&root.join("dev/testkit"), "acme.testkit", "1.0.0");
        fs::write(
            root.join("vendor/runtime/.zkinit"),
            "project { name: \"acme.runtime\"; version: \"1.0.0\"; type: package; }\ndev_dependencies { acme.hidden: { from: \"path:../hidden\"; version: \"^1.0\"; } }\n",
        )
        .unwrap();
        fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.runtime: { from: \"path:vendor/runtime\"; version: \"^1.0\"; } }\ndev_dependencies { acme.testkit: { from: \"path:dev/testkit\"; version: \"^1.0\"; } }\n",
        )
        .unwrap();

        let lock = install_project(&root, false).unwrap();
        assert!(lock.packages.contains_key("acme.runtime"));
        assert!(lock.packages.contains_key("acme.testkit"));
        assert!(!lock.packages.contains_key("acme.hidden"));

        let manifest = project::read_manifest(&root.join(".zkinit")).unwrap();
        let runtime_roots =
            project::resolve_package_roots_with_dev(&root, &manifest, &lock, false).unwrap();
        let test_roots =
            project::resolve_package_roots_with_dev(&root, &manifest, &lock, true).unwrap();
        assert!(runtime_roots.contains_key("acme.runtime"));
        assert!(!runtime_roots.contains_key("acme.testkit"));
        assert!(test_roots.contains_key("acme.testkit"));
        assert!(!test_roots.contains_key("acme.hidden"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_dependency_name_mismatch_fails_before_lock_creation() {
        let root = scratch("path-name-mismatch");
        package(&root.join("vendor/wrong"), "acme.wrong", "1.0.0");
        fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.expected: { from: \"path:vendor/wrong\"; version: \"^1.0\"; } }\n",
        )
        .unwrap();
        let error = install_project(&root, false).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("resolves to package `acme.wrong`")
        );
        assert!(!root.join("zirk.lock").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vendored_package_uses_the_same_logical_name_resolution() {
        let root = scratch("vendored-package");
        package(&root.join("vendor/acme.lib"), "acme.lib", "1.3.0");
        fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.lib: { from: \"vendored:vendor/acme.lib\"; version: \"^1.0\"; } }\n",
        )
        .unwrap();

        let lock = install_project(&root, false).unwrap();
        assert_eq!(lock.packages["acme.lib"].source, "vendored");
        assert_eq!(
            lock.packages["acme.lib"].locator,
            "vendored:vendor/acme.lib"
        );
        let manifest = project::read_manifest(&root.join(".zkinit")).unwrap();
        let roots = project::resolve_package_roots(&root, &manifest, &lock).unwrap();
        assert_eq!(
            roots["acme.lib"],
            root.join("vendor/acme.lib/src/package.zrk")
                .canonicalize()
                .unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_dependency_rejects_symbolic_link_before_lock_creation() {
        use std::os::unix::fs::symlink;

        let root = scratch("path-symlink");
        package(&root.join("outside/lib"), "acme.lib", "1.0.0");
        fs::create_dir_all(root.join("vendor")).unwrap();
        symlink(root.join("outside/lib"), root.join("vendor/link")).unwrap();
        fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.lib: { from: \"path:vendor/link\"; version: \"^1.0\"; } }\n",
        )
        .unwrap();
        let error = install_project(&root, false).unwrap_err();
        assert!(error.to_string().contains("symbolic link"));
        assert!(!root.join("zirk.lock").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_resolution_selects_highest_matching_package_version() {
        let root = scratch("registry-select");
        let registry = root.join("registry");
        package(&registry.join("acme.http/1.0.0"), "acme.http", "1.0.0");
        package(&registry.join("acme.http/1.4.0"), "acme.http", "1.4.0");
        package(&registry.join("acme.http/2.0.0"), "acme.http", "2.0.0");
        let mut resolver = Resolver {
            project_root: root.join("app"),
            registries: [("default".into(), registry.to_string_lossy().into_owned())].into(),
            ..Resolver::default()
        };
        fs::create_dir_all(&resolver.project_root).unwrap();
        let dependency = Dependency {
            name: "acme.http".into(),
            constraint: "^1.0".into(),
            source: DependencySource::Registry("default".into()),
            revision: None,
            integrity: None,
        };
        resolver
            .resolve_one(&resolver.project_root.clone(), &dependency)
            .unwrap();
        assert_eq!(resolver.new_lock["acme.http"].version, "1.4.0");
        let identity = resolver.new_lock["acme.http"]
            .hash
            .strip_prefix("sha256:")
            .unwrap();
        assert!(
            resolver
                .project_root
                .join(".zirk/packages/acme.http/1.4.0")
                .join(identity)
                .join("src/package.zrk")
                .is_file()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_and_named_registries_keep_the_declared_logical_package_names() {
        let root = scratch("named-registry");
        let default_registry = root.join("default-registry");
        let company_registry = root.join("company-registry");
        package(
            &default_registry.join("acme.public/1.2.0"),
            "acme.public",
            "1.2.0",
        );
        package(
            &company_registry.join("acme.private/1.4.0"),
            "acme.private",
            "1.4.0",
        );
        let project_root = root.join("app");
        fs::create_dir_all(&project_root).unwrap();
        let manifest = project::parse_manifest(
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.public: \"^1.0\"; acme.private: { from: \"registry:company\"; version: \"^1.0\"; } }",
        )
        .unwrap();
        let mut resolver = Resolver {
            project_root: project_root.clone(),
            registries: [
                (
                    "default".into(),
                    default_registry.to_string_lossy().into_owned(),
                ),
                (
                    "company".into(),
                    company_registry.to_string_lossy().into_owned(),
                ),
            ]
            .into(),
            ..Resolver::default()
        };
        resolver
            .resolve_dependencies(&project_root, &manifest, true)
            .unwrap();
        assert_eq!(resolver.new_lock["acme.public"].locator, "registry:default");
        assert_eq!(
            resolver.new_lock["acme.private"].locator,
            "registry:company"
        );
        assert_eq!(resolver.new_lock["acme.public"].name, "acme.public");
        assert_eq!(resolver.new_lock["acme.private"].name, "acme.private");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn git_dependency_lock_records_the_resolved_commit_and_source_hash() {
        let root = scratch("git-install");
        let repository = root.join("upstream");
        package(&repository, "acme.git", "1.0.0");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let init = Command::new("git")
            .arg("init")
            .arg(&repository)
            .output()
            .unwrap();
        assert!(init.status.success());
        git(&["config", "user.email", "zirk-test@example.invalid"]);
        git(&["config", "user.name", "Zirk Test"]);
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "package"]);
        git(&["tag", "v1.0.0"]);

        let consumer = root.join("consumer");
        fs::create_dir_all(&consumer).unwrap();
        let repository_url = format!("file://{}", repository.display());
        let dependency = Dependency {
            name: "acme.git".into(),
            constraint: String::new(),
            source: DependencySource::Git(repository_url),
            revision: Some("v1.0.0".into()),
            integrity: None,
        };
        let mut resolver = Resolver {
            project_root: consumer.clone(),
            ..Resolver::default()
        };
        resolver.resolve_one(&consumer, &dependency).unwrap();
        let locked = &resolver.new_lock["acme.git"];
        assert_eq!(locked.revision.as_deref().unwrap().len(), 40);
        assert!(locked.hash.starts_with("sha256:"));
        assert!(
            consumer
                .join(".zirk/packages/acme.git/1.0.0")
                .join(locked.revision.as_deref().unwrap())
                .join(".zkinit")
                .is_file()
        );
        assert!(
            !consumer
                .join(".zirk/packages/acme.git/1.0.0")
                .join(locked.revision.as_deref().unwrap())
                .join(".git")
                .exists()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn moving_git_tag_does_not_change_lock_until_update() {
        let root = scratch("git-tag-move");
        let repository = root.join("upstream");
        package(&repository, "acme.moving", "1.0.0");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let init = Command::new("git")
            .arg("init")
            .arg(&repository)
            .output()
            .unwrap();
        assert!(init.status.success());
        git(&["config", "user.email", "zirk-test@example.invalid"]);
        git(&["config", "user.name", "Zirk Test"]);
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "first package revision"]);
        git(&["tag", "v1.0.0"]);

        let consumer = root.join("consumer");
        fs::create_dir_all(&consumer).unwrap();
        let repository_url = format!("file://{}", repository.display());
        fs::write(
            consumer.join(".zkinit"),
            format!(
                "project {{ name: \"app\"; version: \"0.1.0\"; type: application; }}\ndependencies {{ acme.moving: {{ from: \"git+{repository_url}\"; rev: \"v1.0.0\"; }} }}\n"
            ),
        )
        .unwrap();
        let first = install_project(&consumer, false).unwrap();
        let original_revision = first.packages["acme.moving"].revision.clone().unwrap();

        fs::write(
            repository.join("src/package.zrk"),
            "share class Api { fn changed(): Void { } }\n",
        )
        .unwrap();
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "move package tag"]);
        git(&["tag", "--force", "v1.0.0"]);

        let locked = install_project(&consumer, false).unwrap();
        assert_eq!(
            locked.packages["acme.moving"].revision,
            Some(original_revision.clone())
        );
        let old_cache = consumer
            .join(".zirk/packages/acme.moving/1.0.0")
            .join(&original_revision);
        assert!(old_cache.join(".zkinit").is_file());

        let refreshed = install_project(&consumer, true).unwrap();
        let refreshed_revision = refreshed.packages["acme.moving"].revision.as_ref().unwrap();
        assert_ne!(refreshed_revision, &original_revision);
        assert!(old_cache.join(".zkinit").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
