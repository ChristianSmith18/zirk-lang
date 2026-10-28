//! Bounded, platform-neutral HTTP URL and header values.

use http::{HeaderMap, HeaderName, HeaderValue};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_HTTP_HEADER_COUNT: usize = 256;
pub const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_HTTP_BUFFERED_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpValueError {
    InvalidUrl,
    UnsupportedScheme,
    UrlCredentialsForbidden,
    InvalidHeaderName,
    InvalidHeaderValue,
    TooManyHeaders,
    HeadersTooLarge,
    ConflictingMessageFraming,
    ForbiddenHttp2Header,
    QueryContentTypeRequired,
    QueryContentTypeConflict,
    InvalidAcceptQuery,
    UnsupportedMethod,
    BodyTooLarge,
    InvalidStatusCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
    Query,
    Other,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Connect => "CONNECT",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Patch => "PATCH",
            Self::Query => "QUERY",
            Self::Other => "OTHER",
        }
    }

    pub fn is_safe(self) -> bool {
        matches!(
            self,
            Self::Get | Self::Head | Self::Options | Self::Trace | Self::Query
        )
    }

    pub fn is_idempotent(self) -> bool {
        matches!(
            self,
            Self::Get
                | Self::Head
                | Self::Put
                | Self::Delete
                | Self::Options
                | Self::Trace
                | Self::Query
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HttpStatusCode(http::StatusCode);

impl HttpStatusCode {
    pub fn new(value: u16) -> Result<Self, HttpValueError> {
        http::StatusCode::from_u16(value)
            .map(Self)
            .map_err(|_| HttpValueError::InvalidStatusCode)
    }

    pub fn as_u16(self) -> u16 {
        self.0.as_u16()
    }

    pub fn is_informational(self) -> bool {
        self.0.is_informational()
    }

    pub fn is_success(self) -> bool {
        self.0.is_success()
    }

    pub fn is_redirection(self) -> bool {
        self.0.is_redirection()
    }

    pub fn is_client_error(self) -> bool {
        self.0.is_client_error()
    }

    pub fn is_server_error(self) -> bool {
        self.0.is_server_error()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestValues {
    pub method: HttpMethod,
    /// Exact HTTP token; standard methods match `method`, extensions retain
    /// their original case and are never inferred from a known method.
    pub method_token: String,
    pub url: Url,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl HttpRequestValues {
    pub fn new(method: HttpMethod, url: Url) -> Self {
        Self {
            method,
            method_token: method.as_str().to_owned(),
            url,
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn set_body(&mut self, body: Vec<u8>) -> Result<(), HttpValueError> {
        if body.len() > MAX_HTTP_BUFFERED_BODY_BYTES {
            return Err(HttpValueError::BodyTooLarge);
        }
        self.body = body;
        Ok(())
    }

    /// Validates the safe-query media contract before any network action.
    pub fn validate_query(&self) -> Result<(), HttpValueError> {
        if self.method != HttpMethod::Query {
            return Ok(());
        }
        let values: Vec<_> = self
            .headers
            .get_all("content-type")
            .into_iter()
            .flatten()
            .collect();
        if values.len() != 1 {
            return Err(if values.is_empty() {
                HttpValueError::QueryContentTypeRequired
            } else {
                HttpValueError::QueryContentTypeConflict
            });
        }
        let media_type = values[0]
            .to_str()
            .ok()
            .and_then(|value| value.parse::<mime::Mime>().ok());
        let Some(media_type) = media_type else {
            return Err(HttpValueError::InvalidHeaderValue);
        };
        // Content-Type identifies one representation. Wildcards are valid in
        // media ranges such as Accept/Accept-Query, but cannot identify the
        // query content format required by RFC 10008.
        if media_type.type_().as_str() == "*" || media_type.subtype().as_str() == "*" {
            return Err(HttpValueError::InvalidHeaderValue);
        }
        Ok(())
    }

    /// Validates method semantics for buffered client requests. CONNECT
    /// switches the HTTP connection into a tunnel and cannot be represented
    /// by the ordinary response-body API; the client exposes dedicated proxy
    /// and WebSocket tunnel operations instead.
    pub fn validate_client_request(&self) -> Result<(), HttpValueError> {
        self.validate_query()?;
        if self.method_token == "CONNECT" {
            return Err(HttpValueError::UnsupportedMethod);
        }
        Ok(())
    }

    /// Fixed-size SHA-256 base key for request caches. Callers must additionally
    /// scope entries by origin authority, principal, proxy, TLS identity, and
    /// the response's `Vary` fields before reuse. Content metadata is part of
    /// the digest because identical bytes can describe different queries.
    pub fn cache_key(&self) -> Vec<u8> {
        let mut digest = Sha256::new();
        for part in [
            self.method_token.as_bytes(),
            self.url.as_str().as_bytes(),
            self.body.as_slice(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        for (name, value) in self
            .headers
            .iter()
            .filter(|(name, _)| name.as_str().starts_with("content-"))
        {
            digest.update((name.as_str().len() as u64).to_be_bytes());
            digest.update(name.as_str().as_bytes());
            digest.update((value.as_bytes().len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        digest.finalize().to_vec()
    }

    /// Adds the request fields named by a parsed response `Vary` field to the
    /// base cache identity. `None` means the response is not safely reusable,
    /// including `Vary: *` and invalid field names. Field names are
    /// case-insensitive; repeated request values retain wire order.
    pub fn cache_variant_key<'a>(
        &self,
        vary_fields: impl IntoIterator<Item = &'a str>,
    ) -> Option<Vec<u8>> {
        let mut normalized = BTreeSet::new();
        for field in vary_fields {
            if field.trim() == "*" {
                return None;
            }
            let name = HeaderName::from_bytes(field.trim().as_bytes()).ok()?;
            normalized.insert(name.as_str().to_owned());
        }

        let mut digest = Sha256::new();
        let base_key = self.cache_key();
        digest.update((base_key.len() as u64).to_be_bytes());
        digest.update(base_key);
        for name in normalized {
            digest.update((name.len() as u64).to_be_bytes());
            digest.update(name.as_bytes());
            let values = self.headers.get_all(&name).into_iter().flatten();
            let values = values.collect::<Vec<_>>();
            digest.update((values.len() as u64).to_be_bytes());
            for value in values {
                digest.update((value.as_bytes().len() as u64).to_be_bytes());
                digest.update(value.as_bytes());
            }
        }
        Some(digest.finalize().to_vec())
    }

    /// Computes a response-specific cache identity from its `Vary` metadata.
    /// `Ok(None)` means the response must not be stored or reused; malformed
    /// metadata is returned as an error and must also fail closed.
    pub fn cache_variant_key_for_response(
        &self,
        response_headers: &Headers,
    ) -> Result<Option<Vec<u8>>, HttpValueError> {
        let Some(fields) = response_headers.vary_fields()? else {
            return Ok(None);
        };
        Ok(self.cache_variant_key(fields.iter().map(String::as_str)))
    }
}

/// An immutable absolute HTTP or HTTPS URL normalized by the URI parser.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(url::Url);

#[cfg(not(target_arch = "wasm32"))]
impl Url {
    pub fn parse(input: &str) -> Result<Self, HttpValueError> {
        let parsed = url::Url::parse(input).map_err(|_| HttpValueError::InvalidUrl)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(HttpValueError::UnsupportedScheme);
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(HttpValueError::UrlCredentialsForbidden);
        }
        if parsed.host_str().is_none() || parsed.port_or_known_default().is_none() {
            return Err(HttpValueError::InvalidUrl);
        }
        Ok(Self(parsed))
    }

    pub fn resolve(&self, reference: &str) -> Result<Self, HttpValueError> {
        let resolved = self
            .0
            .join(reference)
            .map_err(|_| HttpValueError::InvalidUrl)?;
        Self::parse(resolved.as_str())
    }

    /// Returns a copy with one additional query pair, percent-encoding both
    /// components according to `application/x-www-form-urlencoded` rules.
    pub fn with_query(&self, name: &str, value: &str) -> Self {
        let mut next = self.0.clone();
        next.query_pairs_mut().append_pair(name, value);
        Self(next)
    }

    /// Returns a copy without its query component.
    pub fn without_query(&self) -> Self {
        let mut next = self.0.clone();
        next.set_query(None);
        Self(next)
    }

    /// Returns a copy with a new path. URL performs the required escaping;
    /// scheme, authority, query, and fragment stay unchanged.
    pub fn with_path(&self, path: &str) -> Self {
        let mut next = self.0.clone();
        next.set_path(path);
        Self(next)
    }

    pub fn origin(&self) -> String {
        let port = self
            .0
            .port_or_known_default()
            .expect("validated HTTP URL always has an effective port");
        let host = self.0.host_str().unwrap();
        let authority_host = if host.starts_with('[') && host.ends_with(']') {
            host.to_owned()
        } else if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        format!("{}://{authority_host}:{port}", self.0.scheme())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    pub fn host(&self) -> &str {
        self.0.host_str().unwrap()
    }

    pub fn port(&self) -> u16 {
        self.0.port_or_known_default().unwrap()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn query(&self) -> Option<&str> {
        self.0.query()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.0.fragment()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    serialized: String,
    origin: String,
    scheme: String,
    host: String,
    port: u16,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl Url {
    fn construct(input: &str, base: Option<&str>) -> Result<wasm_bindgen::JsValue, HttpValueError> {
        use wasm_bindgen::{JsCast, JsValue};
        let global = js_sys::global();
        let constructor = js_sys::Reflect::get(&global, &JsValue::from_str("URL"))
            .map_err(|_| HttpValueError::InvalidUrl)?
            .dyn_into::<js_sys::Function>()
            .map_err(|_| HttpValueError::InvalidUrl)?;
        let args = js_sys::Array::new();
        args.push(&JsValue::from_str(input));
        if let Some(base) = base {
            args.push(&JsValue::from_str(base));
        }
        js_sys::Reflect::construct(&constructor, &args).map_err(|_| HttpValueError::InvalidUrl)
    }

    fn property(value: &wasm_bindgen::JsValue, name: &str) -> Result<String, HttpValueError> {
        use wasm_bindgen::JsValue;
        js_sys::Reflect::get(value, &JsValue::from_str(name))
            .map_err(|_| HttpValueError::InvalidUrl)?
            .as_string()
            .ok_or(HttpValueError::InvalidUrl)
    }

    fn from_js(value: &wasm_bindgen::JsValue) -> Result<Self, HttpValueError> {
        let serialized = Self::property(value, "href")?;
        let origin = Self::property(value, "origin")?;
        let scheme = Self::property(value, "protocol")?
            .strip_suffix(':')
            .ok_or(HttpValueError::InvalidUrl)?
            .to_owned();
        if !matches!(scheme.as_str(), "http" | "https") {
            return Err(HttpValueError::UnsupportedScheme);
        }
        let username = Self::property(value, "username")?;
        let password = Self::property(value, "password")?;
        if !username.is_empty() || !password.is_empty() {
            return Err(HttpValueError::UrlCredentialsForbidden);
        }
        let host = Self::property(value, "hostname")?;
        if host.is_empty() {
            return Err(HttpValueError::InvalidUrl);
        }
        let explicit_port = Self::property(value, "port")?;
        let port = if explicit_port.is_empty() {
            if scheme == "https" { 443 } else { 80 }
        } else {
            explicit_port
                .parse()
                .map_err(|_| HttpValueError::InvalidUrl)?
        };
        let path = Self::property(value, "pathname")?;
        let query = Self::property(value, "search")?
            .strip_prefix('?')
            .filter(|query| !query.is_empty())
            .map(str::to_owned);
        let fragment = Self::property(value, "hash")?
            .strip_prefix('#')
            .filter(|fragment| !fragment.is_empty())
            .map(str::to_owned);
        Ok(Self {
            serialized,
            origin,
            scheme,
            host,
            port,
            path,
            query,
            fragment,
        })
    }

    pub fn parse(input: &str) -> Result<Self, HttpValueError> {
        Self::from_js(&Self::construct(input, None)?)
    }

    pub fn resolve(&self, reference: &str) -> Result<Self, HttpValueError> {
        Self::from_js(&Self::construct(reference, Some(&self.serialized))?)
    }

    pub fn with_query(&self, name: &str, value: &str) -> Self {
        use wasm_bindgen::JsCast;
        let Ok(url) = Self::construct(&self.serialized, None) else {
            return self.clone();
        };
        let Ok(params) = js_sys::Reflect::get(&url, &"searchParams".into()) else {
            return self.clone();
        };
        let Ok(append) = js_sys::Reflect::get(&params, &"append".into()) else {
            return self.clone();
        };
        let Ok(append) = append.dyn_into::<js_sys::Function>() else {
            return self.clone();
        };
        if append.call2(&params, &name.into(), &value.into()).is_err() {
            return self.clone();
        }
        Self::from_js(&url).unwrap_or_else(|_| self.clone())
    }

    pub fn without_query(&self) -> Self {
        self.with_browser_property("search", "")
    }

    pub fn with_path(&self, path: &str) -> Self {
        self.with_browser_property("pathname", path)
    }

    fn with_browser_property(&self, name: &str, value: &str) -> Self {
        use wasm_bindgen::JsValue;
        let Ok(url) = Self::construct(&self.serialized, None) else {
            return self.clone();
        };
        if js_sys::Reflect::set(&url, &JsValue::from_str(name), &JsValue::from_str(value)).is_err()
        {
            return self.clone();
        }
        Self::from_js(&url).unwrap_or_else(|_| self.clone())
    }

    pub fn origin(&self) -> String {
        self.origin.clone()
    }
    pub fn as_str(&self) -> &str {
        &self.serialized
    }
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }
}

/// A case-insensitive HTTP field map with repeated values and strict bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    fields: HeaderMap,
    bytes: usize,
    count: usize,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &str, value: &[u8]) -> Result<(), HttpValueError> {
        let (name, mut value) = Self::validated_pair(name, value)?;
        if self.field_is_sensitive(&name) {
            value.set_sensitive(true);
        }
        let old_bytes = self
            .fields
            .get_all(&name)
            .iter()
            .map(|old| old.as_bytes().len())
            .sum::<usize>()
            + self.fields.get_all(&name).iter().count() * name.as_str().len();
        let old_count = self.fields.get_all(&name).iter().count();
        let next_count = self.count.saturating_sub(old_count).saturating_add(1);
        if next_count > MAX_HTTP_HEADER_COUNT {
            return Err(HttpValueError::TooManyHeaders);
        }
        let next_bytes = self
            .bytes
            .saturating_sub(old_bytes)
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len());
        if next_bytes > MAX_HTTP_HEADER_BYTES {
            return Err(HttpValueError::HeadersTooLarge);
        }
        self.fields.insert(name, value);
        self.bytes = next_bytes;
        self.count = next_count;
        Ok(())
    }

    pub fn append(&mut self, name: &str, value: &[u8]) -> Result<(), HttpValueError> {
        let (name, mut value) = Self::validated_pair(name, value)?;
        if self.field_is_sensitive(&name) {
            value.set_sensitive(true);
        }
        self.append_pair(name, value)
    }

    /// Appends a caller-designated credential field. This is used when the
    /// field name is application-defined and therefore cannot be recognized
    /// by the standard sensitive-header name list.
    pub fn append_sensitive(&mut self, name: &str, value: &[u8]) -> Result<(), HttpValueError> {
        let (name, mut value) = Self::validated_pair(name, value)?;
        value.set_sensitive(true);
        self.append_pair(name, value)
    }

    fn append_pair(&mut self, name: HeaderName, value: HeaderValue) -> Result<(), HttpValueError> {
        if self.count >= MAX_HTTP_HEADER_COUNT {
            return Err(HttpValueError::TooManyHeaders);
        }
        let next_bytes = self
            .bytes
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len());
        if next_bytes > MAX_HTTP_HEADER_BYTES {
            return Err(HttpValueError::HeadersTooLarge);
        }
        self.fields.append(name, value);
        self.bytes = next_bytes;
        self.count += 1;
        Ok(())
    }

    fn field_is_sensitive(&self, name: &HeaderName) -> bool {
        self.fields
            .get_all(name)
            .iter()
            .any(HeaderValue::is_sensitive)
    }

    pub fn get(&self, name: &str) -> Option<&HeaderValue> {
        HeaderName::from_bytes(name.as_bytes())
            .ok()
            .and_then(|name| self.fields.get(name))
    }

    /// Removes every value for a field name and returns its first value.
    /// Repeated entries are removed together, matching `HeaderMap::remove`.
    pub fn remove(&mut self, name: &str) -> Option<HeaderValue> {
        let name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        let values = self.fields.get_all(&name);
        let removed_count = values.iter().count();
        if removed_count == 0 {
            return None;
        }
        let removed_bytes = values
            .iter()
            .map(|value| name.as_str().len() + value.as_bytes().len())
            .sum::<usize>();
        let first = self.fields.remove(&name);
        self.count = self.count.saturating_sub(removed_count);
        self.bytes = self.bytes.saturating_sub(removed_bytes);
        first
    }

    /// Iterates each distinct field name and its first value. Repeated-value
    /// access remains available through `get_all`; protocol adapters should
    /// use this view when copying a map into a native header collection.
    pub fn iter(&self) -> impl Iterator<Item = (&HeaderName, &HeaderValue)> {
        self.fields.iter()
    }

    pub fn get_all(&self, name: &str) -> Option<impl Iterator<Item = &HeaderValue>> {
        let name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        Some(self.fields.get_all(name).iter())
    }

    /// Parses response `Vary` fields into normalized request field names.
    /// `Ok(None)` represents `Vary: *`, which must never be reused from a
    /// cache. Empty or invalid list members fail closed.
    pub fn vary_fields(&self) -> Result<Option<Vec<String>>, HttpValueError> {
        let mut names = BTreeSet::new();
        for value in self.get_all("vary").into_iter().flatten() {
            let text = value
                .to_str()
                .map_err(|_| HttpValueError::InvalidHeaderValue)?;
            for member in text.split(',') {
                let member = member.trim();
                if member == "*" {
                    return Ok(None);
                }
                if member.is_empty() {
                    return Err(HttpValueError::InvalidHeaderValue);
                }
                let name = HeaderName::from_bytes(member.as_bytes())
                    .map_err(|_| HttpValueError::InvalidHeaderName)?;
                names.insert(name.as_str().to_owned());
            }
        }
        Ok(Some(names.into_iter().collect()))
    }

    pub fn validate_message_framing(&self) -> Result<Option<u64>, HttpValueError> {
        let transfer_encoding = self.fields.get_all("transfer-encoding");
        let content_lengths = self.fields.get_all("content-length");
        if transfer_encoding.iter().next().is_some() && content_lengths.iter().next().is_some() {
            return Err(HttpValueError::ConflictingMessageFraming);
        }
        let mut length = None;
        for value in content_lengths.iter() {
            let parsed = value
                .to_str()
                .ok()
                .and_then(|text| text.trim().parse::<u64>().ok())
                .ok_or(HttpValueError::ConflictingMessageFraming)?;
            if length.is_some_and(|previous| previous != parsed) {
                return Err(HttpValueError::ConflictingMessageFraming);
            }
            length = Some(parsed);
        }
        Ok(length)
    }

    /// Validates an `Accept-Query` response field as the RFC 10008 list of
    /// media ranges encoded with RFC 9651 Structured Fields. Repeated field
    /// lines are combined using the list-field comma semantics.
    pub fn validate_accept_query(&self) -> Result<(), HttpValueError> {
        let Some(mut values) = self.get_all("accept-query") else {
            return Ok(());
        };
        let Some(first_value) = values.next() else {
            return Ok(());
        };
        let mut combined = String::new();
        for (index, value) in std::iter::once(first_value).chain(values).enumerate() {
            let value = value
                .to_str()
                .map_err(|_| HttpValueError::InvalidAcceptQuery)?;
            if index != 0 {
                combined.push_str(", ");
            }
            combined.push_str(value);
            if combined.len() > MAX_HTTP_HEADER_BYTES {
                return Err(HttpValueError::InvalidAcceptQuery);
            }
        }
        let entries = sfv::Parser::new(&combined)
            .parse::<sfv::List>()
            .map_err(|_| HttpValueError::InvalidAcceptQuery)?;
        if entries.is_empty() {
            return Err(HttpValueError::InvalidAcceptQuery);
        }
        for entry in entries {
            let sfv::ListEntry::Item(item) = entry else {
                return Err(HttpValueError::InvalidAcceptQuery);
            };
            let range = match item.bare_item {
                sfv::BareItem::String(value) => value.to_string(),
                sfv::BareItem::Token(value) => value.to_string(),
                _ => return Err(HttpValueError::InvalidAcceptQuery),
            };
            validate_query_media_range(&range)?;
            for value in item.params.values() {
                if !matches!(value, sfv::BareItem::String(_) | sfv::BareItem::Token(_)) {
                    return Err(HttpValueError::InvalidAcceptQuery);
                }
            }
        }
        Ok(())
    }

    pub fn validate_http2(&self) -> Result<(), HttpValueError> {
        for forbidden in [
            "connection",
            "keep-alive",
            "proxy-connection",
            "transfer-encoding",
            "upgrade",
        ] {
            if self.fields.contains_key(forbidden) {
                return Err(HttpValueError::ForbiddenHttp2Header);
            }
        }
        if self
            .fields
            .get_all("te")
            .iter()
            .any(|value| match value.to_str() {
                Ok(text) => !text.eq_ignore_ascii_case("trailers"),
                Err(_) => true,
            })
        {
            return Err(HttpValueError::ForbiddenHttp2Header);
        }
        Ok(())
    }

    fn validated_pair(
        name: &str,
        value: &[u8],
    ) -> Result<(HeaderName, HeaderValue), HttpValueError> {
        if name.len().saturating_add(value.len()) > MAX_HTTP_HEADER_BYTES {
            return Err(HttpValueError::HeadersTooLarge);
        }
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpValueError::InvalidHeaderName)?;
        let mut value =
            HeaderValue::from_bytes(value).map_err(|_| HttpValueError::InvalidHeaderValue)?;
        if matches!(
            name.as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
        ) {
            value.set_sensitive(true);
        }
        Ok((name, value))
    }
}

fn validate_query_media_range(range: &str) -> Result<(), HttpValueError> {
    let (major, minor) = range
        .split_once('/')
        .ok_or(HttpValueError::InvalidAcceptQuery)?;
    if minor.contains('/') || major.is_empty() || minor.is_empty() {
        return Err(HttpValueError::InvalidAcceptQuery);
    }
    let token = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
    };
    if (major != "*" && !token(major))
        || (minor != "*" && !token(minor))
        || (major == "*" && minor != "*")
    {
        return Err(HttpValueError::InvalidAcceptQuery);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parsing_normalizes_origin_and_resolves_references() {
        let base = Url::parse("https://Example.com/a/b?old=1").unwrap();
        assert_eq!(base.origin(), "https://example.com:443");
        assert_eq!(
            base.resolve("../items?q=two").unwrap().as_str(),
            "https://example.com/items?q=two"
        );
        assert_eq!(
            base.resolve("//other.example/x").unwrap().origin(),
            "https://other.example:443"
        );
    }

    #[test]
    fn immutable_url_builders_escape_components_and_keep_the_source_unchanged() {
        let original = Url::parse("https://example.com/a%20b?old=1#part").unwrap();
        let queried = original.with_query("filter name", "a+b/c");
        assert_eq!(original.as_str(), "https://example.com/a%20b?old=1#part");
        assert_eq!(
            queried.as_str(),
            "https://example.com/a%20b?old=1&filter+name=a%2Bb%2Fc#part"
        );
        assert_eq!(
            queried.with_path("/new path").as_str(),
            "https://example.com/new%20path?old=1&filter+name=a%2Bb%2Fc#part"
        );
        assert_eq!(
            queried.without_query().as_str(),
            "https://example.com/a%20b#part"
        );
    }

    #[test]
    fn url_rejects_credentials_and_unsupported_schemes() {
        assert_eq!(
            Url::parse("https://user:pass@example.com/"),
            Err(HttpValueError::UrlCredentialsForbidden)
        );
        assert_eq!(
            Url::parse("file:///etc/passwd"),
            Err(HttpValueError::UnsupportedScheme)
        );
    }

    #[test]
    fn ipv6_origin_uses_bracketed_authority() {
        let url = Url::parse("http://[::1]:8080/health").unwrap();
        assert_eq!(url.origin(), "http://[::1]:8080");
    }

    #[test]
    fn headers_are_case_insensitive_and_preserve_set_cookie_repetition() {
        let mut headers = Headers::new();
        headers.append("Set-Cookie", b"a=1").unwrap();
        headers.append("set-cookie", b"b=2").unwrap();
        let cookies: Vec<_> = headers
            .get_all("SET-COOKIE")
            .unwrap()
            .map(|value| value.to_str().unwrap())
            .collect();
        assert_eq!(cookies, ["a=1", "b=2"]);
    }

    #[test]
    fn request_debug_redacts_standard_credential_headers() {
        let mut headers = Headers::new();
        headers
            .insert("Authorization", b"Bearer auth-token-private")
            .unwrap();
        headers
            .insert("Proxy-Authorization", b"Basic proxy-token-private")
            .unwrap();
        headers
            .insert("Cookie", b"session=cookie-token-private")
            .unwrap();
        headers
            .append("Set-Cookie", b"refresh=refresh-token-private; Secure")
            .unwrap();
        headers
            .insert("x-request-id", b"visible-request-id")
            .unwrap();
        let request = HttpRequestValues {
            method: HttpMethod::Get,
            method_token: "GET".to_owned(),
            url: Url::parse("https://example.com/").unwrap(),
            headers,
            body: Vec::new(),
        };

        let debug = format!("{request:?}");
        for private_value in [
            "auth-token-private",
            "proxy-token-private",
            "cookie-token-private",
            "refresh-token-private",
        ] {
            assert!(
                !debug.contains(private_value),
                "leaked {private_value}: {debug}"
            );
        }
        assert!(debug.contains("visible-request-id"));
    }

    #[test]
    fn custom_sensitive_marker_survives_append_and_replacement() {
        let mut headers = Headers::new();
        headers
            .append_sensitive("x-session-credential", b"initial-private")
            .unwrap();
        headers
            .append("x-session-credential", b"appended-private")
            .unwrap();
        assert!(
            headers
                .get_all("x-session-credential")
                .unwrap()
                .all(HeaderValue::is_sensitive)
        );

        headers
            .insert("x-session-credential", b"replacement-private")
            .unwrap();
        let replacement = headers.get("x-session-credential").unwrap();
        assert!(replacement.is_sensitive());
        let debug = format!("{headers:?}");
        for private_value in ["initial-private", "appended-private", "replacement-private"] {
            assert!(
                !debug.contains(private_value),
                "leaked {private_value}: {debug}"
            );
        }
    }

    #[test]
    fn removing_a_repeated_header_updates_limits_and_returns_first_value() {
        let mut headers = Headers::new();
        headers.append("content-encoding", b"gzip").unwrap();
        headers.append("Content-Encoding", b"br").unwrap();
        assert_eq!(headers.count, 2);
        assert!(headers.bytes > 0);
        assert_eq!(
            headers.remove("CONTENT-ENCODING").unwrap().as_bytes(),
            b"gzip"
        );
        assert_eq!(headers.count, 0);
        assert_eq!(headers.bytes, 0);
        assert!(headers.get("content-encoding").is_none());
    }

    #[test]
    fn headers_reject_forbidden_bytes_and_conflicting_framing() {
        let mut headers = Headers::new();
        assert_eq!(
            headers.append("bad name", b"x"),
            Err(HttpValueError::InvalidHeaderName)
        );
        assert_eq!(
            headers.append("x-value", b"bad\r\nvalue"),
            Err(HttpValueError::InvalidHeaderValue)
        );
        headers.append("content-length", b"5").unwrap();
        headers.append("content-length", b"6").unwrap();
        assert_eq!(
            headers.validate_message_framing(),
            Err(HttpValueError::ConflictingMessageFraming)
        );
    }

    #[test]
    fn repeated_header_fields_count_toward_the_shared_limit() {
        let mut headers = Headers::new();
        for _ in 0..MAX_HTTP_HEADER_COUNT {
            headers.append("set-cookie", b"x=1").unwrap();
        }
        assert_eq!(
            headers.append("set-cookie", b"x=2"),
            Err(HttpValueError::TooManyHeaders)
        );

        headers.insert("set-cookie", b"replacement=1").unwrap();
        assert!(headers.append("set-cookie", b"x=2").is_ok());
    }

    #[test]
    fn http2_rejects_connection_specific_fields() {
        let mut headers = Headers::new();
        headers.insert("connection", b"close").unwrap();
        assert_eq!(
            headers.validate_http2(),
            Err(HttpValueError::ForbiddenHttp2Header)
        );
    }

    #[test]
    fn query_is_safe_idempotent_and_requires_a_consistent_media_type() {
        assert!(HttpMethod::Query.is_safe());
        assert!(HttpMethod::Query.is_idempotent());
        assert!(!HttpMethod::Post.is_idempotent());
        assert!(!HttpMethod::Other.is_safe());
        assert!(!HttpMethod::Other.is_idempotent());

        let mut request = HttpRequestValues::new(
            HttpMethod::Query,
            Url::parse("https://example.com/search").unwrap(),
        );
        assert_eq!(
            request.validate_query(),
            Err(HttpValueError::QueryContentTypeRequired)
        );
        request.set_body(b"{\"q\":\"zirk\"}".to_vec()).unwrap();
        assert_eq!(
            request.validate_query(),
            Err(HttpValueError::QueryContentTypeRequired)
        );
        request
            .headers
            .append("content-type", b"application/json")
            .unwrap();
        request.validate_query().unwrap();
        request
            .headers
            .append("content-type", b"application/json")
            .unwrap();
        assert_eq!(
            request.validate_query(),
            Err(HttpValueError::QueryContentTypeConflict)
        );
        request
            .headers
            .insert("content-type", b"application/json")
            .unwrap();
        request
            .headers
            .append("Content-Type", b"text/plain")
            .unwrap();
        assert_eq!(
            request.validate_query(),
            Err(HttpValueError::QueryContentTypeConflict)
        );

        for wildcard in ["application/*", "*/*", "*/json"] {
            assert!(
                wildcard.parse::<mime::Mime>().is_ok(),
                "the generic MIME parser accepts media-range syntax: {wildcard}"
            );
            let mut request = HttpRequestValues::new(
                HttpMethod::Query,
                Url::parse("https://example.com/search").unwrap(),
            );
            request
                .headers
                .insert("content-type", wildcard.as_bytes())
                .unwrap();
            assert_eq!(
                request.validate_query(),
                Err(HttpValueError::InvalidHeaderValue),
                "QUERY Content-Type must identify a concrete media type: {wildcard}"
            );
        }
    }

    #[test]
    fn ordinary_http_client_rejects_connect_tunnel_semantics() {
        let request = HttpRequestValues::new(
            HttpMethod::Connect,
            Url::parse("https://example.com:443/").unwrap(),
        );
        assert_eq!(
            request.validate_client_request(),
            Err(HttpValueError::UnsupportedMethod)
        );

        let extension = HttpRequestValues {
            method: HttpMethod::Other,
            method_token: "connect".to_owned(),
            ..request
        };
        assert!(extension.validate_client_request().is_ok());
    }

    #[test]
    fn accept_query_uses_structured_field_lists_and_media_ranges() {
        // Absence is valid; an explicitly present empty value is malformed.
        Headers::new().validate_accept_query().unwrap();

        let mut headers = Headers::new();
        headers
            .append("Accept-Query", b"\"application/jsonpath\"")
            .unwrap();
        headers
            .append(
                "accept-query",
                b"application/sql;charset=\"UTF-8\", application/*, */*",
            )
            .unwrap();
        headers.validate_accept_query().unwrap();

        for invalid in &[
            &b""[..],
            &b"\"application/*\", */json"[..],
            &b"application/json;q=0.5"[..], // parameter values must be strings or tokens
            &b"(application/json)"[..],     // RFC 10008 members are items, not inner lists
            &b"\"application/json\","[..],
        ] {
            let mut headers = Headers::new();
            headers.insert("accept-query", invalid).unwrap();
            assert_eq!(
                headers.validate_accept_query(),
                Err(HttpValueError::InvalidAcceptQuery),
                "accepted invalid value: {}",
                String::from_utf8_lossy(invalid)
            );
        }

        let mut headers = Headers::new();
        headers.append("accept-query", b"").unwrap();
        headers.append("accept-query", b"application/json").unwrap();
        assert_eq!(
            headers.validate_accept_query(),
            Err(HttpValueError::InvalidAcceptQuery),
            "an empty first field line must not disappear when list fields are combined"
        );
    }

    #[test]
    fn buffered_http_bodies_have_a_hard_limit() {
        let mut request = HttpRequestValues::new(
            HttpMethod::Post,
            Url::parse("https://example.com/upload").unwrap(),
        );
        assert_eq!(
            request.set_body(vec![0; MAX_HTTP_BUFFERED_BODY_BYTES + 1]),
            Err(HttpValueError::BodyTooLarge)
        );
    }

    #[test]
    fn query_cache_key_includes_query_content() {
        let url = Url::parse("https://example.com/search").unwrap();
        let mut first = HttpRequestValues::new(HttpMethod::Query, url.clone());
        first.set_body(b"term=first".to_vec()).unwrap();
        let mut second = HttpRequestValues::new(HttpMethod::Query, url);
        second.set_body(b"term=second".to_vec()).unwrap();
        assert_ne!(first.cache_key(), second.cache_key());
    }

    #[test]
    fn query_cache_key_includes_content_type_metadata() {
        let url = Url::parse("https://example.com/search").unwrap();
        let mut json = HttpRequestValues::new(HttpMethod::Query, url.clone());
        json.set_body(b"same bytes".to_vec()).unwrap();
        json.headers
            .insert("content-type", b"application/json")
            .unwrap();
        let mut cbor = HttpRequestValues::new(HttpMethod::Query, url);
        cbor.set_body(b"same bytes".to_vec()).unwrap();
        cbor.headers
            .insert("content-type", b"application/cbor")
            .unwrap();

        assert_ne!(json.cache_key(), cbor.cache_key());
        assert_eq!(json.cache_key().len(), 32);
    }

    #[test]
    fn cache_variant_key_isolates_vary_values_and_normalizes_field_names() {
        let url = Url::parse("https://example.com/resource").unwrap();
        let mut json = HttpRequestValues::new(HttpMethod::Get, url.clone());
        json.headers.insert("accept", b"application/json").unwrap();
        json.headers
            .append("accept", b"application/problem+json")
            .unwrap();
        let mut text = HttpRequestValues::new(HttpMethod::Get, url);
        text.headers.insert("accept", b"text/plain").unwrap();

        assert_ne!(
            json.cache_variant_key(["Accept"]).unwrap(),
            text.cache_variant_key(["accept"]).unwrap()
        );
        assert_eq!(
            json.cache_variant_key(["ACCEPT", "accept"]).unwrap(),
            json.cache_variant_key(["accept"]).unwrap()
        );
        assert_eq!(json.cache_variant_key(["accept"]).unwrap().len(), 32);
    }

    #[test]
    fn cache_variant_key_rejects_vary_star_and_invalid_names() {
        let request = HttpRequestValues::new(
            HttpMethod::Get,
            Url::parse("https://example.com/resource").unwrap(),
        );
        assert!(request.cache_variant_key(["*"]).is_none());
        assert!(request.cache_variant_key(["bad name"]).is_none());
    }

    #[test]
    fn response_vary_fields_parse_repeated_lines_and_fail_closed() {
        let mut headers = Headers::new();
        headers.append("vary", b"Accept, Accept-Language").unwrap();
        headers.append("Vary", b"accept").unwrap();
        assert_eq!(
            headers.vary_fields().unwrap(),
            Some(vec!["accept".to_owned(), "accept-language".to_owned()])
        );

        let mut wildcard = Headers::new();
        wildcard.append("vary", b"Accept, *").unwrap();
        assert_eq!(wildcard.vary_fields().unwrap(), None);

        let mut malformed = Headers::new();
        malformed
            .append("vary", b"Accept,,Accept-Language")
            .unwrap();
        assert_eq!(
            malformed.vary_fields(),
            Err(HttpValueError::InvalidHeaderValue)
        );
    }

    #[test]
    fn response_vary_controls_request_variant_cache_key() {
        let url = Url::parse("https://example.com/resource").unwrap();
        let mut english = HttpRequestValues::new(HttpMethod::Get, url.clone());
        english.headers.insert("accept-language", b"en").unwrap();
        let mut french = HttpRequestValues::new(HttpMethod::Get, url);
        french.headers.insert("accept-language", b"fr").unwrap();
        let mut response_headers = Headers::new();
        response_headers.append("vary", b"Accept-Language").unwrap();

        assert_ne!(
            english
                .cache_variant_key_for_response(&response_headers)
                .unwrap(),
            french
                .cache_variant_key_for_response(&response_headers)
                .unwrap()
        );

        let mut star_headers = Headers::new();
        star_headers.append("vary", b"*").unwrap();
        assert_eq!(
            english
                .cache_variant_key_for_response(&star_headers)
                .unwrap(),
            None
        );
    }

    #[test]
    fn cache_variant_key_distinguishes_missing_and_present_request_fields() {
        let url = Url::parse("https://example.com/resource").unwrap();
        let missing = HttpRequestValues::new(HttpMethod::Get, url.clone());
        let mut present = HttpRequestValues::new(HttpMethod::Get, url);
        present.headers.insert("accept-language", b"en").unwrap();

        assert_ne!(
            missing.cache_variant_key(["accept-language"]).unwrap(),
            present.cache_variant_key(["accept-language"]).unwrap()
        );
    }

    #[test]
    fn extension_method_cache_key_uses_exact_method_token() {
        let url = Url::parse("https://example.com/resource").unwrap();
        let mut first = HttpRequestValues::new(HttpMethod::Other, url.clone());
        first.method_token = "M-SEARCH".to_owned();
        let mut second = HttpRequestValues::new(HttpMethod::Other, url);
        second.method_token = "PURGE".to_owned();
        assert_ne!(first.cache_key(), second.cache_key());
        assert!(!first.method.is_safe());
        assert!(!second.method.is_idempotent());
    }

    #[test]
    fn status_codes_are_range_validated_and_classified() {
        assert_eq!(HttpStatusCode::new(200).unwrap().as_u16(), 200);
        assert!(HttpStatusCode::new(404).unwrap().is_client_error());
        assert!(HttpStatusCode::new(503).unwrap().is_server_error());
        assert_eq!(
            HttpStatusCode::new(99),
            Err(HttpValueError::InvalidStatusCode)
        );
        assert_eq!(HttpStatusCode::new(600).unwrap().as_u16(), 600);
        assert_eq!(
            HttpStatusCode::new(1000),
            Err(HttpValueError::InvalidStatusCode)
        );
    }
}
