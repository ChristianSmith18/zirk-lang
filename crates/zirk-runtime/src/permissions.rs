//! Runtime verification of launcher-issued project permission grants.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, RwLock};

use zirk_runtime_core::environment::validate_name;
use zirk_runtime_core::network_policy::{
    EffectiveNetworkGrant, NetworkOrigin, NetworkPolicyError, validate_local_endpoint,
};
use zirk_runtime_core::permission_grant::{
    PermissionHistoryAction, PermissionScopes, SignedPermissionGrant, SignedPermissionHistoryEvent,
    VerifiedPermissionGrant,
};

const RUNTIME_GRANT_ENV: &str = "ZIRK_RUNTIME_PERMISSION_GRANT";
static VERIFIED_GRANT: OnceLock<Option<VerifiedPermissionGrant>> = OnceLock::new();
static LIVE_SCOPES: OnceLock<RwLock<PermissionScopes>> = OnceLock::new();
static PROMPT_LOCK: Mutex<()> = Mutex::new(());
static WARNED_DENIALS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
static NEXT_TEMP_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

thread_local! {
    /// Set immediately around a compiler-resolved protected operation. This
    /// keeps provenance tied to the active call site even when several
    /// packages request the same capability.
    static ACTIVE_REQUESTER: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Sets the package identity for the protected operation emitted next to this
/// call. The generated code always supplies a valid UTF-8 identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_permission_requester_set(requester: *const u8, length: u64) {
    let Ok(length) = usize::try_from(length) else {
        return;
    };
    if requester.is_null() && length != 0 {
        return;
    }
    let bytes = if length == 0 {
        &[][..]
    } else {
        // SAFETY: generated code passes a pointer to the global string bytes
        // for exactly `length` bytes, which outlives this synchronous call.
        unsafe { std::slice::from_raw_parts(requester, length) }
    };
    let value = String::from_utf8_lossy(bytes).into_owned();
    ACTIVE_REQUESTER.with(|current| *current.borrow_mut() = value);
}

/// Clears the call-site identity after the protected operation returns.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_permission_requester_clear() {
    ACTIVE_REQUESTER.with(|current| current.borrow_mut().clear());
}

fn active_requester() -> String {
    ACTIVE_REQUESTER.with(|current| current.borrow().clone())
}

#[cfg(test)]
fn set_active_requester(value: &str) {
    ACTIVE_REQUESTER.with(|current| *current.borrow_mut() = value.to_owned());
}

/// Returns a project grant only when the signed token from the trusted
/// launcher verifies with the user-owned approval key. Missing, malformed,
/// or stale tokens all mean no authority.
pub fn verified_grant() -> Option<&'static VerifiedPermissionGrant> {
    VERIFIED_GRANT.get_or_init(load_verified_grant).as_ref()
}

/// Builds the process network policy only from a grant verified from the
/// launcher's signed project approval.
pub fn current_network_grant() -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    network_grant_from_scopes(&current_scopes())
}

pub fn network_grant_for(
    capability: &str,
    resource: &str,
) -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    if request_resource(capability, resource) {
        current_network_grant()
    } else {
        Err(NetworkPolicyError::PermissionDenied)
    }
}

/// Requests an exact HTTP destination and, when configured, the exact proxy
/// origin before creating the immutable grant used by the transport.
pub fn network_grant_for_http_request(
    origin: &str,
    proxy_origin: Option<&str>,
) -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    let mut grant = network_grant_for("network.connect", origin)?;
    if let Some(proxy_origin) = proxy_origin
        && proxy_origin != origin
    {
        grant = network_grant_for("network.connect", proxy_origin)?;
    }
    Ok(grant)
}

fn network_grant_from_scopes(
    scopes: &PermissionScopes,
) -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    EffectiveNetworkGrant::from_verified_datagram_scopes(
        scopes.network_connect.iter().cloned(),
        scopes.network_listen.iter().cloned(),
        scopes.network_udp_destinations.iter().cloned(),
        scopes.network_udp_listen.iter().cloned(),
        scopes.network_udp_broadcast_destinations.iter().cloned(),
        scopes.network_udp_multicast_groups.iter().cloned(),
        scopes.network_local_connect.iter().cloned(),
        scopes.network_local_listen.iter().cloned(),
        scopes.network_inspect_interfaces,
    )
    .map(Arc::new)
}

#[cfg(test)]
fn network_grant_from_verified(
    verified: &VerifiedPermissionGrant,
) -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    network_grant_from_scopes(verified.scopes())
}

/// Returns the verified scopes plus approvals accepted during this process.
/// The mutable copy is process-local and is populated only by the prompt
/// broker below after the signed user-owned record has been written.
pub fn current_scopes() -> PermissionScopes {
    LIVE_SCOPES
        .get_or_init(|| {
            RwLock::new(
                verified_grant()
                    .map(|grant| grant.scopes().clone())
                    .unwrap_or_default(),
            )
        })
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[cfg(test)]
pub(crate) fn replace_current_scopes_for_test(scopes: PermissionScopes) {
    let live = LIVE_SCOPES.get_or_init(|| RwLock::new(PermissionScopes::default()));
    *live
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = scopes;
}

/// Requests one concrete runtime resource. A successful response is persisted
/// before the caller proceeds to its protected host effect.
pub fn request_resource(capability: &str, resource: &str) -> bool {
    if !is_canonical_resource(capability, resource) {
        let requester = requester_text();
        warn_denied(capability, resource, &requester);
        return false;
    }
    let current = current_scopes();
    if scope_contains(&current, capability, resource) {
        return true;
    }

    let _prompt = PROMPT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current = current_scopes();
    if scope_contains(&current, capability, resource) {
        return true;
    }
    if !io::stdin().is_terminal() {
        let requester = requester_text();
        warn_denied(capability, resource, &requester);
        return false;
    }

    let requester = requester_text();
    eprint!(
        "Zirk permission request\n  Project: {}\n  Operation: {}\n  Capability: {}\n  Resource: {}\n  Requested by: {}\nAllow this exact operation? [y/N] ",
        std::env::var("ZIRK_RUNTIME_PROJECT_NAME").unwrap_or_else(|_| "standalone program".into()),
        operation_description(capability),
        capability,
        resource,
        requester
    );
    if io::stderr().flush().is_err() {
        warn_denied(capability, resource, &requester);
        return false;
    }
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err()
        || !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    {
        warn_denied(capability, resource, &requester);
        return false;
    }

    let mut updated = current;
    add_scope(&mut updated, capability, resource);
    if should_add_direct_requirement() {
        if !add_manifest_requirement(capability) {
            warn_denied(capability, resource, &requester);
            return false;
        }
    } else if !ensure_manifest_exists() {
        warn_denied(capability, resource, &requester);
        return false;
    }
    if persist_scopes(&updated).is_err() {
        warn_denied(capability, resource, &requester);
        return false;
    }
    if append_approval_history(
        PermissionHistoryAction::Approved,
        capability,
        resource,
        &requester,
    )
    .is_err()
    {
        eprintln!("zirk warning: permission was saved but its history event could not be recorded");
    }
    if let Some(live) = LIVE_SCOPES.get() {
        *live
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = updated;
    }
    true
}

fn scope_contains(scopes: &PermissionScopes, capability: &str, resource: &str) -> bool {
    match capability {
        "filesystem.read" => scopes.filesystem_read.contains(resource),
        "filesystem.write" => scopes.filesystem_write.contains(resource),
        // Full read access includes the less powerful metadata view, while a
        // metadata-only grant never authorizes reading file contents.
        "filesystem.metadata" => {
            scopes.filesystem_metadata.contains(resource)
                || scopes.filesystem_read.contains(resource)
        }
        "network.connect" => scopes.network_connect.contains(resource),
        "network.listen" => scopes.network_listen.contains(resource),
        "network.udp.send" => {
            if let Some(group) = resource.strip_prefix("udp-multicast://") {
                scopes.network_udp_multicast_groups.contains(group)
            } else if let Some(address) = resource.strip_prefix("udp://") {
                let Ok(address) = address.parse::<std::net::SocketAddr>() else {
                    return false;
                };
                scopes.network_udp_destinations.contains(resource)
                    && (!is_ipv4_broadcast(address)
                        || scopes.network_udp_broadcast_destinations.contains(resource))
            } else {
                false
            }
        }
        "network.udp.listen" => scopes.network_udp_listen.contains(resource),
        "network.local.connect" => scopes.network_local_connect.contains(resource),
        "network.local.listen" => scopes.network_local_listen.contains(resource),
        "network.inspect_interfaces" => scopes.network_inspect_interfaces,
        "environment.read" => scopes.environment_read.contains(resource),
        "environment.list_names" => scopes.environment_list_names,
        "secrets.read" => scopes.environment_secrets.contains(resource),
        "process.execute" => scopes.process_execute.contains(resource),
        "process.shell" => scopes.process_shell.contains(resource),
        _ => false,
    }
}

fn is_canonical_resource(capability: &str, resource: &str) -> bool {
    match capability {
        "network.connect" => NetworkOrigin::parse(resource).is_ok(),
        "network.listen" | "network.udp.listen" => resource
            .parse::<std::net::SocketAddr>()
            .is_ok_and(|address| address.to_string() == resource),
        "network.udp.send" => {
            if let Some(group) = resource.strip_prefix("udp-multicast://") {
                group
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_multicast() && address.to_string() == group)
            } else if let Some(destination) = resource.strip_prefix("udp://") {
                destination
                    .parse::<std::net::SocketAddr>()
                    .is_ok_and(|address| address.port() != 0 && address.to_string() == destination)
            } else {
                false
            }
        }
        "network.local.connect" | "network.local.listen" => {
            validate_local_endpoint(resource).is_ok()
        }
        "network.inspect_interfaces" => resource == "network interfaces",
        "environment.read" | "secrets.read" => validate_name(resource).is_ok(),
        "environment.list_names" => resource == "all environment names",
        "filesystem.read" | "filesystem.write" | "filesystem.metadata" => {
            let path = Path::new(resource);
            path.is_absolute()
                && !resource.is_empty()
                && !resource.contains('\0')
                && path.components().all(|component| {
                    !matches!(
                        component,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
                && path.to_str() == Some(resource)
        }
        "process.execute" | "process.shell" => false,
        _ => false,
    }
}

fn is_ipv4_broadcast(address: std::net::SocketAddr) -> bool {
    matches!(address.ip(), std::net::IpAddr::V4(address) if address.is_broadcast())
}

fn operation_description(capability: &str) -> &'static str {
    match capability {
        "filesystem.read" => "read a file",
        "filesystem.write" => "write a file",
        "filesystem.metadata" => "inspect file metadata",
        "network.connect" => "connect to a network service",
        "network.listen" => "listen for TCP connections",
        "network.udp.send" => "send a UDP datagram",
        "network.udp.listen" => "listen for UDP datagrams",
        "network.local.connect" => "connect to a local socket",
        "network.local.listen" => "listen on a local socket",
        "network.inspect_interfaces" => "inspect network interfaces",
        "environment.read" => "read an environment variable",
        "environment.list_names" => "list environment variable names",
        "secrets.read" => "read a secret environment value",
        "process.execute" => "execute a program",
        "process.shell" => "run a shell command",
        _ => "perform a protected operation",
    }
}

fn add_scope(scopes: &mut PermissionScopes, capability: &str, resource: &str) {
    if capability == "network.udp.send"
        && let Some(group) = resource.strip_prefix("udp-multicast://")
    {
        scopes.network_udp_multicast_groups.insert(group.to_owned());
        return;
    }
    if capability == "network.inspect_interfaces" {
        scopes.network_inspect_interfaces = true;
        return;
    }
    if capability == "environment.list_names" {
        scopes.environment_list_names = true;
        return;
    }
    let target = match capability {
        "filesystem.read" => &mut scopes.filesystem_read,
        "filesystem.write" => &mut scopes.filesystem_write,
        "filesystem.metadata" => &mut scopes.filesystem_metadata,
        "network.connect" => &mut scopes.network_connect,
        "network.listen" => &mut scopes.network_listen,
        "network.udp.send" => &mut scopes.network_udp_destinations,
        "network.udp.listen" => &mut scopes.network_udp_listen,
        "network.local.connect" => &mut scopes.network_local_connect,
        "network.local.listen" => &mut scopes.network_local_listen,
        "environment.read" => &mut scopes.environment_read,
        "secrets.read" => &mut scopes.environment_secrets,
        "process.execute" => &mut scopes.process_execute,
        "process.shell" => &mut scopes.process_shell,
        _ => return,
    };
    target.insert(resource.to_owned());
    if capability == "network.udp.send"
        && let Some(destination) = resource.strip_prefix("udp://")
        && let Ok(address) = destination.parse::<std::net::SocketAddr>()
    {
        if is_ipv4_broadcast(address) {
            scopes
                .network_udp_broadcast_destinations
                .insert(resource.to_owned());
        }
    }
}

fn requester_text() -> String {
    let identity = active_requester();
    if identity.is_empty() {
        return "application code".into();
    }
    if identity == "local" {
        return format!(
            "{} (application)",
            std::env::var("ZIRK_RUNTIME_PROJECT_NAME")
                .unwrap_or_else(|_| "standalone program".into())
        );
    }
    let package_paths = std::env::var("ZIRK_RUNTIME_PACKAGE_PATHS")
        .ok()
        .and_then(|value| {
            serde_json::from_str::<std::collections::BTreeMap<String, Vec<String>>>(&value).ok()
        })
        .and_then(|packages| packages.get(&identity).cloned())
        .unwrap_or_default();
    if package_paths.is_empty() {
        format!("{identity} (dependency path unavailable)")
    } else {
        package_paths.join("; ")
    }
}

fn warn_denied(capability: &str, resource: &str, requester: &str) {
    let key = format!("{capability}\0{resource}\0{requester}");
    let warned = WARNED_DENIALS.get_or_init(|| Mutex::new(BTreeSet::new()));
    let mut warned = warned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if warned.insert(key) {
        eprintln!(
            "zirk warning: attempted but denied {capability} for {resource} (requested by {requester})"
        );
    }
}

fn should_add_direct_requirement() -> bool {
    active_requester() == "local"
}

fn add_manifest_requirement(capability: &str) -> bool {
    let Ok(path) = std::env::var("ZIRK_RUNTIME_MANIFEST") else {
        return false;
    };
    let path = Path::new(&path);
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let Ok(template) = std::env::var("ZIRK_RUNTIME_MANIFEST_TEMPLATE") else {
                return false;
            };
            let Some(updated) = insert_requirement(&template, capability) else {
                return false;
            };
            return atomic_create(path, updated.as_bytes()).is_ok();
        }
        Err(_) => return false,
    };
    if manifest_has_requirement(&source, capability) {
        return true;
    }
    let updated = match insert_requirement(&source, capability) {
        Some(updated) => updated,
        None => return false,
    };
    atomic_replace(path, updated.as_bytes()).is_ok()
}

fn ensure_manifest_exists() -> bool {
    let (Ok(path), Ok(template)) = (
        std::env::var("ZIRK_RUNTIME_MANIFEST"),
        std::env::var("ZIRK_RUNTIME_MANIFEST_TEMPLATE"),
    ) else {
        return false;
    };
    ensure_manifest_exists_at(Path::new(&path), &template)
}

fn ensure_manifest_exists_at(path: &Path, template: &str) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_file(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if atomic_create(path, template.as_bytes()).is_ok() {
                return true;
            }
            std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
        }
        Err(_) => false,
    }
}

fn manifest_has_requirement(source: &str, capability: &str) -> bool {
    let Some((start, end)) = requires_block(source) else {
        return false;
    };
    source[start..end]
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '.' || character == '_')
        })
        .any(|word| word == capability)
}

fn insert_requirement(source: &str, capability: &str) -> Option<String> {
    if let Some((_, end)) = requires_block(source) {
        let mut updated = source.to_owned();
        updated.insert_str(end, &format!("\n    {capability};\n"));
        Some(updated)
    } else {
        Some(format!(
            "{}\nrequires {{\n    {capability};\n}}\n",
            source.trim_end()
        ))
    }
}

fn requires_block(source: &str) -> Option<(usize, usize)> {
    let mut index = 0;
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut line_comment = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'"' {
            quoted = true;
            index += 1;
            continue;
        }
        if depth == 0 && bytes[index..].starts_with(b"requires") {
            let before_ok = index == 0 || !is_word_byte(bytes[index - 1]);
            let after = index + "requires".len();
            let after_ok = bytes.get(after).is_none_or(|next| !is_word_byte(*next));
            if before_ok && after_ok {
                let open = source[after..].find('{')? + after;
                let mut block_depth = 1usize;
                let mut cursor = open + 1;
                let start = cursor;
                let mut in_string = false;
                let mut in_comment = false;
                let mut slash_escape = false;
                while cursor < bytes.len() {
                    let current = bytes[cursor];
                    if in_comment {
                        if current == b'\n' {
                            in_comment = false;
                        }
                        cursor += 1;
                        continue;
                    }
                    if in_string {
                        if slash_escape {
                            slash_escape = false;
                        } else if current == b'\\' {
                            slash_escape = true;
                        } else if current == b'"' {
                            in_string = false;
                        }
                        cursor += 1;
                        continue;
                    }
                    if current == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
                        in_comment = true;
                        cursor += 2;
                        continue;
                    }
                    if current == b'"' {
                        in_string = true;
                        cursor += 1;
                        continue;
                    }
                    if current == b'{' {
                        block_depth += 1;
                    }
                    if current == b'}' {
                        block_depth -= 1;
                        if block_depth == 0 {
                            return Some((start, cursor));
                        }
                    }
                    cursor += 1;
                }
                return None;
            }
        }
        if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth = depth.saturating_sub(1);
        }
        index += 1;
    }
    None
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_scopes(scopes: &PermissionScopes) -> Result<(), ()> {
    let project_id = std::env::var("ZIRK_RUNTIME_PROJECT_ID").map_err(|_| ())?;
    let digest = std::env::var("ZIRK_RUNTIME_MANIFEST_DIGEST").map_err(|_| ())?;
    let security = std::env::var("ZIRK_RUNTIME_SECURITY_DIR")
        .map(PathBuf::from)
        .map_err(|_| ())?;
    std::fs::create_dir_all(&security).map_err(|_| ())?;
    secure_directory(&security)?;
    let key_path = security.join("permissions.key");
    let key = load_or_create_key(&key_path)?;
    let signed = SignedPermissionGrant::sign(project_id.clone(), digest, scopes.clone(), &key)
        .map_err(|_| ())?;
    let encoded = signed.encode().map_err(|_| ())?;
    let directory = security.join("approvals");
    std::fs::create_dir_all(&directory).map_err(|_| ())?;
    secure_directory(&directory)?;
    atomic_replace(
        &directory.join(format!("{project_id}.json")),
        encoded.as_bytes(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn append_approval_history(
    action: PermissionHistoryAction,
    capability: &str,
    resource: &str,
    requester: &str,
) -> Result<(), ()> {
    use std::fs::OpenOptions;

    let project_id = std::env::var("ZIRK_RUNTIME_PROJECT_ID").map_err(|_| ())?;
    let manifest_digest = std::env::var("ZIRK_RUNTIME_MANIFEST_DIGEST").map_err(|_| ())?;
    let security = std::env::var("ZIRK_RUNTIME_SECURITY_DIR")
        .map(PathBuf::from)
        .map_err(|_| ())?;
    std::fs::create_dir_all(&security).map_err(|_| ())?;
    secure_directory(&security)?;
    let key = load_or_create_key(&security.join("permissions.key"))?;
    let timestamp_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ())?
        .as_millis();
    let actor = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "current user".into());
    let event = SignedPermissionHistoryEvent::sign(
        project_id.clone(),
        manifest_digest,
        timestamp_unix_ms,
        action,
        capability,
        resource,
        requester,
        actor,
        &key,
    )
    .map_err(|_| ())?;
    let encoded = format!("{}\n", event.encode().map_err(|_| ())?);
    let directory = security.join("approval-history");
    std::fs::create_dir_all(&directory).map_err(|_| ())?;
    secure_directory(&directory)?;
    let path = directory.join(format!("{project_id}.jsonl"));
    if let Ok(metadata) = std::fs::symlink_metadata(&path)
        && !metadata.file_type().is_file()
    {
        return Err(());
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path).map_err(|_| ())?;
    file.write_all(encoded.as_bytes())
        .and_then(|()| file.sync_data())
        .map_err(|_| ())
}

#[cfg(target_arch = "wasm32")]
fn append_approval_history(
    _: PermissionHistoryAction,
    _: &str,
    _: &str,
    _: &str,
) -> Result<(), ()> {
    Err(())
}

#[cfg(target_arch = "wasm32")]
fn persist_scopes(_: &PermissionScopes) -> Result<(), ()> {
    Err(())
}

fn secure_directory(path: &Path) -> Result<(), ()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_dir() {
        return Err(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(());
        }
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|_| ())?;
    }
    Ok(())
}

fn load_or_create_key(path: &Path) -> Result<Vec<u8>, ()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.mode() & 0o077 != 0 || metadata.uid() != unsafe { libc::geteuid() } {
                    return Err(());
                }
            }
            let key = std::fs::read(path).map_err(|_| ())?;
            if key.len() != 32 {
                return Err(());
            }
            Ok(key)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut key = vec![0u8; 32];
            #[cfg(not(target_arch = "wasm32"))]
            getrandom::getrandom(&mut key).map_err(|_| ())?;
            create_private_file(path, &key)?;
            Ok(key)
        }
        Err(_) => Err(()),
    }
}

fn create_private_file(path: &Path, contents: &[u8]) -> Result<(), ()> {
    use std::fs::OpenOptions;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| ())?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|_| ())
}

fn atomic_replace(path: &Path, contents: &[u8]) -> Result<(), ()> {
    let parent = path.parent().ok_or(())?;
    let counter = NEXT_TEMP_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".zirk-approval-{}-{counter}.tmp",
        std::process::id()
    ));
    #[cfg(unix)]
    let existing_permissions = std::fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.file_type().is_file())
        .map(|metadata| metadata.permissions());
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mode = existing_permissions
            .as_ref()
            .map(|permissions| {
                use std::os::unix::fs::PermissionsExt;
                permissions.mode() & 0o777
            })
            .unwrap_or(0o600);
        options.mode(mode);
    }
    let mut file = options.open(&temporary).map_err(|_| ())?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|_| ())?;
    std::fs::rename(&temporary, path).map_err(|_| ())
}

fn atomic_create(path: &Path, contents: &[u8]) -> Result<(), ()> {
    let parent = path.parent().ok_or(())?;
    let counter = NEXT_TEMP_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".zirk-manifest-{}-{counter}.tmp",
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary).map_err(|_| ())?;
        file.write_all(contents)
            .and_then(|()| file.sync_all())
            .map_err(|_| ())?;
        std::fs::hard_link(&temporary, path).map_err(|_| ())?;
        std::fs::remove_file(&temporary).map_err(|_| ())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn load_verified_grant() -> Option<VerifiedPermissionGrant> {
    let encoded = std::env::var(RUNTIME_GRANT_ENV).ok()?;
    let key = read_approval_key(&approval_key_path()?).ok()?;
    let verified = SignedPermissionGrant::decode(&encoded)
        .ok()?
        .verify(&key)
        .ok()?;
    let project_id = std::env::var("ZIRK_RUNTIME_PROJECT_ID").ok()?;
    let manifest_digest = std::env::var("ZIRK_RUNTIME_MANIFEST_DIGEST").ok()?;
    (verified.project_id() == project_id && verified.manifest_digest() == manifest_digest)
        .then_some(verified)
}

fn approval_key_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)?
        .join("Zirk");
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)?
        .join(".local/share/zirk");
    Some(root.join("security/permissions.key"))
}

fn read_approval_key(path: &std::path::Path) -> Result<Vec<u8>, ()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_file() {
        return Err(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Key material readable by another local account cannot authenticate
        // an application permission grant.
        if metadata.mode() & 0o077 != 0 || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(());
        }
    }
    let key = std::fs::read(path).map_err(|_| ())?;
    if key.len() != 32 {
        return Err(());
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_requirement_write_distinguishes_application_from_dependency_identity() {
        set_active_requester("local");
        assert!(should_add_direct_requirement());
        set_active_requester("parser");
        assert!(!should_add_direct_requirement());
        set_active_requester("");
    }

    #[test]
    fn first_use_prompt_describes_each_finite_capability() {
        for (capability, description) in [
            ("filesystem.read", "read a file"),
            ("filesystem.write", "write a file"),
            ("filesystem.metadata", "inspect file metadata"),
            ("network.connect", "connect to a network service"),
            ("network.listen", "listen for TCP connections"),
            ("network.udp.send", "send a UDP datagram"),
            ("network.udp.listen", "listen for UDP datagrams"),
            ("network.local.connect", "connect to a local socket"),
            ("network.local.listen", "listen on a local socket"),
            ("network.inspect_interfaces", "inspect network interfaces"),
            ("environment.read", "read an environment variable"),
            ("environment.list_names", "list environment variable names"),
            ("secrets.read", "read a secret environment value"),
            ("process.execute", "execute a program"),
            ("process.shell", "run a shell command"),
        ] {
            assert_eq!(
                operation_description(capability),
                description,
                "{capability}"
            );
        }
    }

    #[test]
    fn resource_scopes_keep_network_operations_separate_and_canonical() {
        assert!(is_canonical_resource(
            "network.connect",
            "https://api.example:443"
        ));
        assert!(!is_canonical_resource(
            "network.connect",
            "https://api.example/path"
        ));
        assert!(is_canonical_resource("network.listen", "127.0.0.1:0"));
        assert!(!is_canonical_resource("network.listen", "127.0.0.1:080"));
        assert!(is_canonical_resource("network.udp.listen", "127.0.0.1:0"));
        assert!(is_canonical_resource(
            "network.udp.send",
            "udp://192.0.2.10:5353"
        ));
        assert!(is_canonical_resource(
            "network.udp.send",
            "udp-multicast://239.1.2.3"
        ));
        assert!(is_canonical_resource("environment.read", "APP_MODE"));
        assert!(!is_canonical_resource("environment.read", "BAD=NAME"));
        assert!(is_canonical_resource(
            "filesystem.metadata",
            "/tmp/zirk-metadata-scope.txt"
        ));
        assert!(!is_canonical_resource(
            "filesystem.metadata",
            "../secret.txt"
        ));
        assert!(!is_canonical_resource("process.execute", "/usr/bin/tool"));
    }

    #[test]
    fn metadata_scope_does_not_grant_content_read_but_read_includes_metadata() {
        let path = "/tmp/zirk-metadata-scope.txt";
        let metadata_only = PermissionScopes {
            filesystem_metadata: [path.to_owned()].into(),
            ..PermissionScopes::default()
        };
        assert!(scope_contains(&metadata_only, "filesystem.metadata", path));
        assert!(!scope_contains(&metadata_only, "filesystem.read", path));
        assert!(!scope_contains(&metadata_only, "filesystem.write", path));

        let read_scope = PermissionScopes {
            filesystem_read: [path.to_owned()].into(),
            ..PermissionScopes::default()
        };
        assert!(scope_contains(&read_scope, "filesystem.metadata", path));
        assert!(scope_contains(&read_scope, "filesystem.read", path));
    }

    #[test]
    fn udp_broadcast_and_multicast_scopes_are_added_only_for_exact_destinations() {
        let mut scopes = PermissionScopes::default();
        add_scope(
            &mut scopes,
            "network.udp.send",
            "udp://255.255.255.255:9000",
        );
        assert!(scope_contains(
            &scopes,
            "network.udp.send",
            "udp://255.255.255.255:9000"
        ));
        assert!(!scope_contains(
            &scopes,
            "network.udp.send",
            "udp://255.255.255.255:9001"
        ));

        add_scope(&mut scopes, "network.udp.send", "udp-multicast://239.1.2.3");
        add_scope(&mut scopes, "network.udp.send", "udp://239.1.2.3:9000");
        assert!(scope_contains(
            &scopes,
            "network.udp.send",
            "udp-multicast://239.1.2.3"
        ));
        assert!(scope_contains(
            &scopes,
            "network.udp.send",
            "udp://239.1.2.3:9000"
        ));
        assert!(!scope_contains(
            &scopes,
            "network.udp.send",
            "udp://239.1.2.3:9001"
        ));
    }
    use zirk_runtime_core::permission_grant::{PermissionScopes, SignedPermissionGrant};

    #[test]
    fn missing_grant_is_fail_closed() {
        if std::env::var_os(RUNTIME_GRANT_ENV).is_none() {
            assert!(verified_grant().is_none());
        }
    }

    #[test]
    fn verified_project_scopes_build_an_exact_network_policy() {
        let scopes = PermissionScopes {
            network_connect: ["https://api.example:443".into()].into(),
            network_listen: ["127.0.0.1:8080".into()].into(),
            network_udp_destinations: ["udp://192.0.2.9:5353".into()].into(),
            network_udp_listen: ["127.0.0.1:5353".into()].into(),
            network_udp_broadcast_destinations: ["udp://255.255.255.255:9000".into()].into(),
            network_udp_multicast_groups: ["239.1.2.3".into()].into(),
            network_local_connect: ["unix:///tmp/zirk.sock".into()].into(),
            network_local_listen: ["npipe://./pipe/zirk".into()].into(),
            network_inspect_interfaces: true,
            ..PermissionScopes::default()
        };
        let verified = SignedPermissionGrant::sign("a".repeat(64), "b".repeat(64), scopes, b"key")
            .unwrap()
            .verify(b"key")
            .unwrap();
        let grant = network_grant_from_verified(&verified).unwrap();
        assert!(grant.authorize_origin("https://api.example:443").is_ok());
        assert_eq!(
            grant.authorize_origin("https://other.example:443"),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert!(
            grant
                .authorize_listen("127.0.0.1:8080".parse().unwrap())
                .is_ok()
        );
        assert_eq!(
            grant.authorize_listen("127.0.0.1:8081".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            grant.authorize_udp_send("192.0.2.9:5353".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_udp_bind("127.0.0.1:5353".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_udp_broadcast("255.255.255.255:9000".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_udp_multicast("239.1.2.3".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_local_connect("unix:///tmp/zirk.sock"),
            Ok(())
        );
        assert_eq!(grant.authorize_local_listen("npipe://./pipe/zirk"), Ok(()));
        assert_eq!(grant.authorize_interface_inspection(), Ok(()));
    }

    #[test]
    fn standalone_first_use_creates_the_manifest_without_overwriting() {
        let root = std::env::temp_dir().join(format!(
            "zirk-runtime-manifest-create-{}-{}",
            std::process::id(),
            NEXT_TEMP_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let manifest = root.join(".zkinit");
        let template = "project {\n    name: \"local.fixture\";\n    version: \"0.1.0\";\n    type: application;\n    entry: \"main.zrk\";\n}\n\nsource \".\";\n";
        let updated = insert_requirement(template, "network.connect").unwrap();

        atomic_create(&manifest, updated.as_bytes()).unwrap();
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), updated);
        assert!(atomic_create(&manifest, b"replacement").is_err());
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), updated);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_first_use_ensures_a_manifest_without_duplicating_its_requirement() {
        let root = std::env::temp_dir().join(format!(
            "zirk-runtime-dependency-manifest-{}-{}",
            std::process::id(),
            NEXT_TEMP_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let manifest = root.join(".zkinit");
        let template = "project {\n    name: \"local.fixture\";\n    version: \"0.1.0\";\n    type: application;\n    entry: \"main.zrk\";\n}\n\nsource \".\";\n";

        assert!(ensure_manifest_exists_at(&manifest, template));
        let created = std::fs::read_to_string(&manifest).unwrap();
        assert_eq!(created, template);
        assert!(!manifest_has_requirement(&created, "filesystem.read"));
        assert!(ensure_manifest_exists_at(&manifest, "replacement"));
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), template);

        std::fs::remove_dir_all(root).unwrap();
    }
}
