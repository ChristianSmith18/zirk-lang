//! Runtime verification of launcher-issued project permission grants.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use zirk_runtime_core::network_policy::{EffectiveNetworkGrant, NetworkPolicyError};
use zirk_runtime_core::permission_grant::{SignedPermissionGrant, VerifiedPermissionGrant};

const RUNTIME_GRANT_ENV: &str = "ZIRK_RUNTIME_PERMISSION_GRANT";
static VERIFIED_GRANT: OnceLock<Option<VerifiedPermissionGrant>> = OnceLock::new();

/// Returns a project grant only when the signed token from the trusted
/// launcher verifies with the user-owned approval key. Missing, malformed,
/// or stale tokens all mean no authority.
pub fn verified_grant() -> Option<&'static VerifiedPermissionGrant> {
    VERIFIED_GRANT.get_or_init(load_verified_grant).as_ref()
}

/// Builds the process network policy only from a grant verified from the
/// launcher's signed project approval.
pub fn current_network_grant() -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    let Some(verified) = verified_grant() else {
        return Err(NetworkPolicyError::PermissionDenied);
    };
    network_grant_from_verified(verified)
}

fn network_grant_from_verified(
    verified: &VerifiedPermissionGrant,
) -> Result<Arc<EffectiveNetworkGrant>, NetworkPolicyError> {
    let scopes = verified.scopes();
    EffectiveNetworkGrant::from_verified_scopes(
        scopes.network_connect.iter().cloned(),
        scopes.network_listen.iter().cloned(),
    )
    .map(Arc::new)
}

fn load_verified_grant() -> Option<VerifiedPermissionGrant> {
    let encoded = std::env::var(RUNTIME_GRANT_ENV).ok()?;
    let key = read_approval_key(&approval_key_path()?).ok()?;
    SignedPermissionGrant::decode(&encoded)
        .ok()?
        .verify(&key)
        .ok()
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
    }
}
