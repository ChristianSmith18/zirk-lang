//! Explicit, project-bound local permission approvals.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zirk_runtime_core::permission_grant::{PermissionScopes, SignedPermissionGrant};

use crate::project::{self, ProjectError, ProjectKind, ProjectManifest};

pub const RUNTIME_GRANT_ENV: &str = "ZIRK_RUNTIME_PERMISSION_GRANT";

#[derive(Debug)]
pub struct ApprovalError(pub String);

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ApprovalError {}

struct ApprovalContext {
    root: PathBuf,
    project_name: String,
    project_kind: ProjectKind,
    project_id: String,
    manifest_digest: String,
    scopes: PermissionScopes,
    package_requirements: Vec<(String, PermissionScopes)>,
}

fn context(start: &Path) -> Result<ApprovalContext, ApprovalError> {
    let manifest_path = project::discover(start)
        .ok_or_else(|| ApprovalError("could not find a `.zkinit` project manifest".into()))?;
    let manifest_path = manifest_path
        .canonicalize()
        .map_err(|error| ApprovalError(format!("could not resolve `.zkinit`: {error}")))?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| ApprovalError("the project manifest has no parent directory".into()))?
        .to_path_buf();
    let manifest = project::read_manifest(&manifest_path).map_err(project_error)?;
    let manifest_bytes = std::fs::read(&manifest_path)
        .map_err(|error| ApprovalError(format!("could not read `.zkinit`: {error}")))?;
    let project_id = project_id(&root, &manifest.name);
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"zirk-permission-approval-v2\0");
    digest_field(&mut fingerprint, &manifest_bytes);
    let mut requested_scopes = scopes(&manifest);
    let mut package_requirements = Vec::new();
    collect_package_requirements(
        &root,
        &manifest,
        &mut requested_scopes,
        &mut package_requirements,
        &mut fingerprint,
        &mut BTreeSet::new(),
    )?;
    let manifest_digest = format!("{:x}", fingerprint.finalize());
    Ok(ApprovalContext {
        root,
        project_name: manifest.name,
        project_kind: manifest.kind,
        project_id,
        manifest_digest,
        scopes: requested_scopes,
        package_requirements,
    })
}

fn collect_package_requirements(
    root: &Path,
    manifest: &ProjectManifest,
    effective: &mut PermissionScopes,
    package_requirements: &mut Vec<(String, PermissionScopes)>,
    fingerprint: &mut Sha256,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), ApprovalError> {
    if manifest.dependencies.is_empty() {
        return Ok(());
    }

    let lock_path = root.join("zirk.lock");
    let lock_bytes = std::fs::read(&lock_path).map_err(|error| {
        ApprovalError(format!(
            "could not read dependency lockfile `{}`: {error}",
            lock_path.display()
        ))
    })?;
    let lock =
        project::parse_lockfile(std::str::from_utf8(&lock_bytes).map_err(|error| {
            ApprovalError(format!("dependency lockfile is not UTF-8: {error}"))
        })?)
        .map_err(project_error)?;
    project::validate_lockfile(root, manifest, Some(&lock)).map_err(project_error)?;
    digest_path_field(fingerprint, root);
    digest_field(fingerprint, &lock_bytes);

    for dependency in manifest.dependencies.values() {
        let entry = lock.packages.get(&dependency.name).ok_or_else(|| {
            ApprovalError(format!(
                "dependency `{}` is missing from `zirk.lock`",
                dependency.name
            ))
        })?;
        let package_root = project::resolved_dependency_root(root, dependency, entry)
            .ok_or_else(|| {
                ApprovalError(format!("dependency `{}` is unresolved", dependency.name))
            })?
            .canonicalize()
            .map_err(|error| {
                ApprovalError(format!(
                    "could not resolve dependency `{}`: {error}",
                    dependency.name
                ))
            })?;
        if !visited.insert(package_root.clone()) {
            continue;
        }
        let manifest_path = package_root.join(".zkinit");
        let package_bytes = std::fs::read(&manifest_path).map_err(|error| {
            ApprovalError(format!(
                "could not read dependency `{}` manifest: {error}",
                dependency.name
            ))
        })?;
        let package_manifest =
            project::parse_manifest(std::str::from_utf8(&package_bytes).map_err(|error| {
                ApprovalError(format!("dependency manifest is not UTF-8: {error}"))
            })?)
            .map_err(project_error)?;
        if package_manifest.kind != ProjectKind::Package || package_manifest.name != dependency.name
        {
            return Err(ApprovalError(format!(
                "dependency `{}` does not resolve to the expected package",
                dependency.name
            )));
        }

        digest_path_field(fingerprint, &package_root);
        digest_field(fingerprint, &package_bytes);
        let requirements = scopes(&package_manifest);
        merge_scopes(effective, &requirements);
        package_requirements.push((package_manifest.name.clone(), requirements));
        collect_package_requirements(
            &package_root,
            &package_manifest,
            effective,
            package_requirements,
            fingerprint,
            visited,
        )?;
    }
    Ok(())
}

fn digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn digest_path_field(digest: &mut Sha256, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        digest_field(digest, path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let encoded = path
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        digest_field(digest, &encoded);
    }
    #[cfg(not(any(unix, windows)))]
    {
        digest_field(digest, path.to_string_lossy().as_bytes());
    }
}

fn project_id(root: &Path, name: &str) -> String {
    let mut identity = Sha256::new();
    identity.update(b"zirk-permission-project-v3\0");
    digest_path_field(&mut identity, root);
    digest_field(&mut identity, name.as_bytes());
    format!("{:x}", identity.finalize())
}

fn merge_scopes(target: &mut PermissionScopes, source: &PermissionScopes) {
    target
        .environment_read
        .extend(source.environment_read.iter().cloned());
    target
        .environment_secrets
        .extend(source.environment_secrets.iter().cloned());
    target.environment_list_names |= source.environment_list_names;
    target
        .network_connect
        .extend(source.network_connect.iter().cloned());
    target
        .network_listen
        .extend(source.network_listen.iter().cloned());
}

fn scopes(manifest: &ProjectManifest) -> PermissionScopes {
    PermissionScopes {
        environment_read: manifest.environment_permissions.read_names.clone(),
        environment_secrets: manifest.environment_permissions.secret_names.clone(),
        environment_list_names: manifest.environment_permissions.list_names,
        network_connect: manifest.network_permissions.connect_origins.clone(),
        network_listen: manifest.network_permissions.listen_addresses.clone(),
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn project_error(error: ProjectError) -> ApprovalError {
    ApprovalError(error.to_string())
}

/// Displays the exact declared scopes and records consent only after the user
/// types the explicit confirmation word `approve`.
pub fn approve(start: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    if context.project_kind != ProjectKind::Application {
        return Err(ApprovalError(
            "only an application project can receive runtime permissions; packages may only declare requirements".into(),
        ));
    }
    if context.scopes == PermissionScopes::default() {
        return Err(ApprovalError(
            "this project declares no environment or network permissions".into(),
        ));
    }

    println!("Project: {}", context.project_name);
    println!("Location: {}", context.root.display());
    println!("Requested runtime permissions (application plus dependency requirements):");
    print_scopes(&context.scopes);
    for (package, requirements) in &context.package_requirements {
        if requirements != &PermissionScopes::default() {
            println!("  Required by package `{package}`:");
            print_scopes_indented(requirements);
        }
    }
    println!(
        "Project and dependency fingerprint: {}",
        context.manifest_digest
    );
    print!("Type `approve` to grant exactly these permissions: ");
    io::stdout()
        .flush()
        .map_err(|error| ApprovalError(format!("could not write approval prompt: {error}")))?;
    let mut confirmation = String::new();
    io::stdin()
        .read_line(&mut confirmation)
        .map_err(|error| ApprovalError(format!("could not read approval: {error}")))?;
    if confirmation.trim() != "approve" {
        return Err(ApprovalError(
            "permission approval was not confirmed".into(),
        ));
    }

    let (key, key_path) = load_or_create_key(&data_root()?)?;
    let signed = SignedPermissionGrant::sign(
        context.project_id,
        context.manifest_digest,
        context.scopes,
        &key,
    )
    .map_err(|error| ApprovalError(format!("could not sign permission approval: {error:?}")))?;
    let grant_path = grant_path(&key_path, signed.project_id());
    atomic_write(
        &grant_path,
        signed
            .encode()
            .map_err(|error| {
                ApprovalError(format!("could not encode permission approval: {error:?}"))
            })?
            .as_bytes(),
    )?;
    println!("Approval saved for this project and locked dependency graph.");
    Ok(())
}

/// Removes the saved approval for this project without changing its manifest.
pub fn revoke(start: &Path) -> Result<(), ApprovalError> {
    revoke_in(start, &data_root()?)
}

/// Removes a project approval from an explicit security directory.
pub fn revoke_in(start: &Path, security_directory: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    let grant_path = grant_path(
        &security_directory.join("permissions.key"),
        &context.project_id,
    );
    match std::fs::remove_file(&grant_path) {
        Ok(()) => println!("Permission approval revoked for {}.", context.project_name),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!(
                "No saved permission approval exists for {}.",
                context.project_name
            );
        }
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not revoke permission approval: {error}"
            )));
        }
    }
    Ok(())
}

/// Returns the exact signed approval token for a project. Missing approval is
/// deliberately `None`, which leaves all runtime operations fail-closed.
pub fn verified_token(start: &Path) -> Result<Option<String>, ApprovalError> {
    verified_token_in(start, &data_root()?)
}

/// Loads a project token from an explicit security directory. Kept public for
/// the launcher integration tests, which isolate approval state from the
/// developer's actual per-user installation.
pub fn verified_token_in(
    start: &Path,
    security_directory: &Path,
) -> Result<Option<String>, ApprovalError> {
    let context = context(start)?;
    if context.project_kind != ProjectKind::Application
        || context.scopes == PermissionScopes::default()
    {
        return Ok(None);
    }
    let key_path = security_directory.join("permissions.key");
    let key = match std::fs::symlink_metadata(&key_path) {
        Ok(_) => read_secure_key(&key_path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not inspect approval key: {error}"
            )));
        }
    };
    let grant_path = grant_path(&key_path, &context.project_id);
    let encoded = match std::fs::read_to_string(&grant_path) {
        Ok(encoded) => encoded,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not read the saved permission approval: {error}"
            )));
        }
    };
    let signed = SignedPermissionGrant::decode(&encoded).map_err(|error| {
        ApprovalError(format!("saved permission approval is invalid: {error:?}"))
    })?;
    let verified = signed.verify(&key).map_err(|error| {
        ApprovalError(format!(
            "saved permission approval failed verification: {error:?}"
        ))
    })?;
    if verified.project_id() != context.project_id
        || verified.manifest_digest() != context.manifest_digest
        || verified.scopes() != &context.scopes
    {
        return Ok(None);
    }
    Ok(Some(encoded))
}

/// Creates the child runtime's authenticated grant token for `zirk run`.
/// Projects with missing/stale approval receive no token and stay denied.
pub fn runtime_token_for_path(path: &Path) -> Result<Option<String>, ApprovalError> {
    let project_path = if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
        path
    } else {
        return Ok(None);
    };
    verified_token(project_path)
}

pub fn print_scopes(scopes: &PermissionScopes) {
    for name in &scopes.environment_read {
        println!("  environment.read: {name}");
    }
    for name in &scopes.environment_secrets {
        println!("  environment.secrets: {name}");
    }
    if scopes.environment_list_names {
        println!("  environment.list_names: all visible names");
    }
    for origin in &scopes.network_connect {
        println!("  network.connect: {origin}");
    }
    for address in &scopes.network_listen {
        println!("  network.listen: {address}");
    }
}

fn print_scopes_indented(scopes: &PermissionScopes) {
    for name in &scopes.environment_read {
        println!("    environment.read: {name}");
    }
    for name in &scopes.environment_secrets {
        println!("    environment.secrets: {name}");
    }
    if scopes.environment_list_names {
        println!("    environment.list_names: all visible names");
    }
    for origin in &scopes.network_connect {
        println!("    network.connect: {origin}");
    }
    for address in &scopes.network_listen {
        println!("    network.listen: {address}");
    }
}

fn data_root() -> Result<PathBuf, ApprovalError> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| ApprovalError("LOCALAPPDATA is not set".into()))?
        .join("Zirk");
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| ApprovalError("HOME is not set".into()))?
        .join(".local/share/zirk");
    Ok(root.join("security"))
}

fn load_or_create_key(directory: &Path) -> Result<(Vec<u8>, PathBuf), ApprovalError> {
    std::fs::create_dir_all(directory)
        .map_err(|error| ApprovalError(format!("could not create permission storage: {error}")))?;
    secure_directory(directory)?;
    let path = directory.join("permissions.key");
    if path.exists() {
        return Ok((read_secure_key(&path)?, path));
    }
    let mut key = vec![0_u8; 32];
    getrandom::getrandom(&mut key)
        .map_err(|error| ApprovalError(format!("could not generate an approval key: {error}")))?;
    create_private_file(&path, &key)?;
    Ok((key, path))
}

fn read_secure_key(path: &Path) -> Result<Vec<u8>, ApprovalError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ApprovalError(format!("could not inspect approval key: {error}")))?;
    if !metadata.file_type().is_file() {
        return Err(ApprovalError("approval key is not a regular file".into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(ApprovalError(
                "approval key permissions are not private to the current user".into(),
            ));
        }
    }
    let key = std::fs::read(path)
        .map_err(|error| ApprovalError(format!("could not read approval key: {error}")))?;
    if key.len() != 32 {
        return Err(ApprovalError("approval key has an invalid length".into()));
    }
    Ok(key)
}

fn secure_directory(_path: &Path) -> Result<(), ApprovalError> {
    let metadata = std::fs::symlink_metadata(_path)
        .map_err(|error| ApprovalError(format!("could not inspect permission storage: {error}")))?;
    if !metadata.file_type().is_dir() {
        return Err(ApprovalError(
            "permission storage must be a real directory, not a symbolic link".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(ApprovalError(
                "permission storage is not owned by the current user".into(),
            ));
        }
        std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| ApprovalError(format!("could not secure permission storage: {error}")),
        )?;
    }
    Ok(())
}

fn create_private_file(path: &Path, contents: &[u8]) -> Result<(), ApprovalError> {
    use std::fs::OpenOptions;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| ApprovalError(format!("could not create approval key: {error}")))?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|error| ApprovalError(format!("could not save approval key: {error}")))
}

fn grant_path(key_path: &Path, project_id: &str) -> PathBuf {
    key_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("approvals")
        .join(format!("{project_id}.json"))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), ApprovalError> {
    let parent = path
        .parent()
        .ok_or_else(|| ApprovalError("approval file has no parent directory".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| ApprovalError(format!("could not create approval directory: {error}")))?;
    secure_directory(parent)?;
    let mut random = [0_u8; 8];
    getrandom::getrandom(&mut random).map_err(|error| {
        ApprovalError(format!("could not generate a temporary filename: {error}"))
    })?;
    let temporary = parent.join(format!(".approval-{}.tmp", digest(&random)));
    create_private_file(&temporary, contents)?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).map_err(|error| {
            ApprovalError(format!("could not replace permission approval: {error}"))
        })?;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(ApprovalError(format!(
            "could not install permission approval: {error}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_permission_scopes_are_exact() {
        let manifest = project::parse_manifest(
            "project \"app\"\nkind application\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\nsecrets: { names: [\"DB_PASSWORD\"]; during: runtime; }\nlist_names: { during: runtime; }\n}\nnetwork {\nconnect: { origins: [\"https://api.example:443\"]; during: runtime; }\nlisten: { addresses: [\"127.0.0.1:8080\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();
        let scopes = scopes(&manifest);
        assert_eq!(scopes.environment_read, ["APP_MODE".to_owned()].into());
        assert_eq!(
            scopes.environment_secrets,
            ["DB_PASSWORD".to_owned()].into()
        );
        assert!(scopes.environment_list_names);
        assert_eq!(
            scopes.network_connect,
            ["https://api.example:443".to_owned()].into()
        );
        assert_eq!(scopes.network_listen, ["127.0.0.1:8080".to_owned()].into());
    }

    #[test]
    fn runtime_token_is_only_loaded_for_a_project_entry_path() {
        assert!(
            runtime_token_for_path(Path::new("src/main.zrk"))
                .unwrap()
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_identity_distinguishes_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let base = std::env::temp_dir().join(format!(
            "zirk-permission-non-utf8-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = base.join(std::ffi::OsString::from_vec(b"project-\xff".to_vec()));
        let second = base.join(std::ffi::OsString::from_vec(b"project-\xfe".to_vec()));
        // Some supported filesystems reject invalid UTF-8 names, so exercise
        // the canonical identity encoder directly without creating the paths.
        assert_ne!(project_id(&first, "same"), project_id(&second, "same"));
    }

    #[test]
    fn missing_approval_key_means_no_token_not_launcher_failure() {
        let base = std::env::temp_dir().join(format!(
            "zirk-permission-no-key-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = base.join("project");
        let security = base.join("security");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&security).unwrap();
        std::fs::write(
            project.join(".zkinit"),
            "project \"app\"\nkind application\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();

        assert_eq!(verified_token_in(&project, &security).unwrap(), None);

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn dependency_permissions_are_transitive_and_invalidate_saved_approval() {
        let base = std::env::temp_dir().join(format!(
            "zirk-permission-dependency-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = base.join("app");
        let direct = app.join("vendor/direct");
        let nested = direct.join("vendor/nested");
        let security = base.join("security");
        std::fs::create_dir_all(app.join("src")).unwrap();
        std::fs::create_dir_all(direct.join("src")).unwrap();
        std::fs::create_dir_all(nested.join("src")).unwrap();
        std::fs::write(app.join("src/main.zrk"), "").unwrap();
        std::fs::write(direct.join("src/package.zrk"), "").unwrap();
        std::fs::write(nested.join("src/package.zrk"), "").unwrap();
        std::fs::write(
            app.join(".zkinit"),
            "project \"app\"\nkind application\ndependency acme.direct ^1 local \"vendor/direct\"\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();
        std::fs::write(
            direct.join(".zkinit"),
            "project \"acme.direct\"\nkind package\ndependency acme.nested ^1 local \"vendor/nested\"\npermissions {\nenvironment {\nsecrets: { names: [\"DIRECT_SECRET\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();
        std::fs::write(
            nested.join(".zkinit"),
            "project \"acme.nested\"\nkind package\npermissions {\nnetwork {\nconnect: { origins: [\"https://api.example:443\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();

        fn write_lock(root: &Path, dep_root: &Path, name: &str) {
            let package_manifest = project::read_manifest(&dep_root.join(".zkinit")).unwrap();
            let hash = project::package_integrity_hash(dep_root, &package_manifest).unwrap();
            std::fs::write(
                root.join("zirk.lock"),
                format!("{name} 1.0 local sha256:{hash} src/package.zrk\n"),
            )
            .unwrap();
        }
        write_lock(&app, &direct, "acme.direct");
        write_lock(&direct, &nested, "acme.nested");

        let approval = context(&app).unwrap();
        assert_eq!(
            approval.scopes.environment_read,
            ["APP_MODE".to_owned()].into()
        );
        assert_eq!(
            approval.scopes.environment_secrets,
            ["DIRECT_SECRET".to_owned()].into()
        );
        assert_eq!(
            approval.scopes.network_connect,
            ["https://api.example:443".to_owned()].into()
        );
        assert_eq!(approval.package_requirements.len(), 2);

        let (key, key_path) = load_or_create_key(&security).unwrap();
        let signed = SignedPermissionGrant::sign(
            approval.project_id.clone(),
            approval.manifest_digest.clone(),
            approval.scopes.clone(),
            &key,
        )
        .unwrap();
        let saved = grant_path(&key_path, signed.project_id());
        atomic_write(&saved, signed.encode().unwrap().as_bytes()).unwrap();
        assert!(verified_token_in(&app, &security).unwrap().is_some());

        let mut changed_nested_manifest = std::fs::read_to_string(nested.join(".zkinit")).unwrap();
        changed_nested_manifest = changed_nested_manifest.replace("api.example", "other.example");
        std::fs::write(nested.join(".zkinit"), changed_nested_manifest).unwrap();
        assert!(verified_token_in(&app, &security).unwrap().is_none());

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn packages_cannot_approve_runtime_scopes() {
        let directory =
            std::env::temp_dir().join(format!("zirk-permission-package-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(".zkinit"),
            "project \"dependency\"\nkind package\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\n}\n}\n",
        )
        .unwrap();
        let error = approve(&directory).unwrap_err();
        assert!(error.to_string().contains("packages may only declare"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn permission_storage_refuses_symbolic_link_directories() {
        let base =
            std::env::temp_dir().join(format!("zirk-permission-link-{}", std::process::id()));
        let target = base.join("target");
        let link = base.join("link");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(secure_directory(&link).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
