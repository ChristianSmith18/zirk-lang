//! Project metadata for package-aware frontend loading.
//!
//! `.zkinit` remains a small Zirk-specific declarative surface. This parser is
//! intentionally not the source parser: executable declarations and imports
//! are rejected before module loading begins.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use zirk_ast::Program;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Application,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    FilesystemRead,
    FilesystemWrite,
    FilesystemMetadata,
    NetworkConnect,
    NetworkListen,
    NetworkUdpSend,
    NetworkUdpListen,
    NetworkLocalConnect,
    NetworkLocalListen,
    NetworkInspectInterfaces,
    EnvironmentRead,
    EnvironmentListNames,
    SecretsRead,
    ProcessExecute,
    ProcessShell,
}

impl Capability {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "filesystem.read" => Self::FilesystemRead,
            "filesystem.write" => Self::FilesystemWrite,
            "filesystem.metadata" => Self::FilesystemMetadata,
            "network.connect" => Self::NetworkConnect,
            "network.listen" => Self::NetworkListen,
            "network.udp.send" => Self::NetworkUdpSend,
            "network.udp.listen" => Self::NetworkUdpListen,
            "network.local.connect" => Self::NetworkLocalConnect,
            "network.local.listen" => Self::NetworkLocalListen,
            "network.inspect_interfaces" => Self::NetworkInspectInterfaces,
            "environment.read" => Self::EnvironmentRead,
            "environment.list_names" => Self::EnvironmentListNames,
            "secrets.read" => Self::SecretsRead,
            "process.execute" => Self::ProcessExecute,
            "process.shell" => Self::ProcessShell,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FilesystemRead => "filesystem.read",
            Self::FilesystemWrite => "filesystem.write",
            Self::FilesystemMetadata => "filesystem.metadata",
            Self::NetworkConnect => "network.connect",
            Self::NetworkListen => "network.listen",
            Self::NetworkUdpSend => "network.udp.send",
            Self::NetworkUdpListen => "network.udp.listen",
            Self::NetworkLocalConnect => "network.local.connect",
            Self::NetworkLocalListen => "network.local.listen",
            Self::NetworkInspectInterfaces => "network.inspect_interfaces",
            Self::EnvironmentRead => "environment.read",
            Self::EnvironmentListNames => "environment.list_names",
            Self::SecretsRead => "secrets.read",
            Self::ProcessExecute => "process.execute",
            Self::ProcessShell => "process.shell",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestSpans {
    pub fields: BTreeMap<String, ManifestSpan>,
    pub dependencies: BTreeMap<String, ManifestSpan>,
    pub dev_dependencies: BTreeMap<String, ManifestSpan>,
    pub globals: BTreeMap<String, ManifestSpan>,
    pub requirements: BTreeMap<Capability, ManifestSpan>,
    pub source_roots: Vec<ManifestSpan>,
    pub build_targets: Vec<ManifestSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalVariable {
    pub type_name: String,
    pub default: Option<String>,
    pub from: Option<PathBuf>,
    pub readonly: bool,
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
    pub revision: Option<String>,
    pub integrity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifest {
    pub name: String,
    pub version: String,
    pub kind: ProjectKind,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    /// Effective source file selected as application entry or package root.
    pub entry: PathBuf,
    /// Public source root for packages; equal to `entry` for the current
    /// package-loading compatibility layer.
    pub package_root: Option<PathBuf>,
    pub source_roots: Vec<PathBuf>,
    pub build_targets: Vec<String>,
    pub dependencies: BTreeMap<String, Dependency>,
    pub dev_dependencies: BTreeMap<String, Dependency>,
    pub globals: BTreeMap<String, GlobalVariable>,
    pub requires: std::collections::BTreeSet<Capability>,
    pub spans: ManifestSpans,
    /// Environment requirements declared by this application or package.
    /// Declarations describe requested authority only; they never grant it.
    pub environment_permissions: EnvironmentPermissions,
    /// Network requirements declared by this application or package. They
    /// request authority but never grant it.
    pub network_permissions: NetworkPermissions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentPermissions {
    pub read_names: std::collections::BTreeSet<String>,
    pub secret_names: std::collections::BTreeSet<String>,
    pub list_names: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkPermissions {
    pub connect_origins: std::collections::BTreeSet<String>,
    pub listen_addresses: std::collections::BTreeSet<String>,
    pub udp_destinations: std::collections::BTreeSet<String>,
    pub udp_listen_addresses: std::collections::BTreeSet<String>,
    pub udp_broadcast_destinations: std::collections::BTreeSet<String>,
    pub udp_multicast_groups: std::collections::BTreeSet<String>,
    pub local_connect_endpoints: std::collections::BTreeSet<String>,
    pub local_listen_endpoints: std::collections::BTreeSet<String>,
    pub inspect_interfaces: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub locator: String,
    #[serde(default)]
    pub revision: Option<String>,
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

/// Parses and validates the canonical `.zkinit` grammar.
pub fn parse_manifest(text: &str) -> Result<ProjectManifest, ProjectError> {
    if looks_like_legacy_manifest(text) {
        return Err(ProjectError(
            "legacy `.zkinit` syntax is not accepted for normal project loading; run `zirk migrate --check` to preview conversion".into(),
        ));
    }
    crate::manifest_parser::parse_manifest(text)
}

fn looks_like_legacy_manifest(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        line.starts_with("project \"")
            || line.starts_with("kind ")
            || line.starts_with("entry \"")
            || line.starts_with("dependency ")
    })
}

/// Formats a project model in the canonical, stable `.zkinit` representation.
pub fn format_manifest(manifest: &ProjectManifest) -> String {
    crate::manifest_parser::format_manifest(manifest)
}

/// Reads legacy manifests only for the explicit migration path. Ordinary
/// project loading calls [`parse_manifest`] and rejects the removed syntax.
pub fn parse_legacy_manifest(text: &str) -> Result<ProjectManifest, ProjectError> {
    let mut name = None;
    let mut version = None;
    let mut kind = None;
    let mut entry = None;
    let mut source_roots = Vec::new();
    let mut dependencies = BTreeMap::new();
    let mut environment_permissions = EnvironmentPermissions::default();
    let mut network_permissions = NetworkPermissions::default();
    let mut project_block = false;
    let mut permission_block: Option<&str> = None;
    let mut environment_block = false;
    let mut network_block = false;
    for raw in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
    {
        if raw == "project {" {
            if project_block || permission_block.is_some() {
                return Err(ProjectError(
                    "nested `.zkinit` blocks are not allowed".into(),
                ));
            }
            project_block = true;
            continue;
        }
        if project_block {
            if raw == "}" {
                project_block = false;
                continue;
            }
            let (field, value) = raw
                .strip_suffix(';')
                .unwrap_or(raw)
                .split_once(':')
                .ok_or_else(|| ProjectError(format!("invalid project field `{raw}`")))?;
            let value = value.trim();
            match field.trim() {
                "name" => name = Some(quoted(value)?),
                "version" => {
                    version = Some(quoted(value)?);
                }
                "type" => {
                    kind = Some(parse_project_kind(value.trim_end_matches(';').trim())?);
                }
                "entry" => entry = Some(PathBuf::from(quoted(value)?)),
                other => {
                    return Err(ProjectError(format!("unknown project field `{other}`")));
                }
            }
            continue;
        }
        if raw == "permissions {" {
            if permission_block.is_some() {
                return Err(ProjectError(
                    "nested `permissions` blocks are not allowed".into(),
                ));
            }
            permission_block = Some("permissions");
            continue;
        }
        if permission_block.is_some() {
            match raw {
                "}" if environment_block => environment_block = false,
                "}" if network_block => network_block = false,
                "}" => permission_block = None,
                "environment {" if !environment_block && !network_block => environment_block = true,
                "network {" if !network_block && !environment_block => network_block = true,
                _ if environment_block => {
                    parse_environment_permission(raw, &mut environment_permissions)?;
                }
                _ if network_block => parse_network_permission(raw, &mut network_permissions)?,
                _ => {
                    return Err(ProjectError(format!(
                        "unknown permissions declaration `{raw}`"
                    )));
                }
            }
            continue;
        }
        let mut pieces = raw.split_whitespace();
        match pieces.next() {
            Some("project") => {
                name =
                    Some(quoted(pieces.next().ok_or_else(|| {
                        ProjectError("project needs a name".into())
                    })?)?)
            }
            Some("kind") => kind = Some(parse_project_kind(pieces.next().unwrap_or_default())?),
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
                        revision: None,
                        integrity: None,
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
    if project_block || permission_block.is_some() || environment_block || network_block {
        return Err(ProjectError("unclosed `.zkinit` permissions block".into()));
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
    let mut requires = std::collections::BTreeSet::new();
    if !environment_permissions.read_names.is_empty() {
        requires.insert(Capability::EnvironmentRead);
    }
    if environment_permissions.list_names {
        requires.insert(Capability::EnvironmentListNames);
    }
    if !environment_permissions.secret_names.is_empty() {
        requires.insert(Capability::SecretsRead);
    }
    if !network_permissions.connect_origins.is_empty() {
        requires.insert(Capability::NetworkConnect);
    }
    if !network_permissions.listen_addresses.is_empty() {
        requires.insert(Capability::NetworkListen);
    }
    if !network_permissions.udp_destinations.is_empty()
        || !network_permissions.udp_broadcast_destinations.is_empty()
        || !network_permissions.udp_multicast_groups.is_empty()
    {
        requires.insert(Capability::NetworkUdpSend);
    }
    if !network_permissions.udp_listen_addresses.is_empty() {
        requires.insert(Capability::NetworkUdpListen);
    }
    if !network_permissions.local_connect_endpoints.is_empty() {
        requires.insert(Capability::NetworkLocalConnect);
    }
    if !network_permissions.local_listen_endpoints.is_empty() {
        requires.insert(Capability::NetworkLocalListen);
    }
    if network_permissions.inspect_interfaces {
        requires.insert(Capability::NetworkInspectInterfaces);
    }
    Ok(ProjectManifest {
        name,
        version: version.unwrap_or_else(|| "0.1.0".into()),
        kind,
        authors: Vec::new(),
        description: None,
        license: None,
        entry: entry.clone(),
        package_root: (kind == ProjectKind::Package).then_some(entry),
        source_roots,
        build_targets: Vec::new(),
        dependencies,
        dev_dependencies: BTreeMap::new(),
        globals: BTreeMap::new(),
        requires,
        spans: ManifestSpans::default(),
        environment_permissions,
        network_permissions,
    })
}

/// Converts supported legacy manifest text to the canonical syntax. Mapped
/// permissions are declarations only; this function never creates consent.
pub fn migrate_legacy_manifest_text(text: &str) -> Result<String, ProjectError> {
    let manifest = parse_legacy_manifest(text)?;
    if let Some(dependency) = manifest.dependencies.values().find(|dependency| {
        matches!(dependency.source, DependencySource::Git(_)) && dependency.revision.is_none()
    }) {
        return Err(ProjectError(format!(
            "legacy Git dependency `{}` has no immutable revision; add an explicit `rev` before migration",
            dependency.name
        )));
    }
    Ok(format_manifest(&manifest))
}

/// Returns the canonical preview and, when requested, atomically replaces the
/// manifest while preserving the original at `<manifest>.pre-migrate`.
pub fn migrate_manifest_file(path: &Path, apply: bool) -> Result<(String, String), ProjectError> {
    let original = std::fs::read_to_string(path)
        .map_err(|error| ProjectError(format!("could not read `{}`: {error}", path.display())))?;
    let converted = migrate_legacy_manifest_text(&original)?;
    if !apply {
        return Ok((original, converted));
    }

    let backup = path.with_file_name(format!(
        "{}.pre-migrate",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let mut backup_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| {
            ProjectError(format!(
                "could not create recoverable backup `{}`: {error}",
                backup.display()
            ))
        })?;
    use std::io::Write;
    backup_file
        .write_all(original.as_bytes())
        .and_then(|_| backup_file.sync_all())
        .map_err(|error| {
            ProjectError(format!(
                "could not write recoverable backup `{}`: {error}",
                backup.display()
            ))
        })?;

    let temp = path.with_file_name(format!(".zkinit.tmp.{}", std::process::id()));
    let write_result = (|| {
        let mut temp_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| {
                ProjectError(format!("could not create temporary manifest: {error}"))
            })?;
        temp_file
            .write_all(converted.as_bytes())
            .and_then(|_| temp_file.sync_all())
            .map_err(|error| {
                ProjectError(format!("could not write temporary manifest: {error}"))
            })?;
        std::fs::rename(&temp, path).map_err(|error| {
            ProjectError(format!(
                "could not atomically replace `{}`: {error}",
                path.display()
            ))
        })
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    write_result?;
    Ok((original, converted))
}

fn parse_project_kind(value: &str) -> Result<ProjectKind, ProjectError> {
    match value {
        "application" => Ok(ProjectKind::Application),
        "package" => Ok(ProjectKind::Package),
        _ => Err(ProjectError(
            "project type must be `application` or `package`".into(),
        )),
    }
}

fn parse_network_permission(
    line: &str,
    permissions: &mut NetworkPermissions,
) -> Result<(), ProjectError> {
    let (kind, body) = line
        .split_once(": {")
        .ok_or_else(|| ProjectError(format!("invalid network permission `{line}`")))?;
    let body = body
        .strip_suffix('}')
        .ok_or_else(|| ProjectError(format!("invalid network permission `{line}`")))?
        .trim();
    let scope = match kind {
        "connect" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("origins: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "listen" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("addresses: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "udp_send" | "udp_broadcast" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("destinations: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "udp_listen" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("addresses: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "udp_multicast" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("groups: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "local_connect" | "local_listen" => body
            .strip_suffix("during: runtime;")
            .map(str::trim_end)
            .and_then(|body| body.strip_suffix(';'))
            .map(str::trim_end)
            .and_then(|body| body.strip_prefix("endpoints: ["))
            .and_then(|rest| rest.strip_suffix(']')),
        "inspect_interfaces" => {
            if body == "enabled: true; during: runtime;" {
                permissions.inspect_interfaces = true;
                return Ok(());
            }
            return Err(ProjectError(
                "invalid `inspect_interfaces` network permission scope".into(),
            ));
        }
        _ => return Err(ProjectError(format!("unknown network permission `{kind}`"))),
    }
    .ok_or_else(|| ProjectError(format!("invalid `{kind}` network permission scope")))?;

    for value in scope
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let value = quoted(value)?;
        match kind {
            "connect" => {
                zirk_runtime_core::network_policy::NetworkOrigin::parse(&value)
                    .map_err(|_| ProjectError(format!("invalid network origin `{value}`")))?;
                if !value.starts_with("http://")
                    && !value.starts_with("https://")
                    && !value.starts_with("tcp://")
                {
                    return Err(ProjectError(format!(
                        "network origin `{value}` must be a canonical http, https, or tcp origin"
                    )));
                }
                permissions.connect_origins.insert(value);
            }
            "listen" => {
                value.parse::<std::net::SocketAddr>().map_err(|_| {
                    ProjectError(format!("listen address `{value}` must be an IP:port pair"))
                })?;
                permissions.listen_addresses.insert(value);
            }
            "udp_send" => {
                let address = value
                    .strip_prefix("udp://")
                    .ok_or_else(|| ProjectError(format!("invalid UDP destination `{value}`")))?
                    .parse::<std::net::SocketAddr>()
                    .map_err(|_| {
                        ProjectError(format!(
                            "UDP destination `{value}` must be udp://IP:port with an explicit port"
                        ))
                    })?;
                if address.port() == 0 || value != format!("udp://{address}") {
                    return Err(ProjectError(format!(
                        "UDP destination `{value}` is not canonical"
                    )));
                }
                permissions.udp_destinations.insert(value);
            }
            "udp_broadcast" => {
                let address = value
                    .strip_prefix("udp://")
                    .ok_or_else(|| {
                        ProjectError(format!("invalid UDP broadcast destination `{value}`"))
                    })?
                    .parse::<std::net::SocketAddr>()
                    .map_err(|_| {
                        ProjectError(format!(
                            "UDP broadcast destination `{value}` must be udp://IP:port"
                        ))
                    })?;
                if address.port() == 0
                    || !matches!(address.ip(), std::net::IpAddr::V4(ip) if ip.is_broadcast())
                    || value != format!("udp://{address}")
                {
                    return Err(ProjectError(format!(
                        "UDP broadcast destination `{value}` is not canonical limited broadcast"
                    )));
                }
                permissions.udp_broadcast_destinations.insert(value);
            }
            "udp_multicast" => {
                let group = value.parse::<std::net::IpAddr>().map_err(|_| {
                    ProjectError(format!(
                        "UDP multicast group `{value}` must be a canonical IP literal"
                    ))
                })?;
                let canonical = group.to_string();
                let is_multicast = match group {
                    std::net::IpAddr::V4(address) => address.is_multicast(),
                    std::net::IpAddr::V6(address) => address.is_multicast(),
                };
                if !is_multicast || value != canonical {
                    return Err(ProjectError(format!(
                        "UDP multicast group `{value}` is not a canonical multicast IP"
                    )));
                }
                permissions.udp_multicast_groups.insert(value);
            }
            "udp_listen" => {
                value.parse::<std::net::SocketAddr>().map_err(|_| {
                    ProjectError(format!(
                        "UDP listen address `{value}` must be an IP:port pair"
                    ))
                })?;
                permissions.udp_listen_addresses.insert(value);
            }
            "local_connect" | "local_listen" => {
                if !is_canonical_local_endpoint(&value) {
                    return Err(ProjectError(format!(
                        "local endpoint `{value}` must be a canonical unix or npipe endpoint"
                    )));
                }
                if kind == "local_connect" {
                    permissions.local_connect_endpoints.insert(value);
                } else {
                    permissions.local_listen_endpoints.insert(value);
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn is_canonical_local_endpoint(value: &str) -> bool {
    zirk_runtime_core::network_policy::validate_local_endpoint(value).is_ok()
}

fn parse_environment_permission(
    line: &str,
    permissions: &mut EnvironmentPermissions,
) -> Result<(), ProjectError> {
    let (kind, body) = line
        .split_once(": {")
        .ok_or_else(|| ProjectError(format!("invalid environment permission `{line}`")))?;
    let body = body
        .strip_suffix("}")
        .ok_or_else(|| ProjectError(format!("invalid environment permission `{line}`")))?
        .trim();
    if !body.ends_with("during: runtime;") {
        return Err(ProjectError(
            "environment permissions must use `during: runtime`".into(),
        ));
    }
    if kind == "list_names" {
        if body != "during: runtime;" {
            return Err(ProjectError(
                "`list_names` only accepts `during: runtime`".into(),
            ));
        }
        permissions.list_names = true;
        return Ok(());
    }
    let names = body
        .strip_suffix("during: runtime;")
        .map(str::trim_end)
        .and_then(|body| body.strip_suffix(';'))
        .map(str::trim_end)
        .and_then(|body| body.strip_prefix("names: ["))
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| ProjectError(format!("`{kind}` needs a `names` array")))?;
    let target = match kind {
        "read" => &mut permissions.read_names,
        "secrets" => &mut permissions.secret_names,
        _ => {
            return Err(ProjectError(format!(
                "unknown environment permission `{kind}`"
            )));
        }
    };
    for name in names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let name = quoted(name)?;
        if name.is_empty()
            || name.contains('=')
            || name.contains('\0')
            || name.to_ascii_uppercase().starts_with("ZIRK_RUNTIME_")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(ProjectError(format!(
                "invalid environment variable name `{name}`"
            )));
        }
        target.insert(name);
    }
    Ok(())
}

pub fn parse_lockfile(text: &str) -> Result<Lockfile, ProjectError> {
    if text.trim_start().starts_with('{') {
        let document: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| ProjectError(format!("invalid `zirk.lock` JSON: {error}")))?;
        let version = document
            .get("lock_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ProjectError("`zirk.lock` is missing numeric `lock_version`".into()))?;
        if version != 2 {
            return Err(ProjectError(format!(
                "unsupported `zirk.lock` version {version}; expected 2"
            )));
        }
        let packages = document
            .get("packages")
            .cloned()
            .ok_or_else(|| ProjectError("`zirk.lock` is missing `packages`".into()))?;
        let packages: BTreeMap<String, LockedPackage> = serde_json::from_value(packages)
            .map_err(|error| ProjectError(format!("invalid locked package record: {error}")))?;
        for (name, package) in &packages {
            if name != &package.name {
                return Err(ProjectError(format!(
                    "lock package key `{name}` does not match its logical name `{}`",
                    package.name
                )));
            }
        }
        return Ok(Lockfile { packages });
    }

    // Read-only support for the previous five-column format is retained so
    // `zirk migrate` and old lock diagnostics can identify stale projects.
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
            locator: String::new(),
            revision: None,
            hash: fields[3].into(),
            package_root: fields[4].into(),
        };
        packages.insert(package.name.clone(), package);
    }
    Ok(Lockfile { packages })
}

pub fn format_lockfile(lock: &Lockfile) -> String {
    let value = serde_json::json!({
        "lock_version": 2,
        "packages": lock.packages,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()) + "\n"
}

pub fn dependency_locator(source: &DependencySource) -> String {
    match source {
        DependencySource::Local(path) => format!("path:{}", path.to_string_lossy()),
        DependencySource::Registry(name) => format!("registry:{name}"),
        DependencySource::Git(url) => format!("git+{url}"),
        DependencySource::Vendored(path) => format!("vendored:{}", path.to_string_lossy()),
    }
}

pub(crate) fn canonical_child_without_symlinks(
    base: &Path,
    relative: &Path,
    label: &str,
) -> Result<PathBuf, ProjectError> {
    if relative.is_absolute() {
        return Err(ProjectError(format!("{label} must be a relative path")));
    }
    let mut cursor = base.canonicalize().map_err(|error| {
        ProjectError(format!(
            "could not resolve {label} base `{}`: {error}",
            base.display()
        ))
    })?;
    for component in relative.components() {
        match component {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Err(ProjectError(format!("{label} must be project-relative")));
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cursor.pop();
            }
            std::path::Component::Normal(part) => cursor.push(part),
        }
        if let Ok(metadata) = std::fs::symlink_metadata(&cursor)
            && metadata.file_type().is_symlink()
        {
            return Err(ProjectError(format!(
                "{label} contains symbolic link `{}`",
                cursor.display()
            )));
        }
    }
    cursor.canonicalize().map_err(|error| {
        ProjectError(format!(
            "could not resolve {label} `{}`: {error}",
            relative.display()
        ))
    })
}

/// Resolves the optional source file attached to each custom application
/// global and rejects missing paths or symbolic-link escapes before source
/// loading begins.
pub fn validate_global_type_sources(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<(), ProjectError> {
    for (name, global) in &manifest.globals {
        let Some(source) = &global.from else {
            continue;
        };
        let path = canonical_child_without_symlinks(
            root,
            source,
            &format!("global `{name}` type source"),
        )?;
        if !path.is_file() {
            return Err(ProjectError(format!(
                "global `{name}` type source `{}` is not a file",
                source.display()
            )));
        }
        let source_text = std::fs::read_to_string(&path).map_err(|error| {
            ProjectError(format!(
                "could not read global `{name}` type source `{}`: {error}",
                source.display()
            ))
        })?;
        let source_file = zirk_diagnostics::SourceFile::new(path.to_string_lossy(), &source_text);
        let mut diagnostics = zirk_diagnostics::DiagnosticSink::new();
        let tokens = zirk_lexer::tokenize(&source_file, &mut diagnostics);
        let program = zirk_parser::parse(&source_file, &tokens, &mut diagnostics);
        if diagnostics.has_errors() {
            return Err(ProjectError(format!(
                "global `{name}` type source `{}` contains invalid Zirk syntax",
                source.display()
            )));
        }
        if !program.body.is_empty() {
            return Err(ProjectError(format!(
                "global `{name}` type source `{}` must not contain executable file-scope statements",
                source.display()
            )));
        }
        let declares_type = program
            .classes
            .iter()
            .any(|declaration| declaration.name.name == global.type_name)
            || program
                .enums
                .iter()
                .any(|declaration| declaration.name.name == global.type_name)
            || program
                .contracts
                .iter()
                .any(|declaration| declaration.name.name == global.type_name)
            || program
                .type_aliases
                .iter()
                .any(|declaration| declaration.name.name == global.type_name);
        if !declares_type {
            return Err(ProjectError(format!(
                "global `{name}` declares type `{}`, but `{}` does not declare that type",
                global.type_name,
                source.display()
            )));
        }
    }
    Ok(())
}

fn version_matches(constraint: &str, version: &str) -> Result<bool, ProjectError> {
    if constraint.is_empty() {
        return Ok(true);
    }
    let version = parse_semver_version(version).ok_or_else(|| {
        ProjectError(format!(
            "package version `{version}` is not valid semantic versioning"
        ))
    })?;
    let requirement = semver::VersionReq::parse(constraint).map_err(|error| {
        ProjectError(format!(
            "invalid version requirement `{constraint}`: {error}"
        ))
    })?;
    Ok(requirement.matches(&version))
}

pub fn lock_version_satisfies(constraint: &str, version: &str) -> Result<bool, ProjectError> {
    version_matches(constraint, version)
}

fn parse_semver_version(value: &str) -> Option<semver::Version> {
    semver::Version::parse(value).ok().or_else(|| {
        let components: Vec<_> = value.split('.').collect();
        match components.as_slice() {
            [major] if major.bytes().all(|byte| byte.is_ascii_digit()) => {
                semver::Version::parse(&format!("{major}.0.0")).ok()
            }
            [major, minor]
                if major.bytes().all(|byte| byte.is_ascii_digit())
                    && minor.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                semver::Version::parse(&format!("{major}.{minor}.0")).ok()
            }
            _ => None,
        }
    })
}

pub fn discover(start: &Path) -> Option<PathBuf> {
    let absolute_start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };
    let mut cursor = if absolute_start.is_dir() {
        absolute_start
    } else {
        absolute_start.parent()?.to_path_buf()
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
            let canonical_dependency_root = canonical_child_without_symlinks(
                root,
                path,
                &format!("local dependency `{}`", dependency.name),
            )?;
            let dependency_manifest = read_manifest(&canonical_dependency_root.join(".zkinit"))?;
            if dependency_manifest.kind != ProjectKind::Package
                || dependency_manifest.name != dependency.name
            {
                return Err(ProjectError(format!(
                    "local dependency `{}` resolves to `{}`; its manifest must declare the same package name and `type: package`",
                    dependency.name, dependency_manifest.name
                )));
            }
            if !version_matches(&dependency.constraint, &dependency_manifest.version)? {
                return Err(ProjectError(format!(
                    "local dependency `{}` has version `{}` which does not satisfy `{}`",
                    dependency.name, dependency_manifest.version, dependency.constraint
                )));
            }
            let package_manifest_path = canonical_child_without_symlinks(
                &canonical_dependency_root,
                Path::new(".zkinit"),
                &format!("local dependency `{}` manifest", dependency.name),
            )?;
            if !package_manifest_path.starts_with(&canonical_dependency_root) {
                return Err(ProjectError(format!(
                    "local dependency `{}` manifest escapes its canonical package root",
                    dependency.name
                )));
            }
            visit(
                &canonical_dependency_root,
                &dependency_manifest,
                roots,
                stack,
            )?;
        }
        stack.pop();
        Ok(())
    }
    let mut roots = BTreeMap::new();
    visit(root, manifest, &mut roots, &mut Vec::new())?;
    Ok(roots)
}

/// Resolves every locked package through its logical name. Remote transports
/// are materialized by `zirk install`; all packages then use the same source
/// import namespace.
pub fn resolve_package_roots(
    root: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    resolve_package_roots_with_dev(root, manifest, lock, false)
}

/// Resolves the runtime graph and, when requested, the current application's
/// direct development dependencies. Package development dependencies are
/// never inherited while traversing a dependency graph.
pub fn resolve_package_roots_with_dev(
    root: &Path,
    manifest: &ProjectManifest,
    lock: &Lockfile,
    include_root_dev_dependencies: bool,
) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    fn visit(
        root: &Path,
        manifest: &ProjectManifest,
        lock: &Lockfile,
        roots: &mut BTreeMap<String, PathBuf>,
        visited: &mut BTreeSet<PathBuf>,
        include_root_dev_dependencies: bool,
    ) -> Result<(), ProjectError> {
        validate_lockfile_with_dev(root, manifest, Some(lock), include_root_dev_dependencies)?;
        let mut dependencies: Vec<_> = manifest.dependencies.values().collect();
        if include_root_dev_dependencies {
            dependencies.extend(manifest.dev_dependencies.values());
        }
        for dependency in dependencies {
            let entry = lock.packages.get(&dependency.name).ok_or_else(|| {
                ProjectError(format!(
                    "dependency `{}` is missing from `zirk.lock`",
                    dependency.name
                ))
            })?;
            let package_root =
                resolved_dependency_root(root, dependency, entry).ok_or_else(|| {
                    ProjectError(format!(
                        "dependency `{}` has no materialized package identity",
                        dependency.name
                    ))
                })?;
            let canonical_root = match &dependency.source {
                DependencySource::Local(path) | DependencySource::Vendored(path) => {
                    canonical_child_without_symlinks(
                        root,
                        path,
                        &format!("dependency `{}` package root", dependency.name),
                    )?
                }
                DependencySource::Registry(_) | DependencySource::Git(_) => {
                    package_root.canonicalize().map_err(|error| {
                        ProjectError(format!(
                            "could not resolve dependency `{}` cache `{}`: {error}",
                            dependency.name,
                            package_root.display()
                        ))
                    })?
                }
            };
            let package_manifest = read_manifest(&canonical_root.join(".zkinit"))?;
            roots.insert(
                dependency.name.clone(),
                canonical_root.join(&entry.package_root),
            );
            if visited.insert(canonical_root.clone()) {
                visit(
                    &canonical_root,
                    &package_manifest,
                    lock,
                    roots,
                    visited,
                    false,
                )?;
            }
        }
        Ok(())
    }

    let mut roots = BTreeMap::new();
    visit(
        root,
        manifest,
        lock,
        &mut roots,
        &mut BTreeSet::new(),
        include_root_dev_dependencies,
    )?;
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
        DependencySource::Registry(_) | DependencySource::Git(_) => {
            let identity = lock
                .revision
                .as_deref()
                .or_else(|| lock.hash.strip_prefix("sha256:"))?;
            Some(
                root.join(".zirk")
                    .join("packages")
                    .join(&dependency.name)
                    .join(&lock.version)
                    .join(identity),
            )
        }
    }
}

pub fn validate_lockfile(
    root: &Path,
    manifest: &ProjectManifest,
    lock: Option<&Lockfile>,
) -> Result<(), ProjectError> {
    validate_lockfile_with_dev(root, manifest, lock, false)
}

pub fn validate_lockfile_with_dev(
    root: &Path,
    manifest: &ProjectManifest,
    lock: Option<&Lockfile>,
    include_root_dev_dependencies: bool,
) -> Result<(), ProjectError> {
    if manifest.dependencies.is_empty()
        && (!include_root_dev_dependencies || manifest.dev_dependencies.is_empty())
    {
        return Ok(());
    }
    let lock =
        lock.ok_or_else(|| ProjectError("project dependencies require `zirk.lock`".into()))?;
    let mut dependencies: Vec<_> = manifest.dependencies.values().collect();
    if include_root_dev_dependencies {
        dependencies.extend(manifest.dev_dependencies.values());
    }
    for dependency in dependencies {
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
        let expected_locator = dependency_locator(&dependency.source);
        if entry.locator.is_empty() {
            return Err(ProjectError(format!(
                "lock entry `{}` lacks its source locator; run `zirk install` to refresh `zirk.lock`",
                dependency.name
            )));
        }
        if entry.locator != expected_locator {
            return Err(ProjectError(format!(
                "lock entry `{}` uses locator `{}` but the manifest declares `{expected_locator}`",
                dependency.name, entry.locator
            )));
        }
        match &dependency.source {
            DependencySource::Git(_) => {
                let revision = entry.revision.as_deref().ok_or_else(|| {
                    ProjectError(format!(
                        "Git lock entry `{}` is missing its immutable revision",
                        dependency.name
                    ))
                })?;
                if !matches!(revision.len(), 40 | 64)
                    || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(ProjectError(format!(
                        "Git lock entry `{}` has a non-immutable revision `{revision}`",
                        dependency.name
                    )));
                }
            }
            _ if entry.revision.is_some() => {
                return Err(ProjectError(format!(
                    "non-Git lock entry `{}` must not contain a Git revision",
                    dependency.name
                )));
            }
            _ => {}
        }
        if entry.package_root.is_empty() || Path::new(&entry.package_root).is_absolute() {
            return Err(ProjectError(format!(
                "lock entry `{}` has an invalid package root",
                dependency.name
            )));
        }

        if let Some(package_root) = resolved_dependency_root(root, dependency, entry) {
            let canonical_root = match &dependency.source {
                DependencySource::Local(path) | DependencySource::Vendored(path) => {
                    canonical_child_without_symlinks(
                        root,
                        path,
                        &format!("dependency `{}` package root", dependency.name),
                    )?
                }
                DependencySource::Registry(_) | DependencySource::Git(_) => {
                    package_root.canonicalize().map_err(|error| {
                        ProjectError(format!(
                            "could not resolve package root `{}`: {error}",
                            package_root.display()
                        ))
                    })?
                }
            };
            let package_manifest = read_manifest(&canonical_root.join(".zkinit"))?;
            if package_manifest.name != dependency.name {
                return Err(ProjectError(format!(
                    "dependency `{}` resolves to package `{}`",
                    dependency.name, package_manifest.name
                )));
            }
            if package_manifest.version != entry.version {
                return Err(ProjectError(format!(
                    "dependency `{}` declares version `{}` but the lock selects `{}`",
                    dependency.name, package_manifest.version, entry.version
                )));
            }
            if !version_matches(&dependency.constraint, &entry.version)? {
                return Err(ProjectError(format!(
                    "locked version `{}` for dependency `{}` does not satisfy `{}`",
                    entry.version, dependency.name, dependency.constraint
                )));
            }
            let expected_entry = package_manifest.entry.display().to_string();
            if entry.package_root != expected_entry {
                return Err(ProjectError(format!(
                    "lock entry `{}` points at `{}` but the package entry is `{expected_entry}`",
                    dependency.name, entry.package_root
                )));
            }
            let canonical_entry = canonical_child_without_symlinks(
                &canonical_root,
                Path::new(&entry.package_root),
                &format!("dependency `{}` entry", dependency.name),
            )?;
            if !canonical_entry.starts_with(&canonical_root) || !canonical_entry.is_file() {
                return Err(ProjectError(format!(
                    "package `{}` entry escapes its canonical package root",
                    dependency.name
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
            if let Some(expected_integrity) = &dependency.integrity
                && expected_integrity != &entry.hash
            {
                return Err(ProjectError(format!(
                    "lock entry `{}` integrity differs from the manifest locator",
                    dependency.name
                )));
            }
            let actual_hash = package_integrity_hash(&canonical_root, &package_manifest)?;
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
        let canonical_directory = directory.canonicalize().map_err(|error| {
            ProjectError(format!(
                "could not resolve package source `{}`: {error}",
                directory.display()
            ))
        })?;
        if !canonical_directory.starts_with(root) {
            return Err(ProjectError(format!(
                "package source `{}` escapes its canonical package root",
                directory.display()
            )));
        }
        let entries = std::fs::read_dir(directory).map_err(|error| {
            ProjectError(format!(
                "could not read package source `{}`: {error}",
                directory.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| ProjectError(error.to_string()))?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                ProjectError(format!(
                    "could not inspect package path `{}`: {error}",
                    path.display()
                ))
            })?;
            if metadata.file_type().is_symlink() {
                return Err(ProjectError(format!(
                    "package source contains symlink `{}`; package integrity requires regular files",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                collect(&path, root, files)?;
            } else if metadata.is_file() {
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

    let canonical_root = root.canonicalize().map_err(|error| {
        ProjectError(format!(
            "could not resolve package root `{}`: {error}",
            root.display()
        ))
    })?;
    let mut files = Vec::new();
    for source_root in &manifest.source_roots {
        let source_directory =
            canonical_child_without_symlinks(&canonical_root, source_root, "package source root")?;
        collect(&source_directory, &canonical_root, &mut files)?;
    }
    files.sort();
    let mut hasher = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(&canonical_root)
            .unwrap_or(&path)
            .to_string_lossy();
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
    fn canonical_manifest_parses_all_project_sections_with_spans() {
        let manifest = parse_manifest(
            r#"project {
    name: "acme.api";
    version: "1.2.0";
    type: application;
    authors: ["Acme Engineering"];
    description: "HTTP API";
    license: "MIT";
    entry: "src/server.zrk";
}
source "src";
source "generated";
build_targets { "x86_64-linux"; "aarch64-macos"; }
dependencies {
    acme.http: "^2.1";
    acme.shared: { from: "path:../shared"; }
    acme.crypto: { from: "git+https://code.example/acme/crypto"; rev: "v1.4.2"; }
    acme.private: { from: "registry:company"; version: "^3.0"; }
}
dev_dependencies { acme.testkit: "^0.8"; }
globals {
    request_count: UInt64;
    settings: { type: Settings; from: "src/config.zrk"; readonly: true; }
    retry_limit: { type: UInt32; default: 0; }
}
requires { filesystem.read; network.listen; }
"#,
        )
        .unwrap();

        assert_eq!(manifest.name, "acme.api");
        assert_eq!(manifest.version, "1.2.0");
        assert_eq!(manifest.entry, PathBuf::from("src/server.zrk"));
        assert_eq!(
            manifest.source_roots,
            [PathBuf::from("src"), PathBuf::from("generated")]
        );
        assert_eq!(manifest.build_targets, ["x86_64-linux", "aarch64-macos"]);
        assert_eq!(manifest.dependencies["acme.http"].constraint, "^2.1");
        assert_eq!(
            manifest.dependencies["acme.shared"].source,
            DependencySource::Local("../shared".into())
        );
        assert_eq!(
            manifest.dependencies["acme.crypto"].revision.as_deref(),
            Some("v1.4.2")
        );
        assert!(manifest.dev_dependencies.contains_key("acme.testkit"));
        assert_eq!(manifest.globals["settings"].type_name, "Settings");
        assert!(manifest.globals["settings"].readonly);
        assert_eq!(
            manifest.globals["retry_limit"].default.as_deref(),
            Some("0")
        );
        assert!(manifest.requires.contains(&Capability::FilesystemRead));
        assert!(manifest.requires.contains(&Capability::NetworkListen));
        assert!(manifest.spans.fields.contains_key("project.entry"));
        assert!(manifest.spans.dependencies.contains_key("acme.crypto"));
        assert!(
            manifest
                .spans
                .fields
                .contains_key("dependencies.acme.crypto.rev")
        );
        assert!(manifest.spans.globals.contains_key("settings"));
        assert!(manifest.spans.fields.contains_key("globals.settings.from"));
        assert!(
            manifest
                .spans
                .requirements
                .contains_key(&Capability::NetworkListen)
        );
    }

    #[test]
    fn canonical_manifest_formatting_is_idempotent_and_keeps_ordered_targets() {
        let manifest = parse_manifest(
            r#"project { name: "acme.app"; version: "0.1.0"; type: application; }
build_targets { "aarch64-macos"; "x86_64-linux"; }
dependencies { acme.lib: "^1.0"; }
requires { network.listen; filesystem.read; }
"#,
        )
        .unwrap();
        let formatted = format_manifest(&manifest);
        let reparsed = parse_manifest(&formatted).unwrap();
        assert_eq!(reparsed.name, manifest.name);
        assert_eq!(reparsed.version, manifest.version);
        assert_eq!(reparsed.kind, manifest.kind);
        assert_eq!(reparsed.entry, manifest.entry);
        assert_eq!(reparsed.source_roots, manifest.source_roots);
        assert_eq!(reparsed.build_targets, manifest.build_targets);
        assert_eq!(reparsed.dependencies, manifest.dependencies);
        assert_eq!(reparsed.requires, manifest.requires);
        assert_eq!(format_manifest(&reparsed), formatted);
    }

    #[test]
    fn canonical_manifest_reports_unknown_capability_and_legacy_permission_syntax() {
        let unknown = "project { name: \"a\"; version: \"0.1.0\"; type: application; }\nrequires { filesystem.read; camera.open; }";
        let error = parse_manifest(unknown).unwrap_err().to_string();
        assert!(error.contains("unknown capability `camera.open`"));
        assert!(error.contains("at 2:"));
        let legacy =
            "project { name: \"a\"; version: \"0.1.0\"; type: application; }\npermissions {}";
        let error = parse_manifest(legacy).unwrap_err().to_string();
        assert!(error.contains("legacy permission syntax was removed"));
        assert!(error.contains("requires"));
    }

    #[test]
    fn canonical_manifest_reports_type_and_license_fields_at_their_locations() {
        let wrong_type =
            parse_manifest("project {\n name: 42;\n version: \"0.1.0\";\n type: application;\n }")
                .unwrap_err()
                .to_string();
        assert!(wrong_type.contains("`project.name` must be a string"));
        assert!(wrong_type.contains("2:2"));

        let invalid_license = parse_manifest(
            "project {\n name: \"app\";\n version: \"0.1.0\";\n type: application;\n license: \"MIT OR\";\n }",
        )
        .unwrap_err()
        .to_string();
        assert!(invalid_license.contains("project.license"));
        assert!(invalid_license.contains("5:2"));
    }

    #[test]
    fn canonical_manifest_applies_project_type_defaults_and_rejects_incompatible_fields() {
        let application =
            parse_manifest("project { name: \"app\"; version: \"0.1.0\"; type: application; }")
                .unwrap();
        assert_eq!(application.entry, PathBuf::from("src/main.zrk"));
        assert_eq!(application.source_roots, [PathBuf::from("src")]);

        let package =
            parse_manifest("project { name: \"lib\"; version: \"0.1.0\"; type: package; }")
                .unwrap();
        assert_eq!(package.entry, PathBuf::from("src/package.zrk"));
        assert_eq!(package.package_root, Some(PathBuf::from("src/package.zrk")));

        let error = parse_manifest(
            "project { name: \"lib\"; version: \"0.1.0\"; type: package; entry: \"src/main.zrk\"; }",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("project.entry"));
        assert!(error.contains("only valid for an application"));

        let error = parse_manifest(
            "project { name: \"lib\"; version: \"0.1.0\"; type: package; }\nbuild_targets { \"x86_64-linux\"; }",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("build_targets"));
        assert!(error.contains("only valid for applications"));

        let error = parse_manifest(
            "project { name: \"app\"; version: \"0.1.0\"; type: application; entry: \"../outside.zrk\"; }",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("project.entry"));
        assert!(error.contains("project-relative path"));

        let error = parse_manifest(
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\nbuild_targets { \"wasm32-surprise\"; }",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("unsupported build target"));
    }

    #[test]
    fn canonical_dependency_errors_point_to_the_offending_field() {
        let error = parse_manifest(
            "project { name: \"a\"; version: \"0.1.0\"; type: application; }\ndependencies {\n dep: { from: \"git+https://example.test/dep\"; version: \"^1\"; }\n}",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("Git locator requires an immutable `dependency.rev`"));
        assert!(error.contains("dependency.from"));

        let error = parse_manifest(
            "project { name: \"a\"; version: \"0.1.0\"; type: application; }\ndependencies {\n dep: { from: 42; }\n}",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("dependency.from` must be a string"));
        assert!(error.contains("at 3:"), "{error}");
    }

    #[test]
    fn legacy_manifest_migration_converts_grants_to_requirements_only() {
        let legacy = r#"project {
    name: "legacy-app";
    version: "2.3.0";
    type: application;
    entry: "src/start.zrk";
}
permissions {
    environment {
        read: { names: ["APP_MODE"]; during: runtime; }
        secrets: { names: ["DB_PASSWORD"]; during: runtime; }
    }
    network {
        listen: { addresses: ["127.0.0.1:8080"]; during: runtime; }
    }
}"#;
        let canonical = migrate_legacy_manifest_text(legacy).unwrap();
        let migrated = parse_manifest(&canonical).unwrap();
        assert_eq!(migrated.version, "2.3.0");
        assert_eq!(migrated.entry, PathBuf::from("src/start.zrk"));
        assert!(migrated.requires.contains(&Capability::EnvironmentRead));
        assert!(migrated.requires.contains(&Capability::SecretsRead));
        assert!(migrated.requires.contains(&Capability::NetworkListen));
        assert!(migrated.environment_permissions.read_names.is_empty());
        assert!(migrated.network_permissions.listen_addresses.is_empty());
        assert!(!canonical.contains("during:"));
    }

    #[test]
    fn legacy_package_entry_migrates_to_root_and_unpinned_git_is_rejected() {
        let package = r#"project {
    name: "acme.old";
    version: "1.0.0";
    type: package;
    entry: "src/api.zrk";
}"#;
        let canonical = migrate_legacy_manifest_text(package).unwrap();
        assert!(canonical.contains("root: \"src/api.zrk\";"));
        assert!(!canonical.contains("entry:"));

        let unpinned_git = "project \"app\"\nkind application\ndependency acme.git ^1 git \"https://example.test/acme.git\"";
        let error = migrate_legacy_manifest_text(unpinned_git).unwrap_err();
        assert!(error.to_string().contains("immutable revision"));
    }

    #[test]
    fn explicit_manifest_migration_apply_keeps_a_recoverable_original() {
        let root = std::env::temp_dir().join(format!("zirk-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(".zkinit");
        let original = "project \"migrate.app\"\nkind application\n";
        std::fs::write(&path, original).unwrap();

        let (preview_original, preview) = migrate_manifest_file(&path, false).unwrap();
        assert_eq!(preview_original, original);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert!(parse_manifest(&preview).is_ok());

        migrate_manifest_file(&path, true).unwrap();
        assert!(parse_manifest(&std::fs::read_to_string(&path).unwrap()).is_ok());
        assert_eq!(
            std::fs::read_to_string(root.join(".zkinit.pre-migrate")).unwrap(),
            original
        );
        assert!(migrate_manifest_file(&path, true).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn manifest_models_package_identity_roots_and_dependencies() {
        let manifest = parse_legacy_manifest("project \"acme.cli\"\nkind package\nsource \"src\"\ndependency acme.web ^1.0 local \"../web\"").unwrap();
        assert_eq!(manifest.kind, ProjectKind::Package);
        assert_eq!(manifest.entry, PathBuf::from("src/package.zrk"));
        assert!(manifest.dependencies.contains_key("acme.web"));
        assert!(manifest.environment_permissions.read_names.is_empty());
    }

    #[test]
    fn manifest_parses_documented_project_block_and_permissions() {
        let manifest = parse_legacy_manifest(
            r#"project {
    name: "demo-http";
    version: "0.1.0";
    type: application;
    entry: "main.zrk";
}
permissions {
    network {
        listen: { addresses: ["127.0.0.1:8081"]; during: runtime; }
    }
}"#,
        )
        .unwrap();

        assert_eq!(manifest.name, "demo-http");
        assert_eq!(manifest.kind, ProjectKind::Application);
        assert_eq!(manifest.entry, PathBuf::from("main.zrk"));
        assert_eq!(
            manifest.network_permissions.listen_addresses,
            ["127.0.0.1:8081"].into_iter().map(str::to_owned).collect()
        );
    }

    #[test]
    fn manifest_parses_exact_environment_requirements_without_granting_them() {
        let manifest = parse_legacy_manifest(
            r#"project "acme.cli"
kind application
permissions {
environment {
read: { names: ["APP_MODE", "LOG_LEVEL"]; during: runtime; }
secrets: { names: ["DB_PASSWORD"]; during: runtime; }
list_names: { during: runtime; }
}
}"#,
        )
        .unwrap();
        assert_eq!(
            manifest.environment_permissions.read_names,
            ["APP_MODE", "LOG_LEVEL"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.environment_permissions.secret_names,
            ["DB_PASSWORD"].into_iter().map(str::to_owned).collect()
        );
        assert!(manifest.environment_permissions.list_names);
    }

    #[test]
    fn manifest_cannot_request_runtime_internal_environment_names() {
        let manifest = "project \"app\"\nkind application\npermissions {\nenvironment {\nread: { names: [\"zirk_runtime_permission_grant\"]; during: runtime; }\n}\n}\n";
        assert!(parse_legacy_manifest(manifest).is_err());
    }

    #[test]
    fn manifest_parses_exact_network_origins_and_listen_addresses() {
        let manifest = parse_legacy_manifest(
            r#"project "acme.server"
kind application
permissions {
network {
connect: { origins: ["https://api.example.com:443", "tcp://cache.example:9000"]; during: runtime; }
listen: { addresses: ["127.0.0.1:8080", "[::1]:8443"]; during: runtime; }
}
}"#,
        )
        .unwrap();
        assert_eq!(
            manifest.network_permissions.connect_origins,
            ["https://api.example.com:443", "tcp://cache.example:9000"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.network_permissions.listen_addresses,
            ["127.0.0.1:8080", "[::1]:8443"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
    }

    #[test]
    fn manifest_parses_transport_specific_network_authority() {
        let manifest = parse_legacy_manifest(
            r#"project "network.app"
kind application
permissions {
network {
udp_send: { destinations: ["udp://192.0.2.9:5353", "udp://[2001:db8::1]:53"]; during: runtime; }
udp_listen: { addresses: ["127.0.0.1:5353"]; during: runtime; }
udp_broadcast: { destinations: ["udp://255.255.255.255:9000"]; during: runtime; }
udp_multicast: { groups: ["239.1.2.3", "ff02::1"]; during: runtime; }
local_connect: { endpoints: ["unix:///tmp/zirk.sock", "npipe://./pipe/zirk"]; during: runtime; }
local_listen: { endpoints: ["unix:///tmp/zirk-server.sock"]; during: runtime; }
inspect_interfaces: { enabled: true; during: runtime; }
}
}"#,
        )
        .unwrap();
        assert_eq!(
            manifest.network_permissions.udp_destinations,
            ["udp://192.0.2.9:5353", "udp://[2001:db8::1]:53"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.network_permissions.udp_listen_addresses,
            ["127.0.0.1:5353"].into_iter().map(str::to_owned).collect()
        );
        assert_eq!(
            manifest.network_permissions.udp_broadcast_destinations,
            ["udp://255.255.255.255:9000"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.network_permissions.udp_multicast_groups,
            ["239.1.2.3", "ff02::1"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.network_permissions.local_connect_endpoints,
            ["unix:///tmp/zirk.sock", "npipe://./pipe/zirk"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            manifest.network_permissions.local_listen_endpoints,
            ["unix:///tmp/zirk-server.sock"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert!(manifest.network_permissions.inspect_interfaces);
    }

    #[test]
    fn manifest_rejects_noncanonical_udp_and_traversing_local_scopes() {
        for permission in [
            r#"udp_send: { destinations: ["udp://example.com:53"]; during: runtime; }"#,
            r#"udp_send: { destinations: ["udp://192.0.2.9:0"]; during: runtime; }"#,
            r#"udp_broadcast: { destinations: ["udp://192.0.2.1:9000"]; during: runtime; }"#,
            r#"udp_multicast: { groups: ["192.0.2.1"]; during: runtime; }"#,
            r#"udp_multicast: { groups: ["239.001.002.003"]; during: runtime; }"#,
            r#"local_connect: { endpoints: ["unix:///tmp/../etc/passwd"]; during: runtime; }"#,
            r#"local_listen: { endpoints: ["npipe://./pipe/name/other"]; during: runtime; }"#,
        ] {
            let manifest = format!(
                "project \"app\"\nkind application\npermissions {{\nnetwork {{\n{permission}\n}}\n}}"
            );
            assert!(parse_manifest(&manifest).is_err(), "accepted {permission}");
        }
    }

    #[test]
    fn manifest_rejects_ambiguous_network_scopes() {
        for declaration in [
            r#"connect: { origins: ["https://api.example.com"]; during: runtime; }"#,
            r#"connect: { origins: ["https://user:pass@example.com:443"]; during: runtime; }"#,
            r#"connect: { origins: ["https://api.example.com:443/admin"]; during: runtime; }"#,
            r#"connect: { origins: ["tcp://cache.example"]; during: runtime; }"#,
            r#"listen: { addresses: ["localhost:8080"]; during: runtime; }"#,
            r#"listen: { addresses: ["127.0.0.1:8080"]; during: build; }"#,
        ] {
            let text = format!(
                "project \"app\"\nkind application\npermissions {{\nnetwork {{\n{declaration}\n}}\n}}"
            );
            assert!(parse_manifest(&text).is_err(), "accepted {declaration}");
        }
    }

    #[test]
    fn manifest_rejects_unsafe_environment_permission_declarations() {
        for declaration in [
            r#"read: { names: ["*" ]; during: runtime; }"#,
            r#"read: { names: ["APP_MODE"]; during: build; }"#,
            r#"write: { names: ["APP_MODE"]; during: runtime; }"#,
            r#"read: { names: ["APP_MODE"]; junk: true; during: runtime; }"#,
        ] {
            let text = format!(
                "project \"app\"\nkind application\npermissions {{\nenvironment {{\n{declaration}\n}}\n}}"
            );
            assert!(parse_manifest(&text).is_err(), "accepted {declaration}");
        }
        assert!(
            parse_manifest("project \"app\"\nkind application\npermissions {\nenvironment {")
                .is_err()
        );
    }
    #[test]
    fn malformed_lock_integrity_is_rejected_deterministically() {
        assert!(parse_lockfile("acme.web 1.0 local - src/package.zrk").is_err());
        assert!(parse_lockfile("acme.web 1.0 local sha256:ok src/package.zrk").is_ok());
    }

    #[test]
    fn canonical_lockfile_roundtrips_locator_revision_integrity_and_root() {
        let package = LockedPackage {
            name: "acme.crypto".into(),
            version: "1.4.2".into(),
            source: "git".into(),
            locator: "git+https://code.example/acme/crypto".into(),
            revision: Some("0123456789abcdef0123456789abcdef01234567".into()),
            hash: format!("sha256:{}", "a".repeat(64)),
            package_root: "src/package.zrk".into(),
        };
        let lock = Lockfile {
            packages: [(package.name.clone(), package)].into(),
        };
        let encoded = format_lockfile(&lock);
        let decoded = parse_lockfile(&encoded).unwrap();
        assert_eq!(decoded, lock);
        assert!(encoded.contains("\"lock_version\": 2"));
    }

    #[test]
    fn lockfile_must_cover_each_dependency_with_the_declared_transport() {
        let manifest = parse_legacy_manifest(
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
        let staging = root.join("staging-package");
        std::fs::create_dir_all(staging.join("src")).unwrap();
        std::fs::write(
            root.join(".zkinit"),
            "project { name: \"app\"; version: \"0.1.0\"; type: application; }\ndependencies { acme.web: { from: \"registry:default\"; version: \"^1\"; } }",
        )
        .unwrap();
        std::fs::write(
            staging.join(".zkinit"),
            "project { name: \"acme.web\"; version: \"1.0.0\"; type: package; }",
        )
        .unwrap();
        std::fs::write(staging.join("src/package.zrk"), "package acme.web;").unwrap();
        let dependency_manifest = read_manifest(&staging.join(".zkinit")).unwrap();
        let hash = package_integrity_hash(&staging, &dependency_manifest).unwrap();
        let cached = root.join(".zirk/packages/acme.web/1.0.0").join(&hash);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::rename(&staging, &cached).unwrap();
        let manifest = read_manifest(&root.join(".zkinit")).unwrap();
        let package = LockedPackage {
            name: "acme.web".into(),
            version: "1.0.0".into(),
            source: "registry".into(),
            locator: "registry:default".into(),
            revision: None,
            hash: format!("sha256:{hash}"),
            package_root: "src/package.zrk".into(),
        };
        let lock = Lockfile {
            packages: [(package.name.clone(), package)].into(),
        };
        validate_lockfile(&root, &manifest, Some(&lock)).unwrap();
        let roots = resolve_package_roots(&root, &manifest, &lock).unwrap();
        assert_eq!(
            roots["acme.web"],
            cached.join("src/package.zrk").canonicalize().unwrap()
        );
    }

    #[test]
    fn local_package_cycles_report_the_complete_path() {
        let root = std::env::temp_dir().join(format!("zirk-package-cycle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(
            root.join(".zkinit"),
            "project { name: \"a\"; version: \"1.0.0\"; type: package; }\ndependencies { b: { from: \"path:b\"; version: \"^1\"; } }",
        )
        .unwrap();
        std::fs::write(
            root.join("b/.zkinit"),
            "project { name: \"b\"; version: \"1.0.0\"; type: package; }\ndependencies { a: { from: \"path:..\"; version: \"^1\"; } }",
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
        let manifest = parse_legacy_manifest("project \"acme.api\"\nkind package").unwrap();
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
