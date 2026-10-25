//! Explicit, project-bound local permission approvals.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zirk_runtime_core::permission_grant::{
    FilesystemScopeExtent, PermissionHistoryAction, PermissionScopes, SignedPermissionGrant,
    SignedPermissionHistoryEvent, VerifiedPermissionHistoryEvent,
};

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
    direct_requires: BTreeSet<crate::project::Capability>,
    effective_requires: BTreeSet<crate::project::Capability>,
    requirement_tree: Vec<RequirementNode>,
}

#[derive(Debug, Clone)]
struct RequirementNode {
    name: String,
    version: String,
    integrity: String,
    path: Vec<String>,
    requires: BTreeSet<crate::project::Capability>,
}

enum ConsentState {
    Missing,
    Stale,
    Current(PermissionScopes),
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
    let project_id = project_id(&root, &manifest.name);
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"zirk-permission-approval-v2\0");
    // Direct `requires` declarations describe possible operations, while
    // saved grants contain concrete resources. Keep requirements out of the
    // app identity digest so a first-use approval can add its capability to
    // `.zkinit` without invalidating the exact resource grant it just saved.
    // All other project fields and the full locked dependency graph remain
    // part of the fingerprint.
    let mut identity_manifest = manifest.clone();
    identity_manifest.requires.clear();
    digest_field(
        &mut fingerprint,
        project::format_manifest(&identity_manifest).as_bytes(),
    );
    let mut effective_requires = manifest.requires.clone();
    let mut requirement_tree = Vec::new();
    collect_package_requirements(
        &root,
        &manifest,
        &mut effective_requires,
        &mut requirement_tree,
        &mut vec![manifest.name.clone()],
        &mut fingerprint,
    )?;
    let manifest_digest = format!("{:x}", fingerprint.finalize());
    Ok(ApprovalContext {
        root,
        project_name: manifest.name,
        project_kind: manifest.kind,
        project_id,
        manifest_digest,
        direct_requires: manifest.requires,
        effective_requires,
        requirement_tree,
    })
}

fn collect_package_requirements(
    root: &Path,
    manifest: &ProjectManifest,
    effective_requires: &mut BTreeSet<crate::project::Capability>,
    requirement_tree: &mut Vec<RequirementNode>,
    path: &mut Vec<String>,
    fingerprint: &mut Sha256,
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
        effective_requires.extend(package_manifest.requires.iter().copied());
        let mut dependency_path = path.clone();
        dependency_path.push(package_manifest.name.clone());
        requirement_tree.push(RequirementNode {
            name: package_manifest.name.clone(),
            version: entry.version.clone(),
            integrity: entry.hash.clone(),
            path: dependency_path.clone(),
            requires: package_manifest.requires.clone(),
        });
        if path.contains(&package_manifest.name) {
            continue;
        }
        path.push(package_manifest.name.clone());
        collect_package_requirements(
            &package_root,
            &package_manifest,
            effective_requires,
            requirement_tree,
            path,
            fingerprint,
        )?;
        path.pop();
    }
    Ok(())
}

/// Prints the manifest's finite capability requirements with each locked
/// requester and dependency path. This is an inventory only; it grants no
/// authority and never edits the manifest or approval store.
pub fn inspect(start: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    let consent = load_consent_state(&context, &data_root()?)?;
    println!("Project: {}", context.project_name);
    println!("Location: {}", context.root.display());
    println!("Direct requires:");
    if context.direct_requires.is_empty() {
        println!("  (none)");
    }
    for capability in &context.direct_requires {
        println!("  {}", capability.as_str());
    }
    println!("Dependency requirements:");
    if context.requirement_tree.is_empty() {
        println!("  (none)");
    }
    for node in &context.requirement_tree {
        println!(
            "  {} {} [{}] via {}",
            node.name,
            node.version,
            &node.integrity,
            node.path.join(" -> ")
        );
        if node.requires.is_empty() {
            println!("    (none)");
        }
        for capability in &node.requires {
            println!("    {}", capability.as_str());
        }
    }
    println!("Effective requires:");
    if context.effective_requires.is_empty() {
        println!("  (none)");
    }
    for capability in &context.effective_requires {
        println!("  {}", capability.as_str());
    }
    println!("Saved exact consent:");
    match consent {
        ConsentState::Missing => println!("  (none)"),
        ConsentState::Stale => println!("  (stale or unverifiable; grants no authority)"),
        ConsentState::Current(scopes) => {
            if scope_entries(&scopes).is_empty() {
                println!("  (none)");
            }
            print_scopes(&scopes);
        }
    }
    println!("`requires` declares possible operations; saved consent contains exact resources.");
    Ok(())
}

/// Compares the locked capability tree with currently trusted exact-resource
/// consent. Requirements never grant authority by themselves.
pub fn diff(start: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    let consent = load_consent_state(&context, &data_root()?)?;
    let scopes = match &consent {
        ConsentState::Current(scopes) => scopes.clone(),
        ConsentState::Missing | ConsentState::Stale => PermissionScopes::default(),
    };
    println!("Project: {}", context.project_name);
    println!("Requirement and consent diff:");
    match consent {
        ConsentState::Missing => println!("  Approval record: missing"),
        ConsentState::Stale => {
            println!("  Approval record: stale or unverifiable; grants no authority")
        }
        ConsentState::Current(_) => println!("  Approval record: current"),
    }
    if context.effective_requires.is_empty() {
        println!("  No capabilities are declared by the application or its dependencies.");
    }
    for capability in &context.effective_requires {
        let resources = scope_entries(&scopes)
            .into_iter()
            .filter(|(name, _)| name == capability.as_str())
            .map(|(_, resource)| resource)
            .collect::<Vec<_>>();
        let requesters = requesters_for(&context, *capability);
        println!("  {}", capability.as_str());
        if requesters.is_empty() {
            println!("    Declared by: (requester not present in the locked tree)");
        } else {
            for requester in requesters {
                println!("    Declared by: {requester}");
            }
        }
        if resources.is_empty() {
            println!(
                "    Consent: no exact resource saved; first use will ask interactively or deny without a TTY"
            );
        } else {
            for resource in resources {
                println!("    Saved resource: {resource}");
            }
        }
    }
    let declared = context
        .effective_requires
        .iter()
        .map(|capability| capability.as_str())
        .collect::<BTreeSet<_>>();
    for (capability, resource) in scope_entries(&scopes) {
        if !declared.contains(capability.as_str()) {
            println!("  Saved but undeclared: {capability} {resource}");
        }
    }
    Ok(())
}

/// Prints authenticated, user-owned approval and revocation history without
/// storing or displaying secret values.
pub fn history(start: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    let events = read_history(&context, &data_root()?)?;
    println!("Project: {}", context.project_name);
    if events.is_empty() {
        println!("Permission history: (none)");
        return Ok(());
    }
    println!("Permission history (timestamps are UTC Unix milliseconds):");
    for event in events {
        let action = match event.action() {
            PermissionHistoryAction::Approved => "approved",
            PermissionHistoryAction::Revoked => "revoked",
        };
        println!(
            "  {} {} {} at {} by {} (requested by {}; manifest {})",
            action,
            event.capability(),
            event.resource(),
            event.timestamp_unix_ms(),
            event.actor(),
            event.requester(),
            &event.manifest_digest()[..12]
        );
    }
    Ok(())
}

fn load_consent_state(
    context: &ApprovalContext,
    security_directory: &Path,
) -> Result<ConsentState, ApprovalError> {
    let key_path = security_directory.join("permissions.key");
    let key = match std::fs::symlink_metadata(&key_path) {
        Ok(_) => read_secure_key(&key_path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ConsentState::Missing),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not inspect approval key: {error}"
            )));
        }
    };
    let path = grant_path(&key_path, &context.project_id);
    let encoded = match std::fs::read_to_string(&path) {
        Ok(encoded) => encoded,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ConsentState::Missing),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not read approval record: {error}"
            )));
        }
    };
    let Ok(signed) = SignedPermissionGrant::decode(&encoded) else {
        return Ok(ConsentState::Stale);
    };
    let Ok(verified) = signed.verify(&key) else {
        return Ok(ConsentState::Stale);
    };
    if verified.project_id() != context.project_id
        || verified.manifest_digest() != context.manifest_digest
    {
        return Ok(ConsentState::Stale);
    }
    Ok(ConsentState::Current(verified.scopes().clone()))
}

fn requesters_for(
    context: &ApprovalContext,
    capability: crate::project::Capability,
) -> Vec<String> {
    let mut requesters = Vec::new();
    if context.direct_requires.contains(&capability) {
        requesters.push(format!("{} (application)", context.project_name));
    }
    for node in &context.requirement_tree {
        if node.requires.contains(&capability) {
            requesters.push(format!(
                "{} {} [{}] via {}",
                node.name,
                node.version,
                node.integrity,
                node.path.join(" -> ")
            ));
        }
    }
    requesters
}

fn scope_entries(scopes: &PermissionScopes) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut add = |capability: &str, values: &BTreeSet<String>| {
        entries.extend(
            values
                .iter()
                .cloned()
                .map(|resource| (capability.to_owned(), resource)),
        );
    };
    add("filesystem.read", &scopes.filesystem_read);
    add("filesystem.write", &scopes.filesystem_write);
    add("filesystem.metadata", &scopes.filesystem_metadata);
    add("environment.read", &scopes.environment_read);
    add("secrets.read", &scopes.environment_secrets);
    add("network.connect", &scopes.network_connect);
    add("network.listen", &scopes.network_listen);
    add("network.udp.send", &scopes.network_udp_destinations);
    add("network.udp.listen", &scopes.network_udp_listen);
    add("network.local.connect", &scopes.network_local_connect);
    add("network.local.listen", &scopes.network_local_listen);
    add("process.execute", &scopes.process_execute);
    add("process.shell", &scopes.process_shell);
    drop(add);
    entries.extend(scopes.filesystem_operations.iter().map(|scope| {
        let extent = match scope.extent {
            FilesystemScopeExtent::Exact => "exact object",
            FilesystemScopeExtent::Directory => "directory object only",
            FilesystemScopeExtent::Descendants => "directory and descendants",
        };
        (
            scope.capability.clone(),
            format!(
                "{} [operation={}, extent={extent}, requester={}]",
                scope.resource, scope.operation, scope.requester
            ),
        )
    }));
    entries.extend(scopes.network_udp_multicast_groups.iter().map(|group| {
        (
            "network.udp.send".to_owned(),
            format!("udp-multicast://{group}"),
        )
    }));
    if scopes.environment_list_names {
        entries.push((
            "environment.list_names".into(),
            "all environment names".into(),
        ));
    }
    if scopes.network_inspect_interfaces {
        entries.push((
            "network.inspect_interfaces".into(),
            "network interfaces".into(),
        ));
    }
    entries.sort();
    entries
}

fn read_history(
    context: &ApprovalContext,
    security_directory: &Path,
) -> Result<Vec<VerifiedPermissionHistoryEvent>, ApprovalError> {
    let key_path = security_directory.join("permissions.key");
    let key = match std::fs::symlink_metadata(&key_path) {
        Ok(_) => read_secure_key(&key_path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not inspect approval key: {error}"
            )));
        }
    };
    let path = history_path(security_directory, &context.project_id);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(ApprovalError(format!(
                "could not inspect permission history: {error}"
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(ApprovalError(
            "permission history must be a regular file".into(),
        ));
    }
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| ApprovalError(format!("could not read permission history: {error}")))?;
    let mut events = Vec::new();
    for (line_number, line) in contents.lines().enumerate() {
        let signed = SignedPermissionHistoryEvent::decode(line).map_err(|error| {
            ApprovalError(format!(
                "permission history line {} is invalid: {error:?}",
                line_number + 1
            ))
        })?;
        let event = signed.verify(&key).map_err(|error| {
            ApprovalError(format!(
                "permission history line {} failed verification: {error:?}",
                line_number + 1
            ))
        })?;
        if event.project_id() == context.project_id {
            events.push(event);
        }
    }
    events.sort_by_key(VerifiedPermissionHistoryEvent::timestamp_unix_ms);
    Ok(events)
}

fn append_history_event(
    security_directory: &Path,
    context: &ApprovalContext,
    action: PermissionHistoryAction,
    capability: &str,
    resource: &str,
    requester: &str,
    actor: &str,
) -> Result<(), ApprovalError> {
    use std::fs::OpenOptions;

    let key = read_secure_key(&security_directory.join("permissions.key"))?;
    let timestamp_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| ApprovalError(format!("system clock is invalid: {error}")))?
        .as_millis();
    let event = SignedPermissionHistoryEvent::sign(
        context.project_id.clone(),
        context.manifest_digest.clone(),
        timestamp_unix_ms,
        action,
        capability,
        resource,
        requester,
        actor,
        &key,
    )
    .map_err(|error| ApprovalError(format!("could not sign permission history: {error:?}")))?;
    let encoded = format!(
        "{}\n",
        event
            .encode()
            .map_err(|error| ApprovalError(format!("could not encode history event: {error:?}")))?
    );
    let directory = history_directory(security_directory);
    ensure_private_directory(&directory)?;
    let path = history_path(security_directory, &context.project_id);
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| ApprovalError(format!("could not write permission history: {error}")))?;
    file.write_all(encoded.as_bytes())
        .and_then(|()| file.sync_data())
        .map_err(|error| ApprovalError(format!("could not save permission history: {error}")))
}

fn history_directory(security_directory: &Path) -> PathBuf {
    security_directory.join("approval-history")
}

fn history_path(security_directory: &Path, project_id: &str) -> PathBuf {
    history_directory(security_directory).join(format!("{project_id}.jsonl"))
}

fn ensure_private_directory(path: &Path) -> Result<(), ApprovalError> {
    std::fs::create_dir_all(path)
        .map_err(|error| ApprovalError(format!("could not create permission history: {error}")))?;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ApprovalError(format!("could not inspect permission history: {error}")))?;
    if !metadata.file_type().is_dir() {
        return Err(ApprovalError(
            "permission history path must be a real directory".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(ApprovalError(
                "permission history is not owned by the current user".into(),
            ));
        }
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| ApprovalError(format!("could not secure permission history: {error}")),
        )?;
    }
    Ok(())
}

/// Returns trusted launcher metadata for the generated runtime. Environment
/// values are installed by the CLI after removing inherited Zirk values so
/// the child cannot accidentally borrow another project's identity.
pub fn runtime_environment_for_path(
    path: &Path,
) -> Result<Option<BTreeMap<String, String>>, ApprovalError> {
    let Some(project_root) = project_root_for_execution_path(path) else {
        return standalone_runtime_environment(path);
    };
    let context = context(&project_root)?;
    let manifest_template = project::format_manifest(
        &project::read_manifest(&context.root.join(".zkinit")).map_err(project_error)?,
    );
    let mut package_paths = BTreeMap::<String, Vec<String>>::new();
    for node in &context.requirement_tree {
        package_paths
            .entry(node.name.clone())
            .or_default()
            .push(format!(
                "{} {} [{}] via {}",
                node.name,
                node.version,
                node.integrity,
                node.path.join(" -> ")
            ));
    }
    let requirements = context
        .effective_requires
        .iter()
        .map(|capability| capability.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let package_paths = serde_json::to_string(&package_paths)
        .map_err(|error| ApprovalError(format!("could not encode package provenance: {error}")))?;
    let mut environment = BTreeMap::new();
    environment.insert("ZIRK_RUNTIME_PROJECT_ID".into(), context.project_id);
    environment.insert(
        "ZIRK_RUNTIME_MANIFEST_DIGEST".into(),
        context.manifest_digest,
    );
    environment.insert("ZIRK_RUNTIME_PROJECT_NAME".into(), context.project_name);
    environment.insert(
        "ZIRK_RUNTIME_PROJECT_ROOT".into(),
        context.root.display().to_string(),
    );
    environment.insert(
        "ZIRK_RUNTIME_MANIFEST".into(),
        context.root.join(".zkinit").display().to_string(),
    );
    environment.insert("ZIRK_RUNTIME_REQUIRES".into(), requirements);
    environment.insert("ZIRK_RUNTIME_PACKAGE_PATHS".into(), package_paths);
    environment.insert("ZIRK_RUNTIME_MANIFEST_TEMPLATE".into(), manifest_template);
    environment.insert(
        "ZIRK_RUNTIME_SECURITY_DIR".into(),
        data_root()?.display().to_string(),
    );
    Ok(Some(environment))
}

fn standalone_runtime_environment(
    source_path: &Path,
) -> Result<Option<BTreeMap<String, String>>, ApprovalError> {
    if source_path
        .extension()
        .is_none_or(|extension| extension != "zrk")
    {
        return Ok(None);
    }
    let source_path = source_path.canonicalize().map_err(|error| {
        ApprovalError(format!(
            "could not resolve standalone source `{}`: {error}",
            source_path.display()
        ))
    })?;
    let root = source_path
        .parent()
        .ok_or_else(|| ApprovalError("standalone source has no parent directory".into()))?
        .to_path_buf();
    let entry = source_path
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| ApprovalError("standalone source has no file name".into()))?;
    let directory = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let slug = normalize_project_slug(directory);
    let manifest = ProjectManifest {
        name: format!("local.{slug}"),
        version: "0.1.0".into(),
        kind: ProjectKind::Application,
        authors: Vec::new(),
        description: None,
        license: None,
        entry,
        package_root: None,
        source_roots: vec![PathBuf::from(".")],
        build_targets: Vec::new(),
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
        globals: BTreeMap::new(),
        requires: BTreeSet::new(),
        spans: Default::default(),
        environment_permissions: Default::default(),
        network_permissions: Default::default(),
    };
    let manifest_template = project::format_manifest(&manifest);
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"zirk-permission-approval-v2\0");
    digest_field(&mut fingerprint, manifest_template.as_bytes());
    let manifest_digest = format!("{:x}", fingerprint.finalize());
    let package_paths = serde_json::to_string(&BTreeMap::<String, Vec<String>>::new())
        .map_err(|error| ApprovalError(format!("could not encode package provenance: {error}")))?;
    let mut environment = BTreeMap::new();
    environment.insert(
        "ZIRK_RUNTIME_PROJECT_ID".into(),
        project_id(&root, &manifest.name),
    );
    environment.insert("ZIRK_RUNTIME_MANIFEST_DIGEST".into(), manifest_digest);
    environment.insert("ZIRK_RUNTIME_PROJECT_NAME".into(), manifest.name);
    environment.insert(
        "ZIRK_RUNTIME_PROJECT_ROOT".into(),
        root.display().to_string(),
    );
    environment.insert(
        "ZIRK_RUNTIME_MANIFEST".into(),
        root.join(".zkinit").display().to_string(),
    );
    environment.insert("ZIRK_RUNTIME_REQUIRES".into(), String::new());
    environment.insert("ZIRK_RUNTIME_PACKAGE_PATHS".into(), package_paths);
    environment.insert("ZIRK_RUNTIME_MANIFEST_TEMPLATE".into(), manifest_template);
    environment.insert(
        "ZIRK_RUNTIME_SECURITY_DIR".into(),
        data_root()?.display().to_string(),
    );
    Ok(Some(environment))
}

fn normalize_project_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    if slug.is_empty() {
        "project".into()
    } else {
        slug
    }
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

#[cfg(test)]
fn scopes(manifest: &ProjectManifest) -> PermissionScopes {
    PermissionScopes {
        filesystem_read: Default::default(),
        filesystem_write: Default::default(),
        filesystem_metadata: Default::default(),
        filesystem_operations: Default::default(),
        environment_read: manifest.environment_permissions.read_names.clone(),
        environment_secrets: manifest.environment_permissions.secret_names.clone(),
        environment_list_names: manifest.environment_permissions.list_names,
        network_connect: manifest.network_permissions.connect_origins.clone(),
        network_listen: manifest.network_permissions.listen_addresses.clone(),
        network_udp_destinations: manifest.network_permissions.udp_destinations.clone(),
        network_udp_listen: manifest.network_permissions.udp_listen_addresses.clone(),
        network_udp_broadcast_destinations: manifest
            .network_permissions
            .udp_broadcast_destinations
            .clone(),
        network_udp_multicast_groups: manifest.network_permissions.udp_multicast_groups.clone(),
        network_local_connect: manifest.network_permissions.local_connect_endpoints.clone(),
        network_local_listen: manifest.network_permissions.local_listen_endpoints.clone(),
        network_inspect_interfaces: manifest.network_permissions.inspect_interfaces,
        process_execute: Default::default(),
        process_shell: Default::default(),
    }
}

fn project_error(error: ProjectError) -> ApprovalError {
    ApprovalError(error.to_string())
}

/// Blanket approval was replaced by a resource-specific prompt at the
/// protected operation. Keep this entry point for a clear migration message.
pub fn approve(_start: &Path) -> Result<(), ApprovalError> {
    Err(ApprovalError(
        "blanket permission approval was removed; run the project and answer y/n when a protected operation requests an exact resource".into(),
    ))
}

/// Removes the saved approval for this project without changing its manifest.
pub fn revoke(start: &Path) -> Result<(), ApprovalError> {
    revoke_in(start, &data_root()?)
}

/// Removes a project approval from an explicit security directory.
pub fn revoke_in(start: &Path, security_directory: &Path) -> Result<(), ApprovalError> {
    let context = context(start)?;
    let consent = load_consent_state(&context, security_directory)?;
    let previous_history = read_history(&context, security_directory).unwrap_or_default();
    let grant_path = grant_path(
        &security_directory.join("permissions.key"),
        &context.project_id,
    );
    match std::fs::remove_file(&grant_path) {
        Ok(()) => {
            let mut revoked = match consent {
                ConsentState::Current(scopes) => scope_entries(&scopes),
                ConsentState::Missing | ConsentState::Stale => Vec::new(),
            };
            if revoked.is_empty() {
                revoked.push((
                    "permissions.revoke".into(),
                    "stale or unverified project approval record".into(),
                ));
            }
            let actor = current_user();
            for (capability, resource) in revoked {
                let requester = previous_history
                    .iter()
                    .rev()
                    .find(|event| {
                        event.action() == PermissionHistoryAction::Approved
                            && event.capability() == capability
                            && event.resource() == resource
                    })
                    .map(VerifiedPermissionHistoryEvent::requester)
                    .unwrap_or("project-level approval record");
                if let Err(error) = append_history_event(
                    security_directory,
                    &context,
                    PermissionHistoryAction::Revoked,
                    &capability,
                    &resource,
                    requester,
                    &actor,
                ) {
                    eprintln!("warning: approval was revoked but history was not updated: {error}");
                }
            }
            println!("Permission approval revoked for {}.", context.project_name);
        }
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
    if context.project_kind != ProjectKind::Application {
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
    {
        return Ok(None);
    }
    Ok(Some(encoded))
}

/// Creates the child runtime's authenticated grant token for `zirk run`.
/// Projects with missing/stale approval receive no token and stay denied.
pub fn runtime_token_for_path(path: &Path) -> Result<Option<String>, ApprovalError> {
    let Some(project_path) = project_root_for_execution_path(path) else {
        return Ok(None);
    };
    verified_token(&project_path)
}

fn project_root_for_execution_path(path: &Path) -> Option<PathBuf> {
    if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
        return Some(path.to_path_buf());
    }
    if path.extension().is_none_or(|extension| extension != "zrk") {
        return None;
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent
        .ancestors()
        .find(|directory| directory.join(".zkinit").is_file())
        .map(Path::to_path_buf)
}

pub fn print_scopes(scopes: &PermissionScopes) {
    for path in &scopes.filesystem_read {
        println!("  filesystem.read: {path}");
    }
    for path in &scopes.filesystem_write {
        println!("  filesystem.write: {path}");
    }
    for path in &scopes.filesystem_metadata {
        println!("  filesystem.metadata: {path}");
    }
    for scope in &scopes.filesystem_operations {
        let extent = match scope.extent {
            FilesystemScopeExtent::Exact => "exact object",
            FilesystemScopeExtent::Directory => "directory object only",
            FilesystemScopeExtent::Descendants => "directory and descendants",
        };
        println!(
            "  {}: {} (operation: {}; extent: {}; requester: {})",
            scope.capability, scope.resource, scope.operation, extent, scope.requester
        );
    }
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
    for destination in &scopes.network_udp_destinations {
        println!("  network.udp_send: {destination}");
    }
    for address in &scopes.network_udp_listen {
        println!("  network.udp_listen: {address}");
    }
    for destination in &scopes.network_udp_broadcast_destinations {
        println!("  network.udp_broadcast: {destination}");
    }
    for group in &scopes.network_udp_multicast_groups {
        println!("  network.udp_multicast: {group}");
    }
    for endpoint in &scopes.network_local_connect {
        println!("  network.local_connect: {endpoint}");
    }
    for endpoint in &scopes.network_local_listen {
        println!("  network.local_listen: {endpoint}");
    }
    if scopes.network_inspect_interfaces {
        println!("  network.inspect_interfaces: enabled");
    }
    for executable in &scopes.process_execute {
        println!("  process.execute: {executable}");
    }
    for command in &scopes.process_shell {
        println!("  process.shell: {command}");
    }
}

fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "current user".into())
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

#[cfg(test)]
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

fn grant_path(key_path: &Path, project_id: &str) -> PathBuf {
    key_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("approvals")
        .join(format!("{project_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_scopes_are_visible_with_extent_and_requester() {
        let scopes = PermissionScopes {
            filesystem_operations: [
                zirk_runtime_core::permission_grant::FilesystemOperationScope {
                    capability: "filesystem.read".into(),
                    operation: "directory.walk".into(),
                    requester: "example.parser".into(),
                    resource: "/tmp/project/data".into(),
                    extent: FilesystemScopeExtent::Descendants,
                },
            ]
            .into(),
            ..PermissionScopes::default()
        };

        let entries = scope_entries(&scopes);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "filesystem.read");
        assert!(entries[0].1.contains("directory.walk"));
        assert!(entries[0].1.contains("directory and descendants"));
        assert!(entries[0].1.contains("example.parser"));
    }

    fn write_test_key(path: &Path, key: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, key).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    fn permission_project(root: &Path) -> PathBuf {
        let project = root.join("app");
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(project.join("src/main.zrk"), "").unwrap();
        std::fs::write(
            project.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; entry: \"src/main.zrk\"; }\nrequires { network.connect; }\n",
        )
        .unwrap();
        project
    }

    #[test]
    fn blanket_approval_is_rejected_in_favor_of_exact_first_use_consent() {
        let error =
            approve(Path::new("unused")).expect_err("blanket approval must not mint a grant");
        assert!(error.to_string().contains("exact resource"));
    }

    #[test]
    fn manifest_permission_scopes_are_exact() {
        let manifest = project::parse_legacy_manifest(
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
    fn source_paths_resolve_to_their_nearest_project_manifest() {
        let root = std::env::temp_dir().join(format!(
            "zirk-permission-source-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::write(root.join(".zkinit"), "project \"app\"\nkind application\n").unwrap();
        let source = root.join("src/nested/main.zrk");
        std::fs::write(&source, "fn main(): Void {}\n").unwrap();

        assert_eq!(project_root_for_execution_path(&source), Some(root.clone()));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_token_without_a_project_manifest_is_not_loaded() {
        assert!(
            runtime_token_for_path(Path::new("src/main.zrk"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn standalone_runtime_context_prepares_a_stable_application_manifest() {
        let root = std::env::temp_dir().join(format!(
            "zirk-standalone-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("start-here.zrk");
        std::fs::write(&source, "").unwrap();

        let environment = runtime_environment_for_path(&source).unwrap().unwrap();
        let manifest =
            project::parse_manifest(&environment["ZIRK_RUNTIME_MANIFEST_TEMPLATE"]).unwrap();
        assert_eq!(
            manifest.name,
            format!("local.{}", root.file_name().unwrap().to_string_lossy())
        );
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.kind, ProjectKind::Application);
        assert_eq!(manifest.entry, PathBuf::from("start-here.zrk"));
        assert_eq!(manifest.source_roots, [PathBuf::from(".")]);
        assert!(manifest.requires.is_empty());
        assert_eq!(environment["ZIRK_RUNTIME_PROJECT_NAME"], manifest.name);

        std::fs::remove_dir_all(root).unwrap();
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
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\n",
        )
        .unwrap();

        assert_eq!(verified_token_in(&project, &security).unwrap(), None);

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn dependency_requirements_are_transitive_and_invalidate_saved_approval() {
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
        std::fs::create_dir_all(app.join("src")).unwrap();
        std::fs::create_dir_all(direct.join("src")).unwrap();
        std::fs::create_dir_all(nested.join("src")).unwrap();
        std::fs::write(app.join("src/main.zrk"), "").unwrap();
        std::fs::write(direct.join("src/package.zrk"), "").unwrap();
        std::fs::write(nested.join("src/package.zrk"), "").unwrap();
        std::fs::write(
            app.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.direct: { from: \"path:vendor/direct\"; } }\nrequires { environment.read; }\n",
        )
        .unwrap();
        std::fs::write(
            direct.join(".zkinit"),
            "project { name: \"acme.direct\"; version: \"1.0.0\"; type: package; }\ndependencies { acme.nested: { from: \"path:vendor/nested\"; } }\nrequires { secrets.read; }\n",
        )
        .unwrap();
        std::fs::write(
            nested.join(".zkinit"),
            "project { name: \"acme.nested\"; version: \"1.0.0\"; type: package; }\nrequires { network.connect; }\n",
        )
        .unwrap();

        fn write_lock(root: &Path, dep_root: &Path, name: &str, locator: &str) {
            let package_manifest = project::read_manifest(&dep_root.join(".zkinit")).unwrap();
            let hash = project::package_integrity_hash(dep_root, &package_manifest).unwrap();
            let package = project::LockedPackage {
                name: name.into(),
                version: package_manifest.version,
                source: "local".into(),
                locator: locator.into(),
                revision: None,
                hash: format!("sha256:{hash}"),
                package_root: "src/package.zrk".into(),
            };
            std::fs::write(
                root.join("zirk.lock"),
                project::format_lockfile(&project::Lockfile {
                    packages: [(package.name.clone(), package)].into(),
                }),
            )
            .unwrap();
        }
        write_lock(&app, &direct, "acme.direct", "path:vendor/direct");
        write_lock(&direct, &nested, "acme.nested", "path:vendor/nested");

        let approval = context(&app).unwrap();
        assert_eq!(approval.requirement_tree.len(), 2);

        let original_digest = approval.manifest_digest;
        let mut changed_nested_manifest = std::fs::read_to_string(nested.join(".zkinit")).unwrap();
        changed_nested_manifest =
            changed_nested_manifest.replace("network.connect", "network.listen");
        std::fs::write(nested.join(".zkinit"), changed_nested_manifest).unwrap();
        assert_ne!(context(&app).unwrap().manifest_digest, original_digest);

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn bulk_approval_is_unavailable_for_every_project_kind() {
        let error = approve(Path::new("unused")).unwrap_err();
        assert!(error.to_string().contains("exact resource"));
    }

    #[test]
    fn consent_diff_and_revocation_use_only_the_current_signed_record() {
        let base = std::env::temp_dir().join(format!(
            "zirk-permission-diff-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = permission_project(&base);
        let security = base.join("security");
        std::fs::create_dir_all(security.join("approvals")).unwrap();
        let key = [42; 32];
        write_test_key(&security.join("permissions.key"), &key);
        let context = context(&project).unwrap();
        let scopes = PermissionScopes {
            network_connect: ["https://api.example:443".to_owned()].into(),
            ..PermissionScopes::default()
        };
        let signed = SignedPermissionGrant::sign(
            context.project_id.clone(),
            context.manifest_digest.clone(),
            scopes.clone(),
            &key,
        )
        .unwrap();
        std::fs::write(
            grant_path(&security.join("permissions.key"), &context.project_id),
            signed.encode().unwrap(),
        )
        .unwrap();

        assert!(matches!(
            load_consent_state(&context, &security).unwrap(),
            ConsentState::Current(scopes)
                if scopes.network_connect.contains("https://api.example:443")
        ));
        let entries = scope_entries(&scopes);
        assert!(entries.contains(&("network.connect".into(), "https://api.example:443".into())));

        revoke_in(&project, &security).unwrap();
        assert!(matches!(
            load_consent_state(&context, &security).unwrap(),
            ConsentState::Missing
        ));
        let events = read_history(&context, &security).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action(), PermissionHistoryAction::Revoked);
        assert_eq!(events[0].capability(), "network.connect");
        assert_eq!(events[0].resource(), "https://api.example:443");

        std::fs::remove_dir_all(base).unwrap();
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
