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
    let mut environment_permissions = EnvironmentPermissions::default();
    let mut network_permissions = NetworkPermissions::default();
    let mut permission_block: Option<&str> = None;
    let mut environment_block = false;
    let mut network_block = false;
    for raw in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
    {
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
    if permission_block.is_some() || environment_block || network_block {
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
    Ok(ProjectManifest {
        name,
        kind,
        entry,
        source_roots,
        dependencies,
        environment_permissions,
        network_permissions,
    })
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
        assert!(manifest.environment_permissions.read_names.is_empty());
    }

    #[test]
    fn manifest_parses_exact_environment_requirements_without_granting_them() {
        let manifest = parse_manifest(
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
        assert!(parse_manifest(manifest).is_err());
    }

    #[test]
    fn manifest_parses_exact_network_origins_and_listen_addresses() {
        let manifest = parse_manifest(
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
        let manifest = parse_manifest(
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
