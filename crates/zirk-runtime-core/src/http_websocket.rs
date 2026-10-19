//! Bounded HTTP/1.1 WebSocket server policy shared by native adapters.

use crate::http_values::Url;

pub const MAX_WEBSOCKET_ORIGINS: usize = 64;
pub const MAX_WEBSOCKET_SUBPROTOCOLS: usize = 64;
pub const MAX_WEBSOCKET_POLICY_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketPolicyError {
    InvalidOrigin,
    OriginDenied,
    InvalidSubprotocol,
    DuplicateSubprotocol,
    PolicyLimit,
}

/// Explicit server-side Origin and subprotocol policy. Origins are normalized
/// when the policy is created; protocol selection follows server preference
/// while requiring the selected token to have been offered by the peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketAcceptPolicy {
    allowed_origins: Vec<String>,
    allow_missing_origin: bool,
    subprotocols: Vec<String>,
}

impl WebSocketAcceptPolicy {
    pub fn new(
        allowed_origins: &[&str],
        allow_missing_origin: bool,
        subprotocols: &[&str],
    ) -> Result<Self, WebSocketPolicyError> {
        if allowed_origins.len() > MAX_WEBSOCKET_ORIGINS
            || subprotocols.len() > MAX_WEBSOCKET_SUBPROTOCOLS
        {
            return Err(WebSocketPolicyError::PolicyLimit);
        }
        let policy_bytes = allowed_origins
            .iter()
            .chain(subprotocols)
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .ok_or(WebSocketPolicyError::PolicyLimit)?;
        if policy_bytes > MAX_WEBSOCKET_POLICY_BYTES {
            return Err(WebSocketPolicyError::PolicyLimit);
        }
        let mut normalized_origins = Vec::with_capacity(allowed_origins.len());
        for origin in allowed_origins {
            let normalized = if *origin == "null" {
                "null".to_owned()
            } else {
                normalize_origin(origin)?
            };
            if !normalized_origins.contains(&normalized) {
                normalized_origins.push(normalized);
            }
        }

        let mut normalized_protocols = Vec::with_capacity(subprotocols.len());
        for protocol in subprotocols {
            if !is_token(protocol) {
                return Err(WebSocketPolicyError::InvalidSubprotocol);
            }
            if normalized_protocols.iter().any(|seen| seen == protocol) {
                return Err(WebSocketPolicyError::DuplicateSubprotocol);
            }
            normalized_protocols.push((*protocol).to_owned());
        }
        Ok(Self {
            allowed_origins: normalized_origins,
            allow_missing_origin,
            subprotocols: normalized_protocols,
        })
    }

    /// Checks the complete Origin field set and selects one offered protocol.
    /// Multiple Origin fields, invalid UTF-8, malformed origins, duplicate
    /// offers, and unconfigured origins fail closed.
    pub fn validate(
        &self,
        origins: &[&[u8]],
        requested_protocols: &[&str],
    ) -> Result<Option<String>, WebSocketPolicyError> {
        if origins.len() > 1 {
            return Err(WebSocketPolicyError::InvalidOrigin);
        }
        match origins.first() {
            None if self.allow_missing_origin => {}
            None => return Err(WebSocketPolicyError::OriginDenied),
            Some(value) => {
                let value =
                    std::str::from_utf8(value).map_err(|_| WebSocketPolicyError::InvalidOrigin)?;
                let normalized = if value == "null" {
                    "null".to_owned()
                } else {
                    normalize_origin(value)?
                };
                if !self.allowed_origins.contains(&normalized) {
                    return Err(WebSocketPolicyError::OriginDenied);
                }
            }
        }

        if requested_protocols.len() > MAX_WEBSOCKET_SUBPROTOCOLS {
            return Err(WebSocketPolicyError::PolicyLimit);
        }
        let request_bytes = requested_protocols
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .ok_or(WebSocketPolicyError::PolicyLimit)?;
        if request_bytes > MAX_WEBSOCKET_POLICY_BYTES {
            return Err(WebSocketPolicyError::PolicyLimit);
        }
        let mut requested = Vec::with_capacity(requested_protocols.len());
        for protocol in requested_protocols {
            if !is_token(protocol) {
                return Err(WebSocketPolicyError::InvalidSubprotocol);
            }
            if requested.iter().any(|seen| seen == protocol) {
                return Err(WebSocketPolicyError::DuplicateSubprotocol);
            }
            requested.push(*protocol);
        }

        Ok(self
            .subprotocols
            .iter()
            .find(|preferred| requested.contains(&preferred.as_str()))
            .cloned())
    }
}

fn normalize_origin(value: &str) -> Result<String, WebSocketPolicyError> {
    let url = Url::parse(value).map_err(|_| WebSocketPolicyError::InvalidOrigin)?;
    let parsed = url::Url::parse(url.as_str()).map_err(|_| WebSocketPolicyError::InvalidOrigin)?;
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(WebSocketPolicyError::InvalidOrigin);
    }
    Ok(url.origin())
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_enforces_exact_origin_allowlist() {
        let policy = WebSocketAcceptPolicy::new(
            &["HTTPS://App.Example:443"],
            false,
            &["zirk.v2", "zirk.v1"],
        )
        .unwrap();
        assert_eq!(
            policy
                .validate(&[b"https://app.example"], &["zirk.v1", "zirk.v2"])
                .unwrap(),
            Some("zirk.v2".to_owned())
        );
        assert_eq!(
            policy.validate(&[b"https://evil.example"], &[]),
            Err(WebSocketPolicyError::OriginDenied)
        );
        assert_eq!(
            policy.validate(&[], &[]),
            Err(WebSocketPolicyError::OriginDenied)
        );
    }

    #[test]
    fn missing_and_null_origins_require_explicit_policy() {
        let policy = WebSocketAcceptPolicy::new(&["null"], true, &[]).unwrap();
        assert_eq!(policy.validate(&[], &[]), Ok(None));
        assert_eq!(policy.validate(&[b"null"], &[]), Ok(None));
        assert_eq!(
            policy.validate(&[b""], &[]),
            Err(WebSocketPolicyError::InvalidOrigin)
        );
    }

    #[test]
    fn rejects_ambiguous_origins_and_invalid_origin_components() {
        let policy = WebSocketAcceptPolicy::new(&["https://app.example/path"], true, &[]);
        assert_eq!(policy, Err(WebSocketPolicyError::InvalidOrigin));
        let policy = WebSocketAcceptPolicy::new(&["https://app.example"], true, &[]).unwrap();
        assert_eq!(
            policy.validate(&[b"https://app.example", b"https://app.example"], &[]),
            Err(WebSocketPolicyError::InvalidOrigin)
        );
        assert_eq!(
            policy.validate(&[b"\xff"], &[]),
            Err(WebSocketPolicyError::InvalidOrigin)
        );
    }

    #[test]
    fn validates_subprotocol_tokens_uniqueness_and_server_preference() {
        let policy = WebSocketAcceptPolicy::new(&[], true, &["zirk.v2", "zirk.v1"]).unwrap();
        assert_eq!(
            policy.validate(&[], &["zirk.v1", "zirk.v2"]).unwrap(),
            Some("zirk.v2".to_owned())
        );
        assert_eq!(policy.validate(&[], &["unoffered"]).unwrap(), None);
        assert_eq!(
            policy.validate(&[], &["zirk.v1", "zirk.v1"]),
            Err(WebSocketPolicyError::DuplicateSubprotocol)
        );
        assert_eq!(
            policy.validate(&[], &["bad protocol"]),
            Err(WebSocketPolicyError::InvalidSubprotocol)
        );
        assert_eq!(
            WebSocketAcceptPolicy::new(&[], true, &["bad protocol"]),
            Err(WebSocketPolicyError::InvalidSubprotocol)
        );
    }

    #[test]
    fn policy_has_independent_origin_protocol_and_byte_bounds() {
        assert_eq!(
            WebSocketAcceptPolicy::new(
                &vec!["https://app.example"; MAX_WEBSOCKET_ORIGINS + 1],
                false,
                &[],
            ),
            Err(WebSocketPolicyError::PolicyLimit)
        );
        assert_eq!(
            WebSocketAcceptPolicy::new(
                &[],
                false,
                &vec!["zirk.v1"; MAX_WEBSOCKET_SUBPROTOCOLS + 1],
            ),
            Err(WebSocketPolicyError::PolicyLimit)
        );
        let long = "a".repeat(MAX_WEBSOCKET_POLICY_BYTES + 1);
        assert_eq!(
            WebSocketAcceptPolicy::new(&[long.as_str()], false, &[]),
            Err(WebSocketPolicyError::PolicyLimit)
        );
    }
}
