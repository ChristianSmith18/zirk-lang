//! Bounded private HTTP response cache primitives.
//!
//! The owning HTTP client supplies a principal/client partition. Cache keys
//! are never shared across partitions, responses with sensitive cookie or
//! authentication state are bypassed, and only explicitly fresh responses are
//! reused. Stale validator handling is deliberately left to the client layer.

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

use sha2::{Digest, Sha256};

use crate::http_values::{
    Headers, HttpMethod, HttpRequestValues, HttpStatusCode, MAX_HTTP_BUFFERED_BODY_BYTES,
};

const DEFAULT_MAX_ENTRIES: usize = 128;
const DEFAULT_MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CachedHttpResponse {
    pub status: HttpStatusCode,
    pub headers: Headers,
    pub body: Vec<u8>,
}

struct Entry {
    base_key: [u8; 32],
    target_key: [u8; 32],
    partition_key: [u8; 32],
    vary_fields: Vec<String>,
    variant_key: [u8; 32],
    response: CachedHttpResponse,
    inserted_at: Instant,
    expires_at: Instant,
    freshness_lifetime: Duration,
    corrected_initial_age: Duration,
    size_bytes: usize,
}

/// A bounded LRU cache partitioned by the caller's client/principal identity.
/// It stores only complete responses with explicit `max-age` freshness and
/// does not perform network I/O itself.
pub struct HttpResponseCache {
    entries: VecDeque<Entry>,
    current_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    max_entry_bytes: usize,
}

/// Returns whether a syntactically valid request cache policy contains the
/// `only-if-cached` directive. Malformed Cache-Control fields return `None` so
/// callers can fail closed instead of unexpectedly contacting the origin.
pub fn request_only_if_cached(request: &HttpRequestValues) -> Option<bool> {
    let directives = request_directives(request)?;
    if directives
        .iter()
        .any(|(name, argument)| name == "only-if-cached" && argument.is_some())
    {
        return None;
    }
    Some(
        directives
            .iter()
            .any(|(name, argument)| name == "only-if-cached" && argument.is_none()),
    )
}

impl Default for HttpResponseCache {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_ENTRIES,
            DEFAULT_MAX_BYTES,
            DEFAULT_MAX_ENTRY_BYTES,
        )
    }
}

impl HttpResponseCache {
    pub fn new(max_entries: usize, max_bytes: usize, max_entry_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            current_bytes: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            max_entry_bytes: max_entry_bytes
                .max(1)
                .min(max_bytes.max(1))
                .min(MAX_HTTP_BUFFERED_BODY_BYTES),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Returns a fresh matching response and updates its `Age` field. The
    /// opaque partition is supplied by the owning client and must identify its
    /// project/principal, proxy, and TLS configuration.
    pub fn get(
        &mut self,
        request: &HttpRequestValues,
        partition: &[u8],
        now: Instant,
    ) -> Option<CachedHttpResponse> {
        if partition.len() != 32
            || !request_cache_eligible(request)
            || request_forces_revalidation(request)
        {
            return None;
        }
        let requested_max_age = request_max_age(request)?;
        let min_fresh = request_min_fresh(request)?;
        let base_key = digest(&request.cache_key());
        let partition_key = digest(partition);

        let found = self.entries.iter().position(|entry| {
            if entry.base_key != base_key || entry.partition_key != partition_key {
                return false;
            }
            request
                .cache_variant_key(entry.vary_fields.iter().map(String::as_str))
                .is_some_and(|key| key.as_slice() == entry.variant_key)
        })?;
        let mut entry = self.entries.remove(found)?;
        let age = entry
            .corrected_initial_age
            .saturating_add(now.saturating_duration_since(entry.inserted_at));
        if requested_max_age.is_some_and(|limit| age > limit) {
            self.entries.push_back(entry);
            return None;
        }
        if age.saturating_add(min_fresh) >= entry.freshness_lifetime {
            self.entries.push_back(entry);
            return None;
        }

        let age_seconds = age.as_secs().to_string();
        if entry
            .response
            .headers
            .insert("age", age_seconds.as_bytes())
            .is_err()
        {
            self.entries.push_back(entry);
            return None;
        }
        let response = entry.response.clone();
        self.entries.push_back(entry);
        Some(response)
    }

    /// Returns a stale (or explicitly revalidated) representation with its
    /// validators, if the request can safely use conditional revalidation.
    pub fn revalidation_candidate(
        &mut self,
        request: &HttpRequestValues,
        partition: &[u8],
        now: Instant,
    ) -> Option<(CachedHttpResponse, Option<String>, Option<String>)> {
        if partition.len() != 32
            || !request_cache_eligible(request)
            || request.headers.get("if-none-match").is_some()
            || request.headers.get("if-modified-since").is_some()
            || request_has_no_store(request)
        {
            return None;
        }
        request_directives(request)?;
        let base_key = digest(&request.cache_key());
        let partition_key = digest(partition);
        let found = self.entries.iter().position(|entry| {
            entry.base_key == base_key
                && entry.partition_key == partition_key
                && request
                    .cache_variant_key(entry.vary_fields.iter().map(String::as_str))
                    .is_some_and(|key| key.as_slice() == entry.variant_key)
        })?;
        let entry = self.entries.remove(found)?;
        let age = entry
            .corrected_initial_age
            .saturating_add(now.saturating_duration_since(entry.inserted_at));
        let is_stale = age >= entry.freshness_lifetime || entry.expires_at <= now;
        let etag = entry
            .response
            .headers
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let last_modified = entry
            .response
            .headers
            .get("last-modified")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        self.entries.push_back(entry);
        (is_stale || request_forces_revalidation(request))
            .then_some((self.entries.back()?.response.clone(), etag, last_modified))
            .filter(|(_, etag, last_modified)| etag.is_some() || last_modified.is_some())
    }

    /// Stores one complete response when it is explicitly reusable by this
    /// private cache. The partition is hashed before storage, so project and
    /// proxy identifiers are not retained in cache entries.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        request: &HttpRequestValues,
        response: CachedHttpResponse,
        partition: &[u8],
        request_started: Instant,
        received_at: Instant,
        received_wall: SystemTime,
    ) -> bool {
        if partition.len() != 32
            || !request_cache_eligible(request)
            || request_directives(request).is_none()
            || request_has_no_store(request)
            || !response_cache_eligible(&response)
            || response.body.len() > self.max_entry_bytes
        {
            return false;
        }
        let Some(freshness) = response_freshness(&response.headers, received_wall) else {
            return false;
        };
        if freshness.is_zero() {
            return false;
        }
        let Some(vary_fields) = response.headers.vary_fields().ok().flatten() else {
            return false;
        };
        let Some(variant_key) = request.cache_variant_key(vary_fields.iter().map(String::as_str))
        else {
            return false;
        };
        let Ok(variant_key): Result<[u8; 32], _> = variant_key.try_into() else {
            return false;
        };
        let Some(corrected_age) = corrected_initial_age(
            &response.headers,
            request_started,
            received_at,
            received_wall,
        ) else {
            return false;
        };
        let remaining = freshness.saturating_sub(corrected_age);
        if remaining.is_zero() {
            return false;
        }
        let Some(expires_at) = received_at.checked_add(remaining) else {
            return false;
        };

        let size_bytes = response_size(&response, &vary_fields);
        if size_bytes > self.max_entry_bytes || size_bytes > self.max_bytes {
            return false;
        }

        let base_key = digest(&request.cache_key());
        let target_key = digest(request.url.as_str().as_bytes());
        let partition_key = digest(partition);
        self.remove_matching(base_key, partition_key, &vary_fields);
        while self.entries.len() >= self.max_entries
            || self.current_bytes.saturating_add(size_bytes) > self.max_bytes
        {
            if self.entries.is_empty() {
                return false;
            }
            self.remove(0);
        }

        self.current_bytes += size_bytes;
        self.entries.push_back(Entry {
            base_key,
            target_key,
            partition_key,
            vary_fields,
            variant_key,
            response,
            inserted_at: received_at,
            expires_at,
            freshness_lifetime: freshness,
            corrected_initial_age: corrected_age,
            size_bytes,
        });
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_bytes = 0;
    }

    /// Evicts every stored variant of one request in a caller partition.
    pub fn remove_request(&mut self, request: &HttpRequestValues, partition: &[u8]) {
        if partition.len() != 32 {
            return;
        }
        let base_key = digest(&request.cache_key());
        let partition_key = digest(partition);
        let mut index = 0;
        while index < self.entries.len() {
            if self.entries[index].base_key == base_key
                && self.entries[index].partition_key == partition_key
            {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    /// Invalidates every method/body/variant entry for a URI in one caller
    /// partition after a successful unsafe request.
    pub fn invalidate_uri(&mut self, url: &crate::http_values::Url, partition: &[u8]) {
        if partition.len() != 32 {
            return;
        }
        let target_key = digest(url.as_str().as_bytes());
        let partition_key = digest(partition);
        let mut index = 0;
        while index < self.entries.len() {
            if self.entries[index].target_key == target_key
                && self.entries[index].partition_key == partition_key
            {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn remove_matching(
        &mut self,
        base_key: [u8; 32],
        partition_key: [u8; 32],
        vary_fields: &[String],
    ) {
        let mut index = 0;
        while index < self.entries.len() {
            let entry = &self.entries[index];
            if entry.base_key == base_key
                && entry.partition_key == partition_key
                && entry.vary_fields == vary_fields
            {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn remove(&mut self, index: usize) {
        if let Some(entry) = self.entries.remove(index) {
            self.current_bytes = self.current_bytes.saturating_sub(entry.size_bytes);
        }
    }
}

fn request_cache_eligible(request: &HttpRequestValues) -> bool {
    matches!(
        request.method,
        HttpMethod::Get | HttpMethod::Head | HttpMethod::Query
    ) && request.headers.get("authorization").is_none()
        && request.headers.get("proxy-authorization").is_none()
        && request.headers.get("cookie").is_none()
        && !request
            .headers
            .iter()
            .any(|(_, value)| value.is_sensitive())
        && request.headers.get("if-none-match").is_none()
        && request.headers.get("if-modified-since").is_none()
}

fn request_has_no_store(request: &HttpRequestValues) -> bool {
    request
        .headers
        .get_all("cache-control")
        .into_iter()
        .flatten()
        .filter_map(|value| value.to_str().ok())
        .filter_map(split_directives)
        .flatten()
        .any(|directive| parse_directive(directive).0 == "no-store")
}

fn response_cache_eligible(response: &CachedHttpResponse) -> bool {
    let status = response.status.as_u16();
    !response.status.is_informational()
        && status != 206
        && status != 304
        && status != 401
        && status != 407
        && response.headers.get("set-cookie").is_none()
        && response_cache_control(&response.headers)
            .is_some_and(|control| !control.no_store && !control.no_cache)
}

fn request_forces_revalidation(request: &HttpRequestValues) -> bool {
    request
        .headers
        .get_all("cache-control")
        .into_iter()
        .flatten()
        .any(|value| {
            let Ok(value) = value.to_str() else {
                return true;
            };
            split_directives(value).is_none_or(|directives| {
                directives.into_iter().any(|directive| {
                    let (name, argument) = parse_directive(directive);
                    name == "no-store"
                        || name == "no-cache"
                        || (name == "max-age"
                            && argument
                                .as_deref()
                                .and_then(parse_delta_seconds)
                                .is_some_and(|duration| duration.is_zero()))
                })
            })
        })
}

fn request_max_age(request: &HttpRequestValues) -> Option<Option<Duration>> {
    let directives = request_directives(request)?;
    let values = directives
        .iter()
        .filter(|(name, _)| name == "max-age")
        .collect::<Vec<_>>();
    match values.as_slice() {
        [] => Some(None),
        [(_, Some(value))] => Some(Some(parse_delta_seconds(value)?)),
        _ => None,
    }
}

fn request_min_fresh(request: &HttpRequestValues) -> Option<Duration> {
    let directives = request_directives(request)?;
    let values = directives
        .iter()
        .filter(|(name, _)| name == "min-fresh")
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return None;
    }
    match values.first() {
        None => Some(Duration::ZERO),
        Some((_, value)) => Some(parse_delta_seconds(value.as_deref()?)?),
    }
}

fn request_directives(request: &HttpRequestValues) -> Option<Vec<(String, Option<String>)>> {
    parse_directives(&request.headers, "cache-control")
}

#[derive(Default)]
struct CacheControl {
    no_store: bool,
    no_cache: bool,
    max_age: Option<Duration>,
    malformed: bool,
}

fn response_cache_control(headers: &Headers) -> Option<CacheControl> {
    let directives = parse_directives(headers, "cache-control")?;
    let mut control = CacheControl::default();
    for (name, argument) in directives {
        match name.as_str() {
            "no-store" => control.no_store = true,
            "no-cache" => control.no_cache = true,
            "max-age" => {
                if control.max_age.is_some() {
                    control.malformed = true;
                }
                control.max_age = argument.as_deref().and_then(parse_delta_seconds);
                if control.max_age.is_none() {
                    control.malformed = true;
                }
            }
            "private" if argument.is_some() => control.malformed = true,
            _ => {}
        }
    }
    (!control.malformed).then_some(control)
}

fn response_freshness(headers: &Headers, received_wall: SystemTime) -> Option<Duration> {
    let control = response_cache_control(headers)?;
    if control.no_store || control.no_cache {
        return None;
    }
    if let Some(max_age) = control.max_age {
        return Some(max_age);
    }

    let expires_values = headers
        .get_all("expires")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if expires_values.len() != 1 {
        return None;
    }
    let expires = httpdate::parse_http_date(expires_values[0].to_str().ok()?).ok()?;

    let date_values = headers
        .get_all("date")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let base = match date_values.as_slice() {
        [] => received_wall,
        [value] => httpdate::parse_http_date(value.to_str().ok()?).ok()?,
        _ => return None,
    };
    Some(expires.duration_since(base).unwrap_or_default())
}

fn parse_directives(headers: &Headers, field_name: &str) -> Option<Vec<(String, Option<String>)>> {
    let mut parsed = Vec::new();
    for value in headers.get_all(field_name).into_iter().flatten() {
        let text = value.to_str().ok()?;
        for directive in split_directives(text)? {
            let (name, argument) = parse_directive(directive);
            if name.is_empty() {
                return None;
            }
            parsed.push((name, argument));
        }
    }
    Some(parsed)
}

fn split_directives(value: &str) -> Option<Vec<&str>> {
    let bytes = value.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
        } else if quoted && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        } else if byte == b',' && !quoted {
            parts.push(value[start..index].trim());
            start = index + 1;
        }
    }
    if quoted || escaped {
        return None;
    }
    parts.push(value[start..].trim());
    if parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(parts)
}

fn parse_directive(value: &str) -> (String, Option<String>) {
    let Some((name, argument)) = value.split_once('=') else {
        return (value.trim().to_ascii_lowercase(), None);
    };
    let argument = argument.trim();
    let argument = if argument.starts_with('"') && argument.ends_with('"') && argument.len() >= 2 {
        argument[1..argument.len() - 1].to_owned()
    } else {
        argument.to_owned()
    };
    (name.trim().to_ascii_lowercase(), Some(argument))
}

fn parse_delta_seconds(value: &str) -> Option<Duration> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(Duration::from_secs(value.parse().unwrap_or(u64::MAX)))
}

fn corrected_initial_age(
    headers: &Headers,
    request_started: Instant,
    received_at: Instant,
    received_wall: SystemTime,
) -> Option<Duration> {
    let age_values = headers
        .get_all("age")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if age_values.len() > 1 {
        return None;
    }
    let age_value = match age_values.first() {
        Some(value) => {
            Duration::from_secs(parse_delta_seconds(value.to_str().ok()?.trim())?.as_secs())
        }
        None => Duration::ZERO,
    };
    let date_values = headers
        .get_all("date")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if date_values.len() > 1 {
        return None;
    }
    let apparent_age = if let Some(value) = date_values.first() {
        let date = httpdate::parse_http_date(value.to_str().ok()?).ok()?;
        received_wall.duration_since(date).unwrap_or_default()
    } else {
        Duration::ZERO
    };
    let response_delay = received_at.saturating_duration_since(request_started);
    Some(apparent_age.max(age_value.saturating_add(response_delay)))
}

fn response_size(response: &CachedHttpResponse, vary_fields: &[String]) -> usize {
    let header_bytes = response
        .headers
        .iter()
        .map(|(name, value)| name.as_str().len().saturating_add(value.as_bytes().len()))
        .sum::<usize>();
    let vary_bytes = vary_fields.iter().map(String::len).sum::<usize>();
    response
        .body
        .len()
        .saturating_add(header_bytes)
        .saturating_add(vary_bytes)
        .saturating_add(128)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str) -> HttpRequestValues {
        HttpRequestValues::new(
            HttpMethod::Get,
            crate::http_values::Url::parse(url).unwrap(),
        )
    }

    fn response(headers: Headers, body: &[u8]) -> CachedHttpResponse {
        CachedHttpResponse {
            status: HttpStatusCode::new(200).unwrap(),
            headers,
            body: body.to_vec(),
        }
    }

    fn cache_control(value: &[u8]) -> Headers {
        let mut headers = Headers::new();
        headers.append("cache-control", value).unwrap();
        headers
    }

    #[test]
    fn only_if_cached_is_recognized_and_malformed_policy_is_rejected() {
        let mut hit_request = request("https://example.com/data");
        assert_eq!(request_only_if_cached(&hit_request), Some(false));
        hit_request
            .headers
            .append("cache-control", b"max-age=30, only-if-cached")
            .unwrap();
        assert_eq!(request_only_if_cached(&hit_request), Some(true));

        let mut malformed = request("https://example.com/data");
        malformed
            .headers
            .append("cache-control", b"only-if-cached=1")
            .unwrap();
        assert_eq!(request_only_if_cached(&malformed), None);
    }

    #[test]
    fn explicit_freshness_stores_and_returns_bounded_response_with_updated_age() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::new(4, 4096, 1024);
        let request = request("https://example.com/data");
        let headers = cache_control(b"public, max-age=60");
        assert!(cache.insert(
            &request,
            response(headers, b"payload"),
            &[7; 32],
            start,
            start + Duration::from_secs(1),
            SystemTime::UNIX_EPOCH + Duration::from_secs(10_000),
        ));
        assert_eq!(cache.len(), 1);
        assert!(cache.current_bytes() <= 4096);

        let cached = cache
            .get(&request, &[7; 32], start + Duration::from_secs(8))
            .unwrap();
        assert_eq!(cached.body, b"payload");
        assert_eq!(cached.headers.get("age").unwrap().to_str().unwrap(), "8");
    }

    #[test]
    fn expires_supplies_freshness_when_max_age_is_absent() {
        let start = Instant::now();
        let received_wall = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let received_at = start + Duration::from_secs(2);
        let mut headers = Headers::new();
        headers
            .insert(
                "expires",
                httpdate::fmt_http_date(received_wall + Duration::from_secs(60)).as_bytes(),
            )
            .unwrap();
        let mut cache = HttpResponseCache::default();
        let request = request("https://example.com/expires");

        assert!(cache.insert(
            &request,
            response(headers, b"expires-body"),
            &[3; 32],
            start,
            received_at,
            received_wall,
        ));
        assert_eq!(
            cache
                .get(&request, &[3; 32], received_at + Duration::from_secs(57))
                .unwrap()
                .body,
            b"expires-body"
        );
        assert!(
            cache
                .get(&request, &[3; 32], received_at + Duration::from_secs(58))
                .is_none()
        );
    }

    #[test]
    fn max_age_takes_precedence_over_expires_and_bad_dates_fail_closed() {
        let received_wall = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let mut headers = cache_control(b"max-age=7");
        headers
            .insert(
                "expires",
                httpdate::fmt_http_date(received_wall + Duration::from_secs(60)).as_bytes(),
            )
            .unwrap();
        assert_eq!(
            response_freshness(&headers, received_wall),
            Some(Duration::from_secs(7))
        );

        let mut invalid_expires = Headers::new();
        invalid_expires.insert("expires", b"not-a-date").unwrap();
        assert_eq!(response_freshness(&invalid_expires, received_wall), None);

        let mut ambiguous_expires = Headers::new();
        ambiguous_expires
            .insert("expires", b"Wed, 21 Oct 2037 07:28:00 GMT")
            .unwrap();
        ambiguous_expires
            .append("expires", b"Wed, 21 Oct 2038 07:28:00 GMT")
            .unwrap();
        assert_eq!(response_freshness(&ambiguous_expires, received_wall), None);
    }

    #[test]
    fn cache_is_partitioned_and_vary_values_must_match() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::default();
        let mut request = request("https://example.com/data");
        request.headers.insert("accept-language", b"en").unwrap();
        let mut headers = cache_control(b"max-age=60");
        headers.append("vary", b"Accept-Language").unwrap();
        assert!(cache.insert(
            &request,
            response(headers, b"hello"),
            &[1; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH + Duration::from_secs(10_000),
        ));
        assert!(cache.get(&request, &[2; 32], start).is_none());

        let mut french = request.clone();
        french.headers.insert("accept-language", b"fr").unwrap();
        assert!(cache.get(&french, &[1; 32], start).is_none());
        assert_eq!(cache.get(&request, &[1; 32], start).unwrap().body, b"hello");
    }

    #[test]
    fn query_cache_key_includes_all_content_metadata() {
        let mut compressed = HttpRequestValues::new(
            HttpMethod::Query,
            crate::http_values::Url::parse("https://example.com/search").unwrap(),
        );
        compressed.set_body(b"same-query-bytes".to_vec()).unwrap();
        compressed
            .headers
            .insert("content-type", b"application/query+json")
            .unwrap();
        compressed
            .headers
            .insert("content-encoding", b"gzip")
            .unwrap();
        compressed
            .headers
            .insert("content-language", b"en")
            .unwrap();

        let mut identity = compressed.clone();
        identity
            .headers
            .insert("content-encoding", b"identity")
            .unwrap();

        assert_ne!(compressed.cache_key(), identity.cache_key());

        let mut french = compressed.clone();
        french.headers.insert("content-language", b"fr").unwrap();
        assert_ne!(compressed.cache_key(), french.cache_key());
    }

    #[test]
    fn custom_sensitive_request_fields_are_never_cached_or_revalidated() {
        let now = Instant::now();
        let mut cache = HttpResponseCache::default();
        let mut request = request("https://example.com/private-search");
        request
            .headers
            .append_sensitive("x-session-credential", b"session-secret")
            .unwrap();

        assert!(!cache.insert(
            &request,
            response(cache_control(b"public, max-age=60"), b"private result"),
            &[5; 32],
            now,
            now,
            SystemTime::UNIX_EPOCH + Duration::from_secs(10_000),
        ));
        assert!(cache.get(&request, &[5; 32], now).is_none());
        assert!(
            cache
                .revalidation_candidate(&request, &[5; 32], now)
                .is_none()
        );
        assert!(cache.is_empty());
    }

    #[test]
    fn no_store_no_cache_auth_cookies_and_vary_star_are_not_reused() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::default();
        let request = request("https://example.com/data");
        for directive in [b"no-store, max-age=60".as_slice(), b"no-cache, max-age=60"] {
            assert!(!cache.insert(
                &request,
                response(cache_control(directive), b"private"),
                &[0; 32],
                start,
                start,
                SystemTime::UNIX_EPOCH,
            ));
        }
        let mut star = cache_control(b"max-age=60");
        star.append("vary", b"*").unwrap();
        assert!(!cache.insert(
            &request,
            response(star, b"wildcard"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));

        let mut authenticated = request.clone();
        authenticated
            .headers
            .insert("authorization", b"Bearer token")
            .unwrap();
        assert!(!cache.insert(
            &authenticated,
            response(cache_control(b"max-age=60"), b"secret"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));

        let mut set_cookie = cache_control(b"max-age=60");
        set_cookie.append("set-cookie", b"sid=one").unwrap();
        assert!(!cache.insert(
            &request,
            response(set_cookie, b"cookie"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));
        assert!(cache.is_empty());
    }

    #[test]
    fn response_age_and_request_delay_reduce_freshness() {
        let start = Instant::now();
        let received = start + Duration::from_secs(2);
        let mut headers = cache_control(b"max-age=60");
        headers.append("age", b"50").unwrap();
        let mut cache = HttpResponseCache::default();
        let request = request("https://example.com/data");
        assert!(cache.insert(
            &request,
            response(headers, b"aged"),
            &[0; 32],
            start,
            received,
            SystemTime::UNIX_EPOCH,
        ));
        assert_eq!(
            cache
                .get(&request, &[0; 32], received + Duration::from_secs(7))
                .unwrap()
                .body,
            b"aged"
        );
        assert!(
            cache
                .get(&request, &[0; 32], received + Duration::from_secs(9))
                .is_none()
        );
    }

    #[test]
    fn freshness_request_misses_do_not_evict_a_still_reusable_entry() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::default();
        let mut request = request("https://example.com/data");
        assert!(cache.insert(
            &request,
            response(cache_control(b"max-age=60"), b"body"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));
        request
            .headers
            .insert("cache-control", b"min-fresh=60")
            .unwrap();
        assert!(cache.get(&request, &[0; 32], start).is_none());
        assert_eq!(cache.len(), 1);
        request
            .headers
            .insert("cache-control", b"max-age=0")
            .unwrap();
        assert!(cache.get(&request, &[0; 32], start).is_none());
        assert_eq!(cache.len(), 1);
        request
            .headers
            .insert("cache-control", b"max-age=60")
            .unwrap();
        assert_eq!(cache.get(&request, &[0; 32], start).unwrap().body, b"body");
    }

    #[test]
    fn stale_entries_expose_validators_without_becoming_fresh_hits() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::default();
        let request = request("https://example.com/data");
        let mut headers = cache_control(b"max-age=1");
        headers.insert("etag", b"\"revision-1\"").unwrap();
        headers
            .insert("last-modified", b"Wed, 21 Oct 2015 07:28:00 GMT")
            .unwrap();
        assert!(cache.insert(
            &request,
            response(headers, b"cached-body"),
            &[9; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));
        let stale_at = start + Duration::from_secs(2);
        assert!(cache.get(&request, &[9; 32], stale_at).is_none());
        let (cached, etag, last_modified) = cache
            .revalidation_candidate(&request, &[9; 32], stale_at)
            .unwrap();
        assert_eq!(cached.body, b"cached-body");
        assert_eq!(etag.as_deref(), Some("\"revision-1\""));
        assert_eq!(
            last_modified.as_deref(),
            Some("Wed, 21 Oct 2015 07:28:00 GMT")
        );

        let mut explicit_validator = request.clone();
        explicit_validator
            .headers
            .insert("if-none-match", b"\"caller-value\"")
            .unwrap();
        assert!(
            cache
                .revalidation_candidate(&explicit_validator, &[9; 32], stale_at)
                .is_none()
        );
    }

    #[test]
    fn uri_invalidation_removes_all_method_variants_only_in_one_partition() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::default();
        let get = request("https://example.com/entity");
        let mut query = HttpRequestValues::new(
            HttpMethod::Query,
            crate::http_values::Url::parse("https://example.com/entity").unwrap(),
        );
        query
            .headers
            .insert("content-type", b"application/query+json")
            .unwrap();
        query.set_body(b"{}".to_vec()).unwrap();
        let other_uri = request("https://example.com/other");
        for (request, partition) in [
            (&get, &[1; 32][..]),
            (&query, &[1; 32][..]),
            (&get, &[2; 32][..]),
            (&other_uri, &[1; 32][..]),
        ] {
            assert!(cache.insert(
                request,
                response(cache_control(b"max-age=60"), b"body"),
                partition,
                start,
                start,
                SystemTime::UNIX_EPOCH,
            ));
        }
        assert_eq!(cache.len(), 4);
        cache.invalidate_uri(&get.url, &[1; 32]);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&get, &[1; 32], start).is_none());
        assert_eq!(cache.get(&get, &[2; 32], start).unwrap().body, b"body");
        assert_eq!(
            cache.get(&other_uri, &[1; 32], start).unwrap().body,
            b"body"
        );
    }

    #[test]
    fn capacity_and_clear_bound_memory() {
        let start = Instant::now();
        let mut cache = HttpResponseCache::new(1, 512, 256);
        let first = request("https://example.com/first");
        let second = request("https://example.com/second");
        assert!(cache.insert(
            &first,
            response(cache_control(b"max-age=60"), b"first"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));
        assert!(cache.insert(
            &second,
            response(cache_control(b"max-age=60"), b"second"),
            &[0; 32],
            start,
            start,
            SystemTime::UNIX_EPOCH,
        ));
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&first, &[0; 32], start).is_none());
        assert_eq!(cache.get(&second, &[0; 32], start).unwrap().body, b"second");
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.current_bytes(), 0);
    }
}
