//! Bounded in-memory HTTP cookie storage for reusable clients.
//!
//! The jar never exposes cookie values individually. Callers provide the
//! request URL and its same-site context, and receive only the `Cookie` field
//! value that is eligible for that request.

use std::fmt;
use std::time::{Duration, SystemTime};

use crate::http_values::Url;
use zeroize::{Zeroize, Zeroizing};

const MAX_COOKIES: usize = 3000;
const MAX_COOKIES_PER_DOMAIN: usize = 50;
const MAX_SET_COOKIE_BYTES: usize = 8192;
const MAX_COOKIE_PAIR_BYTES: usize = 4096;
const MAX_COOKIE_ATTRIBUTE_VALUE_BYTES: usize = 1024;
const MAX_COOKIE_STORAGE_ENTRY_BYTES: usize = 8192;
const MAX_JAR_BYTES: usize = 1024 * 1024;
const MAX_COOKIE_HEADER_BYTES: usize = 8192;
const MAX_COOKIE_AGE: Duration = Duration::from_secs(400 * 24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieError {
    InvalidOrigin,
    InvalidSyntax,
    InvalidDomain,
    PublicSuffix,
    SecureOverlay,
    InvalidPrefix,
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SameSite {
    Strict,
    Lax,
    None,
}

#[derive(Clone)]
struct Cookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    host_only: bool,
    secure: bool,
    same_site: SameSite,
    expires: Option<SystemTime>,
    creation: u64,
    last_access: u64,
}

impl Drop for Cookie {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

impl fmt::Debug for Cookie {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .field("host_only", &self.host_only)
            .field("secure", &self.secure)
            .field("same_site", &self.same_site)
            .finish()
    }
}

impl Cookie {
    fn key_matches(&self, name: &str, domain: &str, path: &str) -> bool {
        self.name == name && self.domain == domain && self.path == path
    }

    fn encoded_size(&self) -> usize {
        self.name
            .len()
            .saturating_add(self.value.len())
            .saturating_add(self.domain.len())
            .saturating_add(self.path.len())
            .saturating_add(96)
    }
}

/// A memory-only, bounded cookie jar. Persistence and filesystem access are
/// deliberately owned by a higher-level client policy.
#[derive(Default)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
    bytes: usize,
    sequence: u64,
}

impl fmt::Debug for CookieJar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieJar")
            .field("cookie_count", &self.cookies.len())
            .field("stored_bytes", &self.bytes)
            .finish()
    }
}

/// Ephemeral header material. Its backing string is zeroed when dropped and
/// debug formatting never reveals cookie values.
pub struct CookieHeader(Zeroizing<String>);

impl CookieHeader {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for CookieHeader {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for CookieHeader {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq<String> for CookieHeader {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for CookieHeader {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Debug for CookieHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CookieHeader(<redacted>)")
    }
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    pub fn clear(&mut self) {
        self.cookies.clear();
        self.bytes = 0;
    }

    /// Stores one `Set-Cookie` field received from `request_url`.
    pub fn store_set_cookie(
        &mut self,
        request_url: &Url,
        set_cookie: &[u8],
        now: SystemTime,
    ) -> Result<(), CookieError> {
        if set_cookie.len() > MAX_SET_COOKIE_BYTES {
            return Err(CookieError::TooLarge);
        }
        let text = std::str::from_utf8(set_cookie).map_err(|_| CookieError::InvalidSyntax)?;
        if text
            .bytes()
            .any(|byte| byte < 0x20 && byte != b'\t' || byte == 0x7f)
        {
            return Err(CookieError::InvalidSyntax);
        }
        let host = request_url.host();
        let secure_origin = request_url.scheme() == "https";
        let mut fields = text.split(';');
        let pair = fields.next().ok_or(CookieError::InvalidSyntax)?;
        let (name, value) = pair
            .trim()
            .split_once('=')
            .ok_or(CookieError::InvalidSyntax)?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty()
            || http::header::HeaderName::from_bytes(name.as_bytes()).is_err()
            || !valid_cookie_value(value)
        {
            return Err(CookieError::InvalidSyntax);
        }
        if name.len().saturating_add(value.len()) > MAX_COOKIE_PAIR_BYTES {
            return Err(CookieError::TooLarge);
        }

        let mut domain = host.to_ascii_lowercase();
        let mut host_only = true;
        let mut domain_attribute = None;
        let mut path = default_cookie_path(request_url.path());
        let mut secure = false;
        let mut host_prefix_path = false;
        let mut same_site = SameSite::Lax;
        let mut expires = None;
        let mut max_age = None;

        for attribute in fields {
            let attribute = attribute.trim();
            if attribute.is_empty() {
                continue;
            }
            let (attribute_name, attribute_value) = attribute
                .split_once('=')
                .map_or((attribute, None), |(name, value)| {
                    (name, Some(value.trim()))
                });
            if attribute_value.is_some_and(|value| value.len() > MAX_COOKIE_ATTRIBUTE_VALUE_BYTES) {
                let name = attribute_name.trim();
                if name.eq_ignore_ascii_case("domain") || name.eq_ignore_ascii_case("path") {
                    return Err(CookieError::TooLarge);
                }
                continue;
            }
            match attribute_name.trim().to_ascii_lowercase().as_str() {
                "domain" => {
                    if let Some(raw_domain) = attribute_value.filter(|value| !value.is_empty()) {
                        domain_attribute = Some(
                            raw_domain
                                .strip_prefix('.')
                                .unwrap_or(raw_domain)
                                .to_owned(),
                        );
                    }
                }
                "path" => {
                    let candidate = attribute_value.filter(|value| value.starts_with('/'));
                    path = candidate
                        .map(str::to_owned)
                        .unwrap_or_else(|| default_cookie_path(request_url.path()));
                    host_prefix_path = candidate == Some("/");
                }
                "secure" => secure = true,
                "httponly" => {}
                "samesite" => {
                    same_site = match attribute_value.map(str::to_ascii_lowercase).as_deref() {
                        Some("strict") => SameSite::Strict,
                        Some("none") => SameSite::None,
                        _ => SameSite::Lax,
                    };
                }
                "max-age" => {
                    if let Some(parsed) = attribute_value.and_then(parse_max_age) {
                        max_age = Some(parsed);
                    }
                }
                "expires" => {
                    if let Some(parsed) =
                        attribute_value.and_then(|value| httpdate::parse_http_date(value).ok())
                    {
                        expires = Some(parsed);
                    }
                }
                _ => {}
            }
        }

        if let Some(raw_candidate) = domain_attribute {
            let candidate = normalize_cookie_domain(&raw_candidate)?;
            if !domain_matches(host, &candidate) || is_ip_host(host) {
                return Err(CookieError::InvalidDomain);
            }
            if psl2::lookup(&candidate).is_some_and(|entry| entry.is_public_suffix()) {
                if candidate != host {
                    return Err(CookieError::PublicSuffix);
                }
                // A public-suffix Domain equal to the request host is
                // ignored, leaving the cookie host-only.
            } else {
                domain = candidate;
                host_only = false;
            }
        }

        if secure && !secure_origin {
            return Err(CookieError::InvalidOrigin);
        }
        if same_site == SameSite::None && !secure {
            return Err(CookieError::InvalidSyntax);
        }
        if name
            .get(.."__Secure-".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("__Secure-"))
            && !(secure && secure_origin)
        {
            return Err(CookieError::InvalidPrefix);
        }
        if name
            .get(.."__Host-".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("__Host-"))
            && !(secure && secure_origin && host_only && host_prefix_path)
        {
            return Err(CookieError::InvalidPrefix);
        }

        self.prune_expired(now);
        if !secure_origin
            && !secure
            && self.cookies.iter().any(|existing| {
                existing.secure
                    && existing.name == name
                    && (domain_matches(&existing.domain, &domain)
                        || domain_matches(&domain, &existing.domain))
                    && path_matches(&path, &existing.path)
            })
        {
            return Err(CookieError::SecureOverlay);
        }
        if let Some(max_age) = max_age {
            if max_age <= 0 {
                self.remove_key(name, &domain, &path);
                return Ok(());
            }
            expires = Some(
                now.checked_add(Duration::from_secs(max_age as u64).min(MAX_COOKIE_AGE))
                    .unwrap_or(SystemTime::UNIX_EPOCH + MAX_COOKIE_AGE),
            );
        }
        if let Some(deadline) = expires
            && let Some(maximum_deadline) = now.checked_add(MAX_COOKIE_AGE)
            && deadline > maximum_deadline
        {
            expires = Some(maximum_deadline);
        }
        if expires.is_some_and(|deadline| deadline <= now) {
            self.remove_key(name, &domain, &path);
            return Ok(());
        }

        let existing = self
            .cookies
            .iter()
            .position(|cookie| cookie.key_matches(name, &domain, &path));
        let creation = if let Some(index) = existing {
            self.cookies[index].creation
        } else {
            self.next_sequence()
        };
        let cookie = Cookie {
            name: name.to_owned(),
            value: value.to_owned(),
            domain,
            path,
            host_only,
            secure,
            same_site,
            expires,
            creation,
            last_access: self.sequence,
        };
        let size = cookie.encoded_size();
        if size > MAX_COOKIE_STORAGE_ENTRY_BYTES || size > MAX_JAR_BYTES {
            return Err(CookieError::TooLarge);
        }
        if let Some(index) = existing {
            let old = self.cookies.remove(index);
            self.bytes = self.bytes.saturating_sub(old.encoded_size());
        }
        while self.cookies.len() >= MAX_COOKIES
            || self.bytes.saturating_add(size) > MAX_JAR_BYTES
            || self
                .cookies
                .iter()
                .filter(|existing| existing.domain == cookie.domain)
                .count()
                >= MAX_COOKIES_PER_DOMAIN
        {
            let domain_is_full = self
                .cookies
                .iter()
                .filter(|existing| existing.domain == cookie.domain)
                .count()
                >= MAX_COOKIES_PER_DOMAIN;
            let candidate = self
                .cookies
                .iter()
                .enumerate()
                .filter(|(_, existing)| !domain_is_full || existing.domain == cookie.domain)
                .min_by_key(|(_, existing)| (existing.secure, existing.last_access))
                .map(|(index, _)| index);
            let Some(index) = candidate else {
                break;
            };
            let evicted = self.cookies.remove(index);
            self.bytes = self.bytes.saturating_sub(evicted.encoded_size());
        }
        self.bytes += size;
        self.cookies.push(cookie);
        Ok(())
    }

    /// Builds a request cookie field for `url` using explicit same-site and
    /// top-level navigation context. The jar updates LRU metadata only for
    /// cookies actually sent.
    pub fn cookie_header(
        &mut self,
        url: &Url,
        site_for_cookies: Option<&Url>,
        top_level_navigation: bool,
        safe_method: bool,
        now: SystemTime,
    ) -> Option<CookieHeader> {
        let host = url.host().to_ascii_lowercase();
        let path = url.path();
        let secure = url.scheme() == "https";
        self.prune_expired(now);
        let same_site =
            site_for_cookies.is_some_and(|site| schemeful_site(site) == schemeful_site(url));
        let sequence = self.next_sequence();
        let mut selected = Vec::with_capacity(self.cookies.len());
        for (index, cookie) in self.cookies.iter().enumerate() {
            let eligible = {
                (if cookie.host_only {
                    cookie.domain == host
                } else {
                    domain_matches(&host, &cookie.domain)
                }) && path_matches(path, &cookie.path)
                    && (!cookie.secure || secure)
                    && match cookie.same_site {
                        SameSite::Strict => same_site,
                        SameSite::Lax => same_site || (top_level_navigation && safe_method),
                        SameSite::None => true,
                    }
            };
            if eligible {
                selected.push(index);
            }
        }
        selected.sort_by(|left, right| {
            self.cookies[*right]
                .path
                .len()
                .cmp(&self.cookies[*left].path.len())
                .then_with(|| {
                    self.cookies[*left]
                        .creation
                        .cmp(&self.cookies[*right].creation)
                })
        });
        let mut header = Zeroizing::new(String::new());
        for index in selected {
            let required = self.cookies[index].name.len()
                + self.cookies[index].value.len()
                + usize::from(!header.is_empty()) * 2;
            if header.len().saturating_add(required) > MAX_COOKIE_HEADER_BYTES {
                continue;
            }
            if !header.is_empty() {
                header.push_str("; ");
            }
            let cookie = &self.cookies[index];
            header.push_str(&cookie.name);
            header.push('=');
            header.push_str(&cookie.value);
            self.cookies[index].last_access = sequence;
        }
        (!header.is_empty()).then_some(CookieHeader(header))
    }

    fn prune_expired(&mut self, now: SystemTime) {
        let mut index = 0;
        while index < self.cookies.len() {
            if self.cookies[index]
                .expires
                .is_some_and(|deadline| deadline <= now)
            {
                let expired = self.cookies.remove(index);
                self.bytes = self.bytes.saturating_sub(expired.encoded_size());
            } else {
                index += 1;
            }
        }
    }

    fn remove_key(&mut self, name: &str, domain: &str, path: &str) {
        if let Some(index) = self
            .cookies
            .iter()
            .position(|cookie| cookie.key_matches(name, domain, path))
        {
            let removed = self.cookies.remove(index);
            self.bytes = self.bytes.saturating_sub(removed.encoded_size());
        }
    }

    fn next_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1).max(1);
        self.sequence
    }
}

fn valid_cookie_value(value: &str) -> bool {
    let bytes = value.as_bytes();
    let (start, end) = if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        (1, bytes.len() - 1)
    } else {
        (0, bytes.len())
    };
    bytes[start..end]
        .iter()
        .all(|byte| matches!(*byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

fn parse_max_age(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(value.parse().unwrap_or_else(|_| {
        if value.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    }))
}

fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn valid_domain_name(domain: &str) -> bool {
    domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

/// Canonicalizes an IDNA cookie Domain attribute to the URL host form. URL
/// host parsing percent-decodes input, so reject URL/authority syntax first.
fn normalize_cookie_domain(raw: &str) -> Result<String, CookieError> {
    if raw.is_empty()
        || raw.ends_with('.')
        || raw.chars().any(|character| {
            character.is_ascii()
                && !character.is_ascii_alphanumeric()
                && !matches!(character, '.' | '-')
        })
    {
        return Err(CookieError::InvalidDomain);
    }
    let url::Host::Domain(domain) =
        url::Host::parse(raw).map_err(|_| CookieError::InvalidDomain)?
    else {
        return Err(CookieError::InvalidDomain);
    };
    let domain = domain.to_ascii_lowercase();
    if !valid_domain_name(&domain) {
        return Err(CookieError::InvalidDomain);
    }
    Ok(domain)
}

fn is_ip_host(host: &str) -> bool {
    matches!(
        url::Host::parse(host),
        Ok(url::Host::Ipv4(_) | url::Host::Ipv6(_))
    )
}

fn default_cookie_path(request_path: &str) -> String {
    if !request_path.starts_with('/') || request_path.matches('/').count() <= 1 {
        return "/".to_owned();
    }
    request_path
        .rfind('/')
        .filter(|index| *index > 0)
        .map_or_else(|| "/".to_owned(), |index| request_path[..index].to_owned())
}

fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    request_path == cookie_path
        || request_path
            .strip_prefix(cookie_path)
            .is_some_and(|remainder| cookie_path.ends_with('/') || remainder.starts_with('/'))
}

fn schemeful_site(url: &Url) -> (String, String) {
    let host = url.host().to_ascii_lowercase();
    let site = if host.parse::<std::net::IpAddr>().is_ok() || host == "localhost" {
        host
    } else {
        psl2::lookup(&host)
            .and_then(|domain| domain.registrable_domain().map(str::to_owned))
            .unwrap_or(host)
    };
    (url.scheme().to_owned(), site)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn applies_host_path_secure_and_same_site_rules() {
        let mut jar = CookieJar::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        jar.store_set_cookie(
            &url("https://api.example.com/a/b"),
            b"root=1; Path=/; Secure; HttpOnly; SameSite=Strict",
            now,
        )
        .unwrap();
        jar.store_set_cookie(
            &url("https://api.example.com/a/b"),
            b"scoped=2; Path=/a; SameSite=Lax",
            now,
        )
        .unwrap();

        assert_eq!(
            jar.cookie_header(
                &url("https://api.example.com/a/next"),
                Some(&url("https://www.example.com/")),
                false,
                true,
                now,
            )
            .map(|header| header.as_str().to_owned()),
            Some("scoped=2; root=1".to_owned())
        );
        assert_eq!(
            jar.cookie_header(
                &url("http://api.example.com/a/next"),
                Some(&url("http://www.example.com/")),
                false,
                true,
                now,
            )
            .map(|header| header.as_str().to_owned()),
            Some("scoped=2".to_owned())
        );
        assert_eq!(
            jar.cookie_header(
                &url("https://api.example.com/a/other"),
                Some(&url("https://www.other.test/")),
                true,
                true,
                now,
            )
            .map(|header| header.as_str().to_owned()),
            Some("scoped=2".to_owned())
        );
    }

    #[test]
    fn domain_attributes_obey_domain_match_and_public_suffix_rules() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        jar.store_set_cookie(
            &url("https://api.example.co.uk/"),
            b"shared=1; Domain=.example.co.uk; Path=/",
            now,
        )
        .unwrap();
        assert_eq!(
            jar.cookie_header(
                &url("https://www.example.co.uk/"),
                Some(&url("https://example.co.uk/")),
                false,
                true,
                now,
            )
            .map(|header| header.as_str().to_owned()),
            Some("shared=1".to_owned())
        );
        assert_eq!(
            jar.store_set_cookie(&url("https://example.co.uk/"), b"bad=1; Domain=co.uk", now,),
            Err(CookieError::PublicSuffix)
        );
        assert_eq!(
            jar.store_set_cookie(
                &url("https://example.co.uk/"),
                b"bad=1; Domain=evil.co.uk",
                now,
            ),
            Err(CookieError::InvalidDomain)
        );
        jar.store_set_cookie(&url("https://co.uk/"), b"host-only=1; Domain=co.uk", now)
            .unwrap();
        assert_eq!(
            jar.cookie_header(&url("https://co.uk/"), None, true, true, now)
                .map(|header| header.as_str().to_owned()),
            Some("host-only=1".to_owned())
        );
    }

    #[test]
    fn last_domain_attribute_is_validated_and_stored() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        let origin = url("https://api.example.com/");
        jar.store_set_cookie(&origin, b"shared=1; Domain=com; Domain=example.com", now)
            .unwrap();
        assert_eq!(jar.cookies[0].domain, "example.com");
        assert!(!jar.cookies[0].host_only);
        assert_eq!(
            jar.cookie_header(
                &url("https://www.example.com/"),
                Some(&origin),
                false,
                true,
                now,
            )
            .as_deref(),
            Some("shared=1")
        );

        assert_eq!(
            jar.store_set_cookie(&origin, b"rejected=1; Domain=example.com; Domain=com", now,),
            Err(CookieError::PublicSuffix)
        );
        assert_eq!(
            jar.store_set_cookie(
                &origin,
                b"rejected=1; Domain=example.com; Domain=evil.com",
                now,
            ),
            Err(CookieError::InvalidDomain)
        );
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn domain_attributes_use_idna_without_accepting_url_escapes() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        let origin = url("https://api.xn--bcher-kva.example/");
        jar.store_set_cookie(&origin, "idn=1; Domain=bücher.example".as_bytes(), now)
            .unwrap();
        assert_eq!(jar.len(), 1);
        assert_eq!(
            jar.cookie_header(
                &url("https://www.xn--bcher-kva.example/"),
                Some(&url("https://xn--bcher-kva.example/")),
                false,
                true,
                now,
            )
            .as_deref(),
            Some("idn=1")
        );

        let mut escaped = CookieJar::new();
        assert_eq!(
            escaped.store_set_cookie(
                &url("https://api.example.com/"),
                b"bad=1; Domain=%65xample.com",
                now,
            ),
            Err(CookieError::InvalidDomain)
        );
        assert!(escaped.is_empty());
    }

    #[test]
    fn ip_literal_hosts_cannot_set_domain_scoped_cookies() {
        let mut jar = CookieJar::new();
        assert_eq!(
            jar.store_set_cookie(
                &url("https://[::1]/"),
                b"bad=1; Domain=example.com",
                SystemTime::now(),
            ),
            Err(CookieError::InvalidDomain)
        );
        assert!(jar.is_empty());
    }

    #[test]
    fn expiry_max_age_deletion_and_update_follow_cookie_identity() {
        let mut jar = CookieJar::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        let origin = url("https://example.com/a/b");
        jar.store_set_cookie(&origin, b"sid=old; Path=/; Max-Age=60", now)
            .unwrap();
        jar.store_set_cookie(&origin, b"sid=new; Path=/; Max-Age=120", now)
            .unwrap();
        assert_eq!(jar.len(), 1);
        assert_eq!(
            jar.cookie_header(&origin, Some(&origin), false, true, now)
                .map(|header| header.as_str().to_owned()),
            Some("sid=new".to_owned())
        );
        jar.store_set_cookie(&origin, b"sid=gone; Path=/; Max-Age=0", now)
            .unwrap();
        assert!(jar.is_empty());
        jar.store_set_cookie(
            &origin,
            b"expired=1; Expires=Wed, 21 Oct 2015 07:28:00 GMT",
            now,
        )
        .unwrap();
        assert!(jar.is_empty());
        jar.store_set_cookie(
            &origin,
            b"long=1; Max-Age=999999999999999999999999999999999999",
            now,
        )
        .unwrap();
        assert_eq!(jar.cookies[0].expires, Some(now + MAX_COOKIE_AGE));
        jar.store_set_cookie(
            &origin,
            b"long=gone; Max-Age=-999999999999999999999999999999999999",
            now,
        )
        .unwrap();
        assert!(jar.is_empty());
    }

    #[test]
    fn malformed_repeated_expiry_attributes_do_not_erase_valid_values() {
        let mut jar = CookieJar::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        let origin = url("https://example.com/");

        jar.store_set_cookie(&origin, b"age=kept; Max-Age=60; Max-Age=invalid", now)
            .unwrap();
        jar.store_set_cookie(
            &origin,
            b"date=kept; Expires=Wed, 21 Oct 2037 07:28:00 GMT; Expires=invalid",
            now,
        )
        .unwrap();

        assert_eq!(jar.len(), 2);
        assert_eq!(
            jar.cookie_header(&origin, Some(&origin), false, true, now)
                .map(|header| header.as_str().to_owned()),
            Some("age=kept; date=kept".to_owned())
        );
        assert_eq!(
            jar.cookies
                .iter()
                .find(|cookie| cookie.name == "age")
                .and_then(|cookie| cookie.expires),
            Some(now + Duration::from_secs(60))
        );
        assert_eq!(
            jar.cookies
                .iter()
                .find(|cookie| cookie.name == "date")
                .and_then(|cookie| cookie.expires),
            Some(now + MAX_COOKIE_AGE)
        );
    }

    #[test]
    fn expiration_clamping_does_not_overflow_near_system_time_limit() {
        let mut lower = 0_u64;
        let mut upper = u64::MAX;
        while upper - lower > 1 {
            let midpoint = lower + (upper - lower) / 2;
            if SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(midpoint))
                .is_some()
            {
                lower = midpoint;
            } else {
                upper = midpoint;
            }
        }
        let latest = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(lower))
            .expect("binary search retained the latest representable second");
        let now = latest
            .checked_sub(MAX_COOKIE_AGE / 2)
            .expect("system time range exceeds the cookie lifetime");
        assert!(now.checked_add(MAX_COOKIE_AGE).is_none());

        let mut jar = CookieJar::new();
        jar.store_set_cookie(
            &url("https://example.com/"),
            b"short=alive; Max-Age=60",
            now,
        )
        .unwrap();
        assert_eq!(
            jar.cookies[0].expires,
            now.checked_add(Duration::from_secs(60))
        );
    }

    #[test]
    fn secure_prefixes_and_same_site_none_require_secure_origins() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        assert_eq!(
            jar.store_set_cookie(&url("http://example.com/"), b"__Secure-id=1; Secure", now,),
            Err(CookieError::InvalidOrigin)
        );
        assert_eq!(
            jar.store_set_cookie(
                &url("https://example.com/"),
                b"__Host-id=1; Secure; Domain=example.com; Path=/",
                now,
            ),
            Err(CookieError::InvalidPrefix)
        );
        assert_eq!(
            jar.store_set_cookie(&url("https://example.com/"), b"id=1; SameSite=None", now,),
            Err(CookieError::InvalidSyntax)
        );
        jar.store_set_cookie(
            &url("https://example.com/"),
            b"__Host-id=1; Secure; Path=/; SameSite=None",
            now,
        )
        .unwrap();
        assert_eq!(
            jar.cookie_header(&url("https://example.com/"), None, false, false, now)
                .map(|header| header.as_str().to_owned()),
            Some("__Host-id=1".to_owned())
        );
        assert_eq!(
            jar.store_set_cookie(
                &url("https://example.com/"),
                b"__HOST-other=1; Secure; Path=/; Domain=example.com",
                now,
            ),
            Err(CookieError::InvalidPrefix)
        );
        assert_eq!(
            jar.store_set_cookie(
                &url("https://example.com/"),
                b"__Host-no-path=1; Secure",
                now,
            ),
            Err(CookieError::InvalidPrefix)
        );
    }

    #[test]
    fn last_invalid_path_attribute_uses_the_request_default_path() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        let origin = url("https://example.com/admin/login");

        jar.store_set_cookie(&origin, b"scope=private; Path=/; Path=invalid", now)
            .unwrap();
        assert_eq!(jar.cookies[0].path, "/admin");
        assert_eq!(
            jar.cookie_header(
                &url("https://example.com/admin/next"),
                Some(&origin),
                false,
                true,
                now,
            )
            .as_deref(),
            Some("scope=private")
        );
        assert!(
            jar.cookie_header(
                &url("https://example.com/public"),
                Some(&origin),
                false,
                true,
                now,
            )
            .is_none()
        );
    }

    #[test]
    fn oversized_repeated_scope_attributes_reject_the_cookie() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        let origin = url("https://api.example.com/admin/login");
        let oversized = "a".repeat(MAX_COOKIE_ATTRIBUTE_VALUE_BYTES + 1);
        let oversized_path = format!("scope=private; Path=/; Path=/{oversized}");
        let oversized_domain = format!("scope=private; Domain=example.com; Domain={oversized}");

        assert_eq!(
            jar.store_set_cookie(&origin, oversized_path.as_bytes(), now),
            Err(CookieError::TooLarge)
        );
        assert_eq!(
            jar.store_set_cookie(&origin, oversized_domain.as_bytes(), now),
            Err(CookieError::TooLarge)
        );
        assert!(jar.is_empty());
    }

    #[test]
    fn insecure_origins_cannot_overlay_or_delete_secure_cookies() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        jar.store_set_cookie(
            &url("https://example.com/login"),
            b"sid=private; Secure; Path=/login",
            now,
        )
        .unwrap();
        assert_eq!(
            jar.store_set_cookie(
                &url("http://example.com/login/child"),
                b"sid=attacker; Path=/login/child",
                now,
            ),
            Err(CookieError::SecureOverlay)
        );
        assert_eq!(
            jar.store_set_cookie(
                &url("http://example.com/login"),
                b"sid=; Path=/login; Max-Age=0",
                now,
            ),
            Err(CookieError::SecureOverlay)
        );
        // A cookie scoped above the secure cookie's path does not overlay it.
        jar.store_set_cookie(
            &url("http://example.com/login"),
            b"sid=separate; Path=/",
            now,
        )
        .unwrap();
    }

    #[test]
    fn rejects_bad_values_and_bounds_the_jar() {
        let mut jar = CookieJar::new();
        let now = SystemTime::now();
        let origin = url("https://example.com/");
        assert_eq!(
            jar.store_set_cookie(&origin, b"bad=value;\r\nInjected: x", now),
            Err(CookieError::InvalidSyntax)
        );
        assert_eq!(
            jar.store_set_cookie(&origin, &vec![b'a'; MAX_SET_COOKIE_BYTES + 1], now),
            Err(CookieError::TooLarge)
        );
        for index in 0..MAX_COOKIES + 10 {
            let field = format!("c{index}=v; Path=/");
            let host = format!("https://host{index}.example{index}.test/");
            jar.store_set_cookie(&url(&host), field.as_bytes(), now)
                .unwrap();
        }
        assert_eq!(jar.len(), MAX_COOKIES);
        assert!(jar.bytes <= MAX_JAR_BYTES);

        let mut per_domain = CookieJar::new();
        for index in 0..MAX_COOKIES_PER_DOMAIN + 10 {
            let field = format!("c{index}=v; Path=/");
            per_domain
                .store_set_cookie(&origin, field.as_bytes(), now)
                .unwrap();
        }
        assert_eq!(per_domain.len(), MAX_COOKIES_PER_DOMAIN);
        let header = per_domain
            .cookie_header(&origin, Some(&origin), false, true, now)
            .unwrap();
        assert!(!header.contains("c0=v"));
        assert!(header.contains("c59=v"));
    }

    #[test]
    fn cookie_values_are_redacted_from_debug_output() {
        let mut jar = CookieJar::new();
        let origin = url("https://example.com/");
        let now = SystemTime::now();
        jar.store_set_cookie(&origin, b"session=private-session-token; Secure", now)
            .unwrap();
        let header = jar
            .cookie_header(&origin, Some(&origin), false, true, now)
            .unwrap();
        assert!(!format!("{jar:?}").contains("private-session-token"));
        assert!(!format!("{header:?}").contains("private-session-token"));
        assert_eq!(header.as_str(), "session=private-session-token");
    }
}
