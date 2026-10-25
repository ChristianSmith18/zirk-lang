//! Consent records shared by the trusted launcher and native runtime.
//!
//! A manifest only requests authority. The launcher signs an exact, project-
//! bound scope after the user approves it; the runtime verifies that record
//! before constructing any effective environment or network grant.

use std::collections::BTreeSet;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const GRANT_SCHEMA_VERSION: u32 = 4;
const HISTORY_SCHEMA_VERSION: u32 = 1;
const MAX_GRANT_BYTES: usize = 64 * 1024;
const MAX_HISTORY_EVENT_BYTES: usize = 16 * 1024;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PermissionScopes {
    #[serde(default)]
    pub filesystem_read: BTreeSet<String>,
    #[serde(default)]
    pub filesystem_write: BTreeSet<String>,
    // Empty scopes are omitted so existing schema-v4 grants keep the same
    // signed representation until metadata-only consent is actually saved.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub filesystem_metadata: BTreeSet<String>,
    /// Operation-specific filesystem approvals added after the exact-path
    /// core contract. Keeping them separate ensures legacy exact file grants
    /// cannot silently become directory, watcher, link, or lock authority.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub filesystem_operations: BTreeSet<FilesystemOperationScope>,
    pub environment_read: BTreeSet<String>,
    pub environment_secrets: BTreeSet<String>,
    pub environment_list_names: bool,
    pub network_connect: BTreeSet<String>,
    pub network_listen: BTreeSet<String>,
    #[serde(default)]
    pub network_udp_destinations: BTreeSet<String>,
    #[serde(default)]
    pub network_udp_listen: BTreeSet<String>,
    #[serde(default)]
    pub network_udp_broadcast_destinations: BTreeSet<String>,
    #[serde(default)]
    pub network_udp_multicast_groups: BTreeSet<String>,
    #[serde(default)]
    pub network_local_connect: BTreeSet<String>,
    #[serde(default)]
    pub network_local_listen: BTreeSet<String>,
    #[serde(default)]
    pub network_inspect_interfaces: bool,
    #[serde(default)]
    pub process_execute: BTreeSet<String>,
    #[serde(default)]
    pub process_shell: BTreeSet<String>,
}

/// Extent recorded for an operation-specific filesystem approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemScopeExtent {
    /// A single file or other filesystem object.
    Exact,
    /// An operation on the directory object itself, such as listing entries.
    Directory,
    /// The named directory and its descendants for one approved operation.
    Descendants,
}

/// Authority for a specific filesystem operation, independent of ordinary
/// exact-path read/write grants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FilesystemOperationScope {
    pub capability: String,
    pub operation: String,
    pub requester: String,
    pub resource: String,
    pub extent: FilesystemScopeExtent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPermissionGrant {
    schema_version: u32,
    project_id: String,
    manifest_digest: String,
    scopes: PermissionScopes,
    mac: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPermissionGrant {
    project_id: String,
    manifest_digest: String,
    scopes: PermissionScopes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionHistoryAction {
    Approved,
    Revoked,
}

/// One user-owned, authenticated permission decision for the project history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPermissionHistoryEvent {
    schema_version: u32,
    project_id: String,
    manifest_digest: String,
    timestamp_unix_ms: u128,
    action: PermissionHistoryAction,
    capability: String,
    resource: String,
    requester: String,
    actor: String,
    mac: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPermissionHistoryEvent {
    project_id: String,
    manifest_digest: String,
    timestamp_unix_ms: u128,
    action: PermissionHistoryAction,
    capability: String,
    resource: String,
    requester: String,
    actor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantVerificationError {
    TooLarge,
    Malformed,
    UnsupportedVersion,
    InvalidMac,
    InvalidIdentity,
}

impl SignedPermissionGrant {
    pub fn sign(
        project_id: impl Into<String>,
        manifest_digest: impl Into<String>,
        scopes: PermissionScopes,
        key: &[u8],
    ) -> Result<Self, GrantVerificationError> {
        let project_id = project_id.into();
        let manifest_digest = manifest_digest.into();
        validate_identity(&project_id, &manifest_digest)?;
        let mut grant = Self {
            schema_version: GRANT_SCHEMA_VERSION,
            project_id,
            manifest_digest,
            scopes,
            mac: String::new(),
        };
        grant.mac = hex(&compute_mac(key, &grant.signing_bytes()?)?);
        Ok(grant)
    }

    pub fn verify(self, key: &[u8]) -> Result<VerifiedPermissionGrant, GrantVerificationError> {
        if self.schema_version != GRANT_SCHEMA_VERSION {
            return Err(GrantVerificationError::UnsupportedVersion);
        }
        validate_identity(&self.project_id, &self.manifest_digest)?;
        let provided = decode_hex(&self.mac).ok_or(GrantVerificationError::InvalidMac)?;
        let mut verifier =
            HmacSha256::new_from_slice(key).map_err(|_| GrantVerificationError::InvalidMac)?;
        verifier.update(&self.signing_bytes()?);
        verifier
            .verify_slice(&provided)
            .map_err(|_| GrantVerificationError::InvalidMac)?;
        Ok(VerifiedPermissionGrant {
            project_id: self.project_id,
            manifest_digest: self.manifest_digest,
            scopes: self.scopes,
        })
    }

    pub fn encode(&self) -> Result<String, GrantVerificationError> {
        let encoded = serde_json::to_string(self).map_err(|_| GrantVerificationError::Malformed)?;
        if encoded.len() > MAX_GRANT_BYTES {
            return Err(GrantVerificationError::TooLarge);
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &str) -> Result<Self, GrantVerificationError> {
        if encoded.len() > MAX_GRANT_BYTES {
            return Err(GrantVerificationError::TooLarge);
        }
        serde_json::from_str(encoded).map_err(|_| GrantVerificationError::Malformed)
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn scopes(&self) -> &PermissionScopes {
        &self.scopes
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, GrantVerificationError> {
        serde_json::to_vec(&(
            self.schema_version,
            &self.project_id,
            &self.manifest_digest,
            &self.scopes,
        ))
        .map_err(|_| GrantVerificationError::Malformed)
    }
}

impl VerifiedPermissionGrant {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn scopes(&self) -> &PermissionScopes {
        &self.scopes
    }
}

impl SignedPermissionHistoryEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        project_id: impl Into<String>,
        manifest_digest: impl Into<String>,
        timestamp_unix_ms: u128,
        action: PermissionHistoryAction,
        capability: impl Into<String>,
        resource: impl Into<String>,
        requester: impl Into<String>,
        actor: impl Into<String>,
        key: &[u8],
    ) -> Result<Self, GrantVerificationError> {
        let mut event = Self {
            schema_version: HISTORY_SCHEMA_VERSION,
            project_id: project_id.into(),
            manifest_digest: manifest_digest.into(),
            timestamp_unix_ms,
            action,
            capability: capability.into(),
            resource: resource.into(),
            requester: requester.into(),
            actor: actor.into(),
            mac: String::new(),
        };
        validate_identity(&event.project_id, &event.manifest_digest)?;
        if event.capability.is_empty() || event.resource.is_empty() || event.requester.is_empty() {
            return Err(GrantVerificationError::InvalidIdentity);
        }
        event.mac = hex(&compute_mac(key, &event.signing_bytes()?)?);
        Ok(event)
    }

    pub fn verify(
        self,
        key: &[u8],
    ) -> Result<VerifiedPermissionHistoryEvent, GrantVerificationError> {
        if self.schema_version != HISTORY_SCHEMA_VERSION {
            return Err(GrantVerificationError::UnsupportedVersion);
        }
        validate_identity(&self.project_id, &self.manifest_digest)?;
        let provided = decode_hex(&self.mac).ok_or(GrantVerificationError::InvalidMac)?;
        let mut verifier =
            HmacSha256::new_from_slice(key).map_err(|_| GrantVerificationError::InvalidMac)?;
        verifier.update(&self.signing_bytes()?);
        verifier
            .verify_slice(&provided)
            .map_err(|_| GrantVerificationError::InvalidMac)?;
        Ok(VerifiedPermissionHistoryEvent {
            project_id: self.project_id,
            manifest_digest: self.manifest_digest,
            timestamp_unix_ms: self.timestamp_unix_ms,
            action: self.action,
            capability: self.capability,
            resource: self.resource,
            requester: self.requester,
            actor: self.actor,
        })
    }

    pub fn encode(&self) -> Result<String, GrantVerificationError> {
        let encoded = serde_json::to_string(self).map_err(|_| GrantVerificationError::Malformed)?;
        if encoded.len() > MAX_HISTORY_EVENT_BYTES {
            return Err(GrantVerificationError::TooLarge);
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &str) -> Result<Self, GrantVerificationError> {
        if encoded.len() > MAX_HISTORY_EVENT_BYTES {
            return Err(GrantVerificationError::TooLarge);
        }
        serde_json::from_str(encoded).map_err(|_| GrantVerificationError::Malformed)
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, GrantVerificationError> {
        serde_json::to_vec(&(
            self.schema_version,
            &self.project_id,
            &self.manifest_digest,
            self.timestamp_unix_ms,
            self.action,
            &self.capability,
            &self.resource,
            &self.requester,
            &self.actor,
        ))
        .map_err(|_| GrantVerificationError::Malformed)
    }
}

impl VerifiedPermissionHistoryEvent {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }
    pub fn timestamp_unix_ms(&self) -> u128 {
        self.timestamp_unix_ms
    }
    pub fn action(&self) -> PermissionHistoryAction {
        self.action
    }
    pub fn capability(&self) -> &str {
        &self.capability
    }
    pub fn resource(&self) -> &str {
        &self.resource
    }
    pub fn requester(&self) -> &str {
        &self.requester
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

fn compute_mac(key: &[u8], message: &[u8]) -> Result<Vec<u8>, GrantVerificationError> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| GrantVerificationError::InvalidMac)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn validate_identity(
    project_id: &str,
    manifest_digest: &str,
) -> Result<(), GrantVerificationError> {
    if project_id.len() != 64
        || !project_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || manifest_digest.len() != 64
        || !manifest_digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GrantVerificationError::InvalidIdentity);
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Some((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?))
        .collect()
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes() -> PermissionScopes {
        PermissionScopes {
            filesystem_read: BTreeSet::new(),
            filesystem_write: BTreeSet::new(),
            filesystem_metadata: BTreeSet::new(),
            filesystem_operations: BTreeSet::new(),
            environment_read: ["APP_MODE".into()].into(),
            environment_secrets: ["DB_PASSWORD".into()].into(),
            environment_list_names: false,
            network_connect: ["https://api.example:443".into()].into(),
            network_listen: BTreeSet::new(),
            network_udp_destinations: ["udp://192.0.2.9:53".into()].into(),
            network_udp_listen: ["127.0.0.1:5353".into()].into(),
            network_udp_broadcast_destinations: ["udp://255.255.255.255:9000".into()].into(),
            network_udp_multicast_groups: ["239.1.2.3".into()].into(),
            network_local_connect: ["unix:///tmp/zirk.sock".into()].into(),
            network_local_listen: ["npipe://./pipe/zirk".into()].into(),
            network_inspect_interfaces: true,
            process_execute: BTreeSet::new(),
            process_shell: BTreeSet::new(),
        }
    }

    #[test]
    fn signed_scope_round_trips_and_is_project_bound() {
        let signed =
            SignedPermissionGrant::sign("a".repeat(64), "b".repeat(64), scopes(), b"test-key")
                .unwrap();
        let encoded = signed.encode().unwrap();
        assert!(
            !encoded.contains("filesystem_metadata"),
            "empty metadata scopes preserve the legacy signed representation"
        );
        assert!(
            !encoded.contains("filesystem_operations"),
            "empty operation scopes preserve the legacy signed representation"
        );
        let verified = SignedPermissionGrant::decode(&encoded)
            .unwrap()
            .verify(b"test-key")
            .unwrap();
        assert_eq!(verified.project_id(), "a".repeat(64));
        assert_eq!(verified.manifest_digest(), "b".repeat(64));
        assert_eq!(verified.scopes(), &scopes());
        assert_eq!(
            SignedPermissionGrant::decode(&encoded)
                .unwrap()
                .verify(b"different-key"),
            Err(GrantVerificationError::InvalidMac)
        );
    }

    #[test]
    fn changing_any_scope_invalidates_the_signature() {
        let signed =
            SignedPermissionGrant::sign("a".repeat(64), "b".repeat(64), scopes(), b"test-key")
                .unwrap();
        let mut value = serde_json::to_value(signed).unwrap();
        value["scopes"]["environment_list_names"] = serde_json::Value::Bool(true);
        let tampered = serde_json::to_string(&value).unwrap();
        assert_eq!(
            SignedPermissionGrant::decode(&tampered)
                .unwrap()
                .verify(b"test-key"),
            Err(GrantVerificationError::InvalidMac)
        );
    }

    #[test]
    fn metadata_only_scope_round_trips_as_signed_authority() {
        let scopes = PermissionScopes {
            filesystem_metadata: ["/tmp/metadata-only.txt".to_owned()].into(),
            ..scopes()
        };
        let signed = SignedPermissionGrant::sign(
            "a".repeat(64),
            "b".repeat(64),
            scopes.clone(),
            b"test-key",
        )
        .unwrap();
        let encoded = signed.encode().unwrap();
        assert!(encoded.contains("filesystem_metadata"));
        let verified = SignedPermissionGrant::decode(&encoded)
            .unwrap()
            .verify(b"test-key")
            .unwrap();
        assert_eq!(verified.scopes(), &scopes);

        let mut value = serde_json::to_value(signed).unwrap();
        value["scopes"]["filesystem_metadata"][0] =
            serde_json::Value::String("/tmp/other.txt".to_owned());
        let tampered = serde_json::to_string(&value).unwrap();
        assert_eq!(
            SignedPermissionGrant::decode(&tampered)
                .unwrap()
                .verify(b"test-key"),
            Err(GrantVerificationError::InvalidMac)
        );
    }

    #[test]
    fn operation_scoped_directory_approval_round_trips_with_provenance_and_extent() {
        let scope = FilesystemOperationScope {
            capability: "filesystem.read".into(),
            operation: "directory.walk".into(),
            requester: "example.parser".into(),
            resource: "/tmp/project/data".into(),
            extent: FilesystemScopeExtent::Descendants,
        };
        let scopes = PermissionScopes {
            filesystem_operations: [scope.clone()].into(),
            ..PermissionScopes::default()
        };
        let signed = SignedPermissionGrant::sign(
            "c".repeat(64),
            "d".repeat(64),
            scopes,
            b"operation-scope-key",
        )
        .unwrap();
        let verified = SignedPermissionGrant::decode(&signed.encode().unwrap())
            .unwrap()
            .verify(&"c".repeat(64), &"d".repeat(64), b"operation-scope-key")
            .unwrap();
        assert!(verified.scopes().filesystem_operations.contains(&scope));
    }

    #[test]
    fn decoding_rejects_unknown_fields_and_oversized_tokens() {
        assert_eq!(
            SignedPermissionGrant::decode(&" ".repeat(MAX_GRANT_BYTES + 1)),
            Err(GrantVerificationError::TooLarge)
        );
        assert_eq!(
            SignedPermissionGrant::decode(r#"{"schema_version":1,"unknown":true}"#),
            Err(GrantVerificationError::Malformed)
        );
    }

    #[test]
    fn signed_history_event_round_trips_and_detects_tampering() {
        let encoded = SignedPermissionHistoryEvent::sign(
            "a".repeat(64),
            "b".repeat(64),
            1_798_000_000_123,
            PermissionHistoryAction::Approved,
            "network.connect",
            "https://api.example:443",
            "acme.http 1.0.0 via app -> acme.http",
            "cristian",
            b"test-key",
        )
        .unwrap()
        .encode()
        .unwrap();
        let verified = SignedPermissionHistoryEvent::decode(&encoded)
            .unwrap()
            .verify(b"test-key")
            .unwrap();
        assert_eq!(verified.capability(), "network.connect");
        assert_eq!(verified.resource(), "https://api.example:443");
        assert_eq!(verified.requester(), "acme.http 1.0.0 via app -> acme.http");
        assert_eq!(verified.timestamp_unix_ms(), 1_798_000_000_123);

        let mut tampered: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        tampered["resource"] = serde_json::Value::String("https://other.example:443".into());
        let tampered = serde_json::to_string(&tampered).unwrap();
        assert_eq!(
            SignedPermissionHistoryEvent::decode(&tampered)
                .unwrap()
                .verify(b"test-key"),
            Err(GrantVerificationError::InvalidMac)
        );
    }
}
