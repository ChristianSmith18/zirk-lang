//! Consent records shared by the trusted launcher and native runtime.
//!
//! A manifest only requests authority. The launcher signs an exact, project-
//! bound scope after the user approves it; the runtime verifies that record
//! before constructing any effective environment or network grant.

use std::collections::BTreeSet;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const GRANT_SCHEMA_VERSION: u32 = 1;
const MAX_GRANT_BYTES: usize = 64 * 1024;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PermissionScopes {
    pub environment_read: BTreeSet<String>,
    pub environment_secrets: BTreeSet<String>,
    pub environment_list_names: bool,
    pub network_connect: BTreeSet<String>,
    pub network_listen: BTreeSet<String>,
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
            environment_read: ["APP_MODE".into()].into(),
            environment_secrets: ["DB_PASSWORD".into()].into(),
            environment_list_names: false,
            network_connect: ["https://api.example:443".into()].into(),
            network_listen: BTreeSet::new(),
        }
    }

    #[test]
    fn signed_scope_round_trips_and_is_project_bound() {
        let signed =
            SignedPermissionGrant::sign("a".repeat(64), "b".repeat(64), scopes(), b"test-key")
                .unwrap();
        let encoded = signed.encode().unwrap();
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
}
