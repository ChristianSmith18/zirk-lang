//! Platform-neutral validation and decoding rules for `std.environment`.
//!
//! The native runtime supplies the process environment and enforces the
//! effective permission before calling these helpers. Keeping validation and
//! decoding here lets the runtime and deterministic fixtures share semantics.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};

/// Maximum decoded environment value accepted by the standard library.
pub const MAX_ENVIRONMENT_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentValueError {
    InvalidName,
    Missing,
    PermissionDenied,
    InvalidEncoding,
    ValueTooLarge,
    Platform,
}

/// The environment portion of an already authenticated effective runtime
/// grant. Construction belongs to the trusted host after approval validation;
/// Zirk source and package code never construct grants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveEnvironmentGrant {
    names: BTreeSet<String>,
    secret_names: BTreeSet<String>,
    list_names: bool,
}

impl EffectiveEnvironmentGrant {
    /// Creates policy data from an authenticated host grant. This is not an
    /// approval mechanism and must only receive scopes from the trusted grant
    /// verifier.
    pub fn from_verified_scopes(
        names: impl IntoIterator<Item = String>,
        secret_names: impl IntoIterator<Item = String>,
        list_names: bool,
    ) -> Self {
        Self {
            names: names.into_iter().collect(),
            secret_names: secret_names.into_iter().collect(),
            list_names,
        }
    }

    /// Checks a scope using the host's environment-name comparison rule.
    /// Windows callers use the same ordinal case-insensitive comparison as
    /// the Win32 environment APIs; Unix callers use exact, case-sensitive
    /// comparison.
    pub fn allows_name_by(
        &self,
        name: &str,
        secret: bool,
        names_equal: fn(&str, &str) -> bool,
    ) -> bool {
        let contains =
            |scopes: &BTreeSet<String>| scopes.iter().any(|scope| names_equal(scope, name));

        if secret {
            contains(&self.secret_names)
        } else {
            contains(&self.names)
        }
    }

    pub fn allows_listing_name_by(&self, name: &str, names_equal: fn(&str, &str) -> bool) -> bool {
        let contains =
            |scopes: &BTreeSet<String>| scopes.iter().any(|scope| names_equal(scope, name));

        contains(&self.names) || contains(&self.secret_names)
    }
}

#[cfg(windows)]
fn environment_names_equal(left: &str, right: &str) -> bool {
    windows_environment_name_key(left) == windows_environment_name_key(right)
}

#[cfg(any(windows, test))]
fn windows_environment_name_key(value: &str) -> String {
    let mut key = String::with_capacity(value.len());
    for character in value.chars() {
        let mut uppercase = character.to_uppercase();
        let Some(mapped) = uppercase.next() else {
            key.push(character);
            continue;
        };
        // Avoid merging identifiers through Unicode multi-character casing
        // expansions (for example, ß becoming "SS"). Keep that scalar as-is
        // while still comparing the other name characters case-insensitively.
        if uppercase.next().is_some() {
            key.push(character);
            continue;
        }
        key.push(mapped);
    }
    key
}

#[cfg(not(windows))]
fn environment_names_equal(left: &str, right: &str) -> bool {
    left == right
}

/// Environment names are non-empty and cannot contain NUL or `=` on any
/// supported target. The NUL restriction also matches the host process APIs.
pub fn validate_name(name: &str) -> Result<(), EnvironmentValueError> {
    if name.is_empty() || name.contains('\0') || name.contains('=') {
        return Err(EnvironmentValueError::InvalidName);
    }
    Ok(())
}

/// Decodes a host value strictly. Invalid host text is an error, never lossy
/// replacement text. The size check happens on the native representation
/// before allocating the decoded `String`.
pub fn decode_value(value: OsString) -> Result<String, EnvironmentValueError> {
    // `OsString::into_string` may transcode on Windows and release its source
    // allocation without erasing it. Keep the encoded host copy in a
    // zeroizing buffer while making the managed-language value.
    let mut encoded = zeroize::Zeroizing::new(value.into_encoded_bytes());
    decode_encoded_value(&mut encoded)
}

fn decode_encoded_value(encoded: &mut Vec<u8>) -> Result<String, EnvironmentValueError> {
    if encoded.len() > MAX_ENVIRONMENT_VALUE_BYTES {
        zeroize::Zeroize::zeroize(encoded);
        return Err(EnvironmentValueError::ValueTooLarge);
    }
    let decoded = std::str::from_utf8(encoded)
        .map(str::to_owned)
        .map_err(|_| EnvironmentValueError::InvalidEncoding);
    zeroize::Zeroize::zeroize(encoded);
    decoded
}

fn validate_os_value(value: &OsStr) -> Result<(), EnvironmentValueError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = value.as_bytes();
        if bytes.len() > MAX_ENVIRONMENT_VALUE_BYTES {
            return Err(EnvironmentValueError::ValueTooLarge);
        }
        std::str::from_utf8(bytes).map_err(|_| EnvironmentValueError::InvalidEncoding)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        checked_utf8_length(char::decode_utf16(value.encode_wide()))?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let decoded = value
            .to_str()
            .ok_or(EnvironmentValueError::InvalidEncoding)?;
        checked_utf8_length(decoded.chars().map(Ok::<char, std::convert::Infallible>))?;
    }
    Ok(())
}

/// Erases the temporary copy of a value when an operation needs only its
/// presence or the corresponding variable name. The operating system's own
/// environment storage is outside this process-local guarantee.
fn zeroize_os_string(value: OsString) {
    drop(zeroize::Zeroizing::new(value.into_encoded_bytes()));
}

#[cfg(any(windows, test, not(unix)))]
fn checked_utf8_length<E>(
    characters: impl IntoIterator<Item = Result<char, E>>,
) -> Result<usize, EnvironmentValueError> {
    let mut bytes = 0usize;
    for decoded in characters {
        let character = decoded.map_err(|_| EnvironmentValueError::InvalidEncoding)?;
        bytes = bytes
            .checked_add(character.len_utf8())
            .ok_or(EnvironmentValueError::ValueTooLarge)?;
        if bytes > MAX_ENVIRONMENT_VALUE_BYTES {
            return Err(EnvironmentValueError::ValueTooLarge);
        }
    }
    Ok(bytes)
}

/// Looks up a validated name in an injected snapshot. Production callers must
/// check authority before obtaining/passing the snapshot, so denial cannot
/// reveal whether the name exists.
pub fn lookup<I>(name: &str, values: I) -> Result<String, EnvironmentValueError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    validate_name(name)?;
    let mut value = None;
    for (candidate, candidate_value) in values {
        let matches = candidate
            .into_string()
            .is_ok_and(|candidate| environment_names_equal(&candidate, name));
        if matches && value.is_none() {
            value = Some(candidate_value);
        } else {
            // Snapshot lookup owns every copied process value, not only the
            // value returned to the caller. Erase unrelated and duplicate
            // entries before continuing so a secret in another variable is
            // not left in a dropped OsString allocation.
            zeroize_os_string(candidate_value);
        }
    }
    decode_value(value.ok_or(EnvironmentValueError::Missing)?)
}

/// Checks the exact effective scope before calling `snapshot`. This ordering
/// prevents a denied request from observing whether the variable exists.
pub fn lookup_authorized<F, I>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    snapshot: F,
) -> Result<String, EnvironmentValueError>
where
    F: FnOnce() -> I,
    I: IntoIterator<Item = (OsString, OsString)>,
{
    lookup_authorized_fallible(name, secret, grant, || Ok(snapshot()))
}

/// Reads a single authorized value through the host's direct name lookup.
/// This avoids enumerating and cloning unrelated environment variables for
/// the common `Env.get` path. Authority and name validation happen before
/// `read_value` is invoked.
pub fn get_authorized_value<F>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    read_value: F,
) -> Result<String, EnvironmentValueError>
where
    F: FnOnce() -> Result<Option<OsString>, EnvironmentValueError>,
{
    get_authorized_value_by(name, secret, grant, environment_names_equal, read_value)
}

pub fn get_authorized_value_by<F>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    names_equal: fn(&str, &str) -> bool,
    read_value: F,
) -> Result<String, EnvironmentValueError>
where
    F: FnOnce() -> Result<Option<OsString>, EnvironmentValueError>,
{
    validate_name(name)?;
    if !grant.allows_name_by(name, secret, names_equal) {
        return Err(EnvironmentValueError::PermissionDenied);
    }
    let value = read_value()?.ok_or(EnvironmentValueError::Missing)?;
    decode_value(value)
}

/// Fallible host adapter for environment lookup. The permission check happens
/// before invoking the host, and host failures retain their typed category
/// rather than being reported as a missing variable.
pub fn lookup_authorized_fallible<F, I>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    snapshot: F,
) -> Result<String, EnvironmentValueError>
where
    F: FnOnce() -> Result<I, EnvironmentValueError>,
    I: IntoIterator<Item = (OsString, OsString)>,
{
    lookup_authorized_fallible_by(name, secret, grant, environment_names_equal, snapshot)
}

pub fn lookup_authorized_fallible_by<F, I>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    names_equal: fn(&str, &str) -> bool,
    snapshot: F,
) -> Result<String, EnvironmentValueError>
where
    F: FnOnce() -> Result<I, EnvironmentValueError>,
    I: IntoIterator<Item = (OsString, OsString)>,
{
    validate_name(name)?;
    if !grant.allows_name_by(name, secret, names_equal) {
        return Err(EnvironmentValueError::PermissionDenied);
    }
    lookup(name, snapshot()?)
}

/// Checks authorized presence without materializing the value as a `String`.
/// Permission validation occurs before `read_value`, so denied names cannot
/// be used as an existence oracle.
pub fn contains_authorized<F>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    read_value: F,
) -> Result<bool, EnvironmentValueError>
where
    F: FnOnce() -> Result<Option<OsString>, EnvironmentValueError>,
{
    contains_authorized_by(name, secret, grant, environment_names_equal, read_value)
}

pub fn contains_authorized_by<F>(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
    names_equal: fn(&str, &str) -> bool,
    read_value: F,
) -> Result<bool, EnvironmentValueError>
where
    F: FnOnce() -> Result<Option<OsString>, EnvironmentValueError>,
{
    validate_name(name)?;
    if !grant.allows_name_by(name, secret, names_equal) {
        return Err(EnvironmentValueError::PermissionDenied);
    }
    let Some(value) = read_value()? else {
        return Ok(false);
    };
    let validated = validate_os_value(&value);
    zeroize_os_string(value);
    validated?;
    Ok(true)
}

/// Returns valid UTF-8 names from an injected snapshot. Callers filter these
/// names through the exact effective grant before exposing them.
pub fn names<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut decoded_names = Vec::new();
    for (name, value) in values {
        zeroize_os_string(value);
        if let Ok(name) = name.into_string()
            && validate_name(&name).is_ok()
        {
            decoded_names.push(name);
        }
    }
    decoded_names
}

/// Applies names-only listing authority. An explicit broad grant exposes all
/// valid names; otherwise only exact names already granted for ordinary reads
/// are returned. Values are discarded in either case.
pub fn names_authorized<F, I>(
    grant: &EffectiveEnvironmentGrant,
    snapshot: F,
) -> Result<Vec<String>, EnvironmentValueError>
where
    F: FnOnce() -> I,
    I: IntoIterator<Item = (OsString, OsString)>,
{
    names_authorized_by(grant, environment_names_equal, snapshot)
}

pub fn names_authorized_by<F, I>(
    grant: &EffectiveEnvironmentGrant,
    names_equal: fn(&str, &str) -> bool,
    snapshot: F,
) -> Result<Vec<String>, EnvironmentValueError>
where
    F: FnOnce() -> I,
    I: IntoIterator<Item = (OsString, OsString)>,
{
    if !grant.list_names && grant.names.is_empty() && grant.secret_names.is_empty() {
        return Err(EnvironmentValueError::PermissionDenied);
    }
    let names = names(snapshot());
    let mut visible_names = if grant.list_names {
        names
    } else {
        names
            .into_iter()
            .filter(|name| grant.allows_listing_name_by(name, names_equal))
            .collect()
    };
    visible_names.sort_unstable();
    Ok(visible_names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect()
    }

    #[cfg(unix)]
    fn invalid_os_string() -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xFF])
    }

    #[cfg(windows)]
    fn invalid_os_string() -> OsString {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xD800])
    }

    #[test]
    fn names_reject_empty_nul_and_equal_sign() {
        assert_eq!(validate_name(""), Err(EnvironmentValueError::InvalidName));
        assert_eq!(
            validate_name("A\0B"),
            Err(EnvironmentValueError::InvalidName)
        );
        assert_eq!(
            validate_name("A=B"),
            Err(EnvironmentValueError::InvalidName)
        );
        assert_eq!(validate_name("APP_MODE"), Ok(()));
    }

    #[test]
    fn lookup_distinguishes_missing_and_decodes_valid_values() {
        assert_eq!(
            lookup("MODE", vars(&[("MODE", "release")])),
            Ok("release".into())
        );
        assert_eq!(
            lookup("MISSING", vars(&[])),
            Err(EnvironmentValueError::Missing)
        );
    }

    #[test]
    fn snapshot_lookup_keeps_the_first_match_and_ignores_invalid_names() {
        let values = vec![
            (invalid_os_string(), OsString::from("unrelated secret")),
            (OsString::from("MODE"), OsString::from("first")),
            (OsString::from("MODE"), OsString::from("duplicate secret")),
        ];
        assert_eq!(lookup("MODE", values), Ok("first".into()));
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn lookup_does_not_replace_invalid_utf8() {
        let invalid = invalid_os_string();
        assert_eq!(
            decode_value(invalid),
            Err(EnvironmentValueError::InvalidEncoding)
        );
    }

    #[test]
    fn decoding_zeroizes_the_encoded_host_copy_on_success_and_failure() {
        let mut valid = b"sensitive-value".to_vec();
        assert_eq!(
            decode_encoded_value(&mut valid),
            Ok("sensitive-value".into())
        );
        assert!(valid.iter().all(|byte| *byte == 0));

        let mut invalid = vec![0xff, 0xfe];
        assert_eq!(
            decode_encoded_value(&mut invalid),
            Err(EnvironmentValueError::InvalidEncoding)
        );
        assert!(invalid.iter().all(|byte| *byte == 0));

        let mut oversized = vec![b'x'; MAX_ENVIRONMENT_VALUE_BYTES + 1];
        assert_eq!(
            decode_encoded_value(&mut oversized),
            Err(EnvironmentValueError::ValueTooLarge)
        );
        assert!(oversized.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn oversized_host_values_have_a_distinct_typed_error() {
        let oversized = OsString::from("x".repeat(MAX_ENVIRONMENT_VALUE_BYTES + 1));
        assert_eq!(
            decode_value(oversized),
            Err(EnvironmentValueError::ValueTooLarge)
        );
    }

    #[test]
    fn host_value_at_the_byte_limit_is_accepted() {
        let value = OsString::from("x".repeat(MAX_ENVIRONMENT_VALUE_BYTES));
        let decoded = decode_value(value).unwrap();
        assert_eq!(decoded.len(), MAX_ENVIRONMENT_VALUE_BYTES);
    }

    #[test]
    fn utf8_encoded_environment_values_obey_byte_limit() {
        let encoded = std::iter::repeat_n(Ok::<char, ()>('é'), MAX_ENVIRONMENT_VALUE_BYTES / 2 + 1);
        assert_eq!(
            checked_utf8_length(encoded),
            Err(EnvironmentValueError::ValueTooLarge)
        );
        assert_eq!(
            checked_utf8_length([Err::<char, _>(())]),
            Err(EnvironmentValueError::InvalidEncoding)
        );
    }

    #[test]
    fn names_return_only_decodable_names() {
        assert_eq!(names(vars(&[("A", "1"), ("B", "2")])), vec!["A", "B"]);
    }

    #[test]
    fn denied_lookup_never_inspects_the_environment_snapshot() {
        let grant = EffectiveEnvironmentGrant::default();
        let mut inspected = false;
        let result = lookup_authorized("SECRET", false, &grant, || {
            inspected = true;
            vars(&[("SECRET", "value")])
        });
        assert_eq!(result, Err(EnvironmentValueError::PermissionDenied));
        assert!(!inspected);
    }

    #[test]
    fn direct_lookup_validates_authority_before_querying_host() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["APP_MODE".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            get_authorized_value("APP_MODE", false, &grant, || {
                Ok(Some(OsString::from("release")))
            }),
            Ok("release".into())
        );
        assert_eq!(
            get_authorized_value("APP_MODE", false, &grant, || Ok(None)),
            Err(EnvironmentValueError::Missing)
        );

        let mut queried = false;
        assert_eq!(
            get_authorized_value("SECRET", false, &grant, || {
                queried = true;
                Ok(Some(OsString::from("hidden")))
            }),
            Err(EnvironmentValueError::PermissionDenied)
        );
        assert!(!queried);
    }

    #[test]
    fn direct_lookup_preserves_host_errors_and_invalid_encoding() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["APP_MODE".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            get_authorized_value("APP_MODE", false, &grant, || {
                Err(EnvironmentValueError::Platform)
            }),
            Err(EnvironmentValueError::Platform)
        );
        assert_eq!(
            get_authorized_value("APP_MODE", false, &grant, || {
                Ok(Some(invalid_os_string()))
            }),
            Err(EnvironmentValueError::InvalidEncoding)
        );
    }

    #[test]
    fn fallible_host_lookup_preserves_platform_failure_after_authorization() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["APP_MODE".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            lookup_authorized_fallible(
                "APP_MODE",
                false,
                &grant,
                || -> Result<Vec<(OsString, OsString)>, EnvironmentValueError> {
                    Err(EnvironmentValueError::Platform)
                },
            ),
            Err(EnvironmentValueError::Platform)
        );

        let denied = lookup_authorized_fallible(
            "APP_MODE",
            false,
            &EffectiveEnvironmentGrant::default(),
            || -> Result<Vec<(OsString, OsString)>, EnvironmentValueError> {
                panic!("host lookup must not run before authorization")
            },
        );
        assert_eq!(denied, Err(EnvironmentValueError::PermissionDenied));
    }

    #[test]
    fn denied_contains_never_queries_the_host_value() {
        let mut queried = false;
        assert_eq!(
            contains_authorized(
                "SECRET",
                false,
                &EffectiveEnvironmentGrant::default(),
                || {
                    queried = true;
                    Ok(Some(OsString::from("present")))
                }
            ),
            Err(EnvironmentValueError::PermissionDenied)
        );
        assert!(!queried);
    }

    #[test]
    fn contains_distinguishes_absence_and_invalid_host_encoding() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["SETTING".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            contains_authorized("SETTING", false, &grant, || Ok(None)),
            Ok(false)
        );
        assert_eq!(
            contains_authorized("SETTING", false, &grant, || {
                Ok(Some(invalid_os_string()))
            }),
            Err(EnvironmentValueError::InvalidEncoding)
        );
        assert_eq!(
            contains_authorized("SETTING", false, &grant, || {
                Ok(Some(OsString::from("present")))
            }),
            Ok(true)
        );
    }

    #[test]
    fn secret_reads_require_the_secret_scope_independently() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["TOKEN".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            lookup_authorized("TOKEN", true, &grant, || vars(&[("TOKEN", "value")])),
            Err(EnvironmentValueError::PermissionDenied)
        );
        assert_eq!(
            lookup_authorized("TOKEN", false, &grant, || vars(&[("TOKEN", "value")])),
            Ok("value".into())
        );

        let secret_grant = EffectiveEnvironmentGrant::from_verified_scopes(
            std::iter::empty(),
            ["TOKEN".into()],
            false,
        );
        assert_eq!(
            lookup_authorized("TOKEN", true, &secret_grant, || vars(&[("TOKEN", "value")])),
            Ok("value".into())
        );
        assert_eq!(
            lookup_authorized("TOKEN", false, &secret_grant, || vars(&[(
                "TOKEN", "value"
            )])),
            Err(EnvironmentValueError::PermissionDenied)
        );
    }

    #[test]
    fn broad_listing_returns_names_only() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["VISIBLE".into()],
            std::iter::empty(),
            true,
        );
        assert_eq!(
            names_authorized(&grant, || {
                vars(&[("VISIBLE", "secret"), ("HIDDEN", "hidden")])
            }),
            Ok(vec!["HIDDEN".into(), "VISIBLE".into()])
        );
    }

    #[test]
    fn narrow_listing_returns_only_exactly_authorized_names() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["VISIBLE".into(), "ALPHA".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            names_authorized(&grant, || {
                vars(&[
                    ("VISIBLE", "visible-value"),
                    ("HIDDEN", "hidden-value"),
                    ("ALPHA", "alpha-value"),
                ])
            }),
            Ok(vec!["ALPHA".into(), "VISIBLE".into()])
        );
    }

    fn ascii_case_insensitive(left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }

    #[test]
    fn host_name_comparator_controls_reads_and_name_listing() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["APP_MODE".into()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            get_authorized_value_by("app_mode", false, &grant, ascii_case_insensitive, || {
                Ok(Some(OsString::from("release")))
            }),
            Ok("release".into())
        );
        assert_eq!(
            names_authorized_by(&grant, ascii_case_insensitive, || {
                vars(&[("app_mode", "discard-me"), ("OTHER", "discard-me")])
            }),
            Ok(vec!["app_mode".into()])
        );
    }

    #[test]
    fn secret_scope_authorizes_listing_its_name_without_ordinary_read() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            std::iter::empty(),
            ["TOKEN".into()],
            false,
        );
        assert_eq!(
            names_authorized(&grant, || vars(&[("TOKEN", "secret"), ("OTHER", "value")])),
            Ok(vec!["TOKEN".into()])
        );
    }

    #[test]
    #[cfg(windows)]
    fn windows_scope_matching_is_unicode_case_insensitive() {
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            ["APP_MODE".into()],
            ["ΣERVICE_TOKEN".into()],
            false,
        );

        assert!(grant.allows_name_by("app_mode", false, environment_names_equal));
        assert!(grant.allows_name_by("σervice_token", true, environment_names_equal));
        assert!(!grant.allows_name_by("OTHER", false, environment_names_equal));
        assert!(!grant.allows_name_by("APP_MODE", true, environment_names_equal));
        assert_eq!(
            names_authorized(&grant, || vars(&[
                ("app_Mode", "value"),
                ("OTHER", "hidden")
            ])),
            Ok(vec!["app_Mode".into()])
        );
    }

    #[test]
    fn windows_name_key_normalizes_around_multicharacter_case_expansions() {
        assert_eq!(
            windows_environment_name_key("ßervice_TOKEN"),
            windows_environment_name_key("ßERVICE_token")
        );
        assert_ne!(
            windows_environment_name_key("ß"),
            windows_environment_name_key("SS")
        );
    }

    #[test]
    fn denied_listing_never_inspects_the_environment_snapshot() {
        let grant = EffectiveEnvironmentGrant::default();
        let mut inspected = false;
        let result = names_authorized(&grant, || {
            inspected = true;
            vars(&[("SECRET", "value")])
        });
        assert_eq!(result, Err(EnvironmentValueError::PermissionDenied));
        assert!(!inspected);
    }
}
