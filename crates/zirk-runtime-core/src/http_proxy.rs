//! Explicit HTTP proxy configuration shared by the runtime client pool.

use std::fmt;
use std::net::IpAddr;

use base64::Engine as _;
use url::Url;
use zeroize::Zeroizing;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyConfigError {
    InvalidUrl,
    UnsupportedScheme,
    CredentialsForbidden,
    PathNotAllowed,
    InvalidCredentials,
}

#[derive(Clone, PartialEq, Eq)]
struct ProxyAuthorization(Zeroizing<String>);

impl fmt::Debug for ProxyAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProxyAuthorization(<redacted>)")
    }
}

impl ProxyAuthorization {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// An HTTP(S) forward proxy. Credentials are a separate secret-bearing
/// configuration so they cannot enter URL diagnostics or cleartext transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpProxy {
    url: Url,
    authorization: Option<ProxyAuthorization>,
}

impl HttpProxy {
    pub fn parse(input: &str) -> Result<Self, ProxyConfigError> {
        Self::parse_with_credentials(input, None)
    }

    /// Parses an HTTP(S) forward proxy and optional Basic credentials supplied
    /// separately from its URL as one `username:password` secret.
    pub fn parse_with_credentials(
        input: &str,
        credentials: Option<&str>,
    ) -> Result<Self, ProxyConfigError> {
        let url = Url::parse(input).map_err(|_| ProxyConfigError::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ProxyConfigError::UnsupportedScheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ProxyConfigError::CredentialsForbidden);
        }
        if url.host_str().is_none() || url.port_or_known_default().is_none() {
            return Err(ProxyConfigError::InvalidUrl);
        }
        if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
            return Err(ProxyConfigError::PathNotAllowed);
        }
        let authorization = credentials
            .map(|credentials| {
                let is_loopback = url
                    .host_str()
                    .and_then(|host| host.parse::<IpAddr>().ok())
                    .is_some_and(|address| address.is_loopback());
                let (username, password) = credentials
                    .split_once(':')
                    .ok_or(ProxyConfigError::InvalidCredentials)?;
                let encrypted_transport = url.scheme() == "https";
                if (!is_loopback && !encrypted_transport)
                    || username.is_empty()
                    || username.contains(':')
                    || credentials.len() > 4096
                    || credentials.bytes().any(|byte| byte.is_ascii_control())
                {
                    return Err(ProxyConfigError::InvalidCredentials);
                }
                let mut raw = Zeroizing::new(Vec::with_capacity(credentials.len()));
                raw.extend_from_slice(username.as_bytes());
                raw.push(b':');
                raw.extend_from_slice(password.as_bytes());
                // Keep every owned credential representation in zeroizing
                // storage. `format!` here would create an ordinary temporary
                // String containing a second copy of the encoded secret.
                let encoded = Zeroizing::new(
                    base64::engine::general_purpose::STANDARD.encode(raw.as_slice()),
                );
                let mut authorization = Zeroizing::new(String::with_capacity(6 + encoded.len()));
                authorization.push_str("Basic ");
                authorization.push_str(&encoded);
                Ok(ProxyAuthorization(authorization))
            })
            .transpose()?;
        Ok(Self { url, authorization })
    }

    pub fn origin(&self) -> String {
        let host = self
            .url
            .host_str()
            .expect("validated proxy has a host")
            .trim_start_matches('[')
            .trim_end_matches(']');
        let authority_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        format!(
            "{}://{authority_host}:{}",
            self.url.scheme(),
            self.url
                .port_or_known_default()
                .expect("validated proxy has an effective port")
        )
    }

    pub fn authority(&self) -> String {
        let host = self
            .url
            .host_str()
            .expect("validated proxy has a host")
            .trim_start_matches('[')
            .trim_end_matches(']');
        let authority_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        format!(
            "{authority_host}:{}",
            self.url
                .port_or_known_default()
                .expect("validated proxy has an effective port")
        )
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn authorization_header(&self) -> Option<&str> {
        self.authorization.as_ref().map(ProxyAuthorization::as_str)
    }

    pub fn host(&self) -> &str {
        self.url
            .host_str()
            .expect("validated proxy has a host")
            .trim_start_matches('[')
            .trim_end_matches(']')
    }

    pub fn port(&self) -> u16 {
        self.url
            .port_or_known_default()
            .expect("validated proxy has an effective port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_origin_scoped_http_and_https_proxy_urls() {
        let proxy = HttpProxy::parse("http://proxy.example:8080").unwrap();
        assert_eq!(proxy.origin(), "http://proxy.example:8080");
        assert_eq!(proxy.authority(), "proxy.example:8080");
        assert_eq!(proxy.host(), "proxy.example");
        assert_eq!(proxy.port(), 8080);
    }

    #[test]
    fn normalizes_default_port_and_ipv6_authority() {
        let proxy = HttpProxy::parse("http://[::1]/").unwrap();
        assert_eq!(proxy.origin(), "http://[::1]:80");
        assert_eq!(proxy.authority(), "[::1]:80");
    }

    #[test]
    fn rejects_proxy_credentials_and_non_origin_urls() {
        assert_eq!(
            HttpProxy::parse("http://user:password@proxy.example:8080"),
            Err(ProxyConfigError::CredentialsForbidden)
        );
        assert_eq!(
            HttpProxy::parse("http://proxy.example:8080/path"),
            Err(ProxyConfigError::PathNotAllowed)
        );
        assert_eq!(
            HttpProxy::parse("http://proxy.example:8080?mode=1"),
            Err(ProxyConfigError::PathNotAllowed)
        );
        assert_eq!(
            HttpProxy::parse("https://proxy.example:8443")
                .unwrap()
                .origin(),
            "https://proxy.example:8443"
        );
    }

    #[test]
    fn proxy_basic_credentials_are_separate_and_redacted() {
        let proxy =
            HttpProxy::parse_with_credentials("http://127.0.0.1:8080", Some("zirk-user:p@ss:word"))
                .unwrap();
        assert_eq!(
            proxy.authorization_header(),
            Some("Basic emlyay11c2VyOnBAc3M6d29yZA==")
        );
        let debug = format!("{proxy:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("p@ss"));
        assert!(!proxy.as_str().contains("zirk-user"));
    }

    #[test]
    fn refuses_to_send_basic_proxy_credentials_to_cleartext_remote_proxies() {
        assert_eq!(
            HttpProxy::parse_with_credentials("http://proxy.example:8080", Some("user:secret")),
            Err(ProxyConfigError::InvalidCredentials)
        );
    }

    #[test]
    fn permits_remote_basic_credentials_only_when_proxy_transport_uses_tls() {
        let proxy = HttpProxy::parse_with_credentials(
            "https://proxy.example:8443",
            Some("zirk-user:secret"),
        )
        .unwrap();
        assert_eq!(proxy.origin(), "https://proxy.example:8443");
        assert_eq!(
            proxy.authorization_header(),
            Some("Basic emlyay11c2VyOnNlY3JldA==")
        );
        let debug = format!("{proxy:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn rejects_invalid_proxy_basic_credentials() {
        for credentials in ["", "no-separator", ":password", "user\n:password"] {
            assert_eq!(
                HttpProxy::parse_with_credentials("http://proxy.example:8080", Some(credentials)),
                Err(ProxyConfigError::InvalidCredentials),
                "credentials {credentials:?}"
            );
        }
    }
}
