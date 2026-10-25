//! Permission-gated access to the current process environment.
//!
//! The grant must be obtained from the trusted approval verifier. The lookup
//! path validates authority before asking the host for its environment
//! snapshot, so a denied name cannot be used as an existence oracle.

use std::ffi::c_void;

use std::sync::{OnceLock, RwLock};
use zeroize::Zeroizing;
pub use zirk_runtime_core::environment::{
    EffectiveEnvironmentGrant, EnvironmentValueError, MAX_ENVIRONMENT_VALUE_BYTES,
};

static EFFECTIVE_GRANT: OnceLock<RwLock<Option<EffectiveEnvironmentGrant>>> = OnceLock::new();

/// Uses the platform's process-environment name comparison semantics for both
/// scope checks and host lookups.
#[cfg(windows)]
fn environment_names_equal(left: &str, right: &str) -> bool {
    use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};

    let left: Vec<u16> = left.encode_utf16().collect();
    let right: Vec<u16> = right.encode_utf16().collect();
    let (Ok(left_len), Ok(right_len)) = (i32::try_from(left.len()), i32::try_from(right.len()))
    else {
        return false;
    };
    // SAFETY: both buffers remain alive for the call and their explicit
    // lengths are UTF-16 code-unit counts, as required by CompareStringOrdinal.
    unsafe {
        CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) == CSTR_EQUAL
    }
}

#[cfg(not(windows))]
fn environment_names_equal(left: &str, right: &str) -> bool {
    left == right
}

fn grant_lock() -> &'static RwLock<Option<EffectiveEnvironmentGrant>> {
    EFFECTIVE_GRANT.get_or_init(|| RwLock::new(None))
}

/// Installs or replaces the grant after the trusted launcher has authenticated
/// the external approval record. Replacing it takes effect before the next
/// synchronous environment operation.
pub fn install_effective_grant(grant: EffectiveEnvironmentGrant) {
    *grant_lock()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(grant);
}

/// Revokes the process environment grant before the next lookup.
pub fn revoke_effective_grant() {
    *grant_lock()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

fn with_effective_grant<T>(
    operation: impl FnOnce(&EffectiveEnvironmentGrant) -> Result<T, EnvironmentValueError>,
) -> Result<T, EnvironmentValueError> {
    ensure_process_grant();
    let guard = grant_lock()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match guard.as_ref() {
        Some(grant) => operation(grant),
        None => Err(EnvironmentValueError::PermissionDenied),
    }
}

/// Reads one authorized environment variable without changing process state.
pub fn get(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
) -> Result<String, EnvironmentValueError> {
    zirk_runtime_core::environment::get_authorized_value_by(
        name,
        secret,
        grant,
        environment_names_equal,
        || read_process_environment_value(name),
    )
}

/// Checks authorized presence without allocating the value as a `String`.
pub fn contains(
    name: &str,
    secret: bool,
    grant: &EffectiveEnvironmentGrant,
) -> Result<bool, EnvironmentValueError> {
    zirk_runtime_core::environment::contains_authorized_by(
        name,
        secret,
        grant,
        environment_names_equal,
        || read_process_environment_value(name),
    )
}

#[cfg(not(windows))]
fn read_process_environment_value(
    name: &str,
) -> Result<Option<std::ffi::OsString>, EnvironmentValueError> {
    Ok(std::env::var_os(name))
}

/// Windows' standard-library lookup exposes only `Option`, which loses the
/// Win32 distinction between a missing variable and a failed host query. Keep
/// that distinction so callers receive `Missing` only for error 203 and
/// `Platform` for other OS failures.
#[cfg(windows)]
fn read_process_environment_value(
    name: &str,
) -> Result<Option<std::ffi::OsString>, EnvironmentValueError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError},
        System::Environment::GetEnvironmentVariableW,
    };

    const MAX_QUERY_ATTEMPTS: u8 = 3;

    let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    for _ in 0..MAX_QUERY_ATTEMPTS {
        unsafe { SetLastError(0) };
        let required = unsafe { GetEnvironmentVariableW(name.as_ptr(), std::ptr::null_mut(), 0) };
        if required == 0 {
            return classify_windows_zero_result(unsafe { GetLastError() });
        }

        // The size returned for an undersized buffer includes its terminating
        // NUL. UTF-8 byte length is never smaller than the UTF-16 unit count,
        // so this rejects definitely oversized values before allocating.
        let buffer_len = checked_windows_environment_buffer_len(required)?;
        // The wide buffer is a separate copy of the host value. Keep it
        // zeroizing across success, retries, and native API error paths.
        let mut buffer = zeroize::Zeroizing::new(vec![0u16; buffer_len]);
        unsafe { SetLastError(0) };
        let written = unsafe {
            GetEnvironmentVariableW(name.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32)
        };
        if written == 0 {
            return classify_windows_zero_result(unsafe { GetLastError() });
        }
        if written as usize >= buffer.len() {
            // The value changed between the size query and read. Re-query a
            // bounded number of times instead of allocating without limit.
            continue;
        }

        buffer.truncate(written as usize);
        return Ok(Some(std::ffi::OsString::from_wide(buffer.as_slice())));
    }

    Err(EnvironmentValueError::Platform)
}

#[cfg(any(windows, test))]
fn classify_windows_zero_result(
    last_error: u32,
) -> Result<Option<std::ffi::OsString>, EnvironmentValueError> {
    const ERROR_ENVVAR_NOT_FOUND: u32 = 203;
    match last_error {
        ERROR_ENVVAR_NOT_FOUND => Ok(None),
        0 => Ok(Some(std::ffi::OsString::new())),
        _ => Err(EnvironmentValueError::Platform),
    }
}

#[cfg(any(windows, test))]
fn checked_windows_environment_buffer_len(required: u32) -> Result<usize, EnvironmentValueError> {
    if required as usize > MAX_ENVIRONMENT_VALUE_BYTES.saturating_add(1) {
        Err(EnvironmentValueError::ValueTooLarge)
    } else {
        Ok(required as usize)
    }
}

/// Lists only names authorized by an explicit listing grant. Values are
/// discarded by the shared validation layer before the result is returned.
pub fn list_names(grant: &EffectiveEnvironmentGrant) -> Result<Vec<String>, EnvironmentValueError> {
    zirk_runtime_core::environment::names_authorized_by(
        grant,
        environment_names_equal,
        std::env::vars_os,
    )
    .map(|mut names| {
        names.retain(|name| !name.to_ascii_uppercase().starts_with("ZIRK_RUNTIME_"));
        names
    })
}

fn with_effective_grant_for_names<T>(
    operation: impl FnOnce(&EffectiveEnvironmentGrant) -> Result<T, EnvironmentValueError>,
) -> Result<T, EnvironmentValueError> {
    ensure_process_grant();
    let guard = grant_lock()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match guard.as_ref() {
        Some(grant) => operation(grant),
        None => Err(EnvironmentValueError::PermissionDenied),
    }
}

fn ensure_process_grant() {
    let scopes = crate::permissions::current_scopes();
    *grant_lock()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(EffectiveEnvironmentGrant::from_verified_scopes(
            scopes.environment_read,
            scopes.environment_secrets,
            scopes.environment_list_names,
        ));
}

/// Stable result shape shared with generated code for environment reads.
#[repr(C)]
pub struct EnvironmentReadOut {
    /// Result tag: `0 = Ok`, `1 = Error`.
    pub kind: i32,
    /// Error discriminant when `kind == 1`; contains boolean when `kind == 0`.
    pub error: i32,
    /// Owned Zirk `String` handle when `kind == 0`, otherwise null.
    pub payload: *mut c_void,
}

fn error_discriminant(error: EnvironmentValueError) -> i32 {
    match error {
        EnvironmentValueError::InvalidName => 0,
        EnvironmentValueError::Missing => 1,
        EnvironmentValueError::PermissionDenied => 2,
        EnvironmentValueError::InvalidEncoding => 3,
        EnvironmentValueError::ValueTooLarge => 4,
        EnvironmentValueError::Platform => 6,
    }
}

/// Compiler-owned `Environment.get(String)` host leaf. With no authenticated
/// runtime grant installed, this returns permission denial before consulting
/// the process environment.
///
/// # Safety
/// `name` must be a live Zirk string handle and `out` must point to writable
/// `EnvironmentReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_environment_get(name: *const c_void, out: *mut EnvironmentReadOut) {
    unsafe { environment_get(name, out, false) }
}

/// Secret environment leaf with the same ownership and result ABI as
/// `zirk_environment_get`, but requiring exact-name secret authority.
///
/// # Safety
/// `name` must be a live Zirk string handle and `out` must point to writable
/// `EnvironmentReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_environment_get_secret(
    name: *const c_void,
    out: *mut EnvironmentReadOut,
) {
    unsafe { environment_get(name, out, true) }
}

/// Compiler-owned presence check; success stores its Boolean in `error` and
/// leaves `payload` null so the host value never enters the Zirk heap.
///
/// # Safety
/// `name` must be a live Zirk string handle and `out` must point to writable
/// `EnvironmentReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_environment_contains(
    name: *const c_void,
    out: *mut EnvironmentReadOut,
) {
    if out.is_null() {
        return;
    }
    let Some(name) = (unsafe { crate::string::borrow(name) }) else {
        unsafe {
            *out = EnvironmentReadOut {
                kind: 1,
                error: error_discriminant(EnvironmentValueError::InvalidName),
                payload: std::ptr::null_mut(),
            }
        };
        return;
    };
    let text = unsafe { name.as_str() };
    if !crate::permissions::request_resource("environment.read", text) {
        unsafe {
            *out = EnvironmentReadOut {
                kind: 1,
                error: error_discriminant(EnvironmentValueError::PermissionDenied),
                payload: std::ptr::null_mut(),
            }
        };
        return;
    }
    let result = with_effective_grant(|grant| contains(text, false, grant));
    let outcome = match result {
        Ok(value) => EnvironmentReadOut {
            kind: 0,
            error: i32::from(value),
            payload: std::ptr::null_mut(),
        },
        Err(error) => EnvironmentReadOut {
            kind: 1,
            error: error_discriminant(error),
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = outcome };
}

unsafe fn environment_get(name: *const c_void, out: *mut EnvironmentReadOut, secret: bool) {
    if out.is_null() {
        return;
    }
    let Some(name) = (unsafe { crate::string::borrow(name) }) else {
        unsafe {
            *out = EnvironmentReadOut {
                kind: 1,
                error: 0,
                payload: std::ptr::null_mut(),
            }
        };
        return;
    };
    let text = unsafe { name.as_str() };
    let capability = if secret {
        "secrets.read"
    } else {
        "environment.read"
    };
    if !crate::permissions::request_resource(capability, text) {
        unsafe {
            *out = EnvironmentReadOut {
                kind: 1,
                error: error_discriminant(EnvironmentValueError::PermissionDenied),
                payload: std::ptr::null_mut(),
            }
        };
        return;
    }
    let result = with_effective_grant(|grant| get(text, secret, grant));
    let outcome = match result {
        Ok(value) => {
            // The value must enter the managed heap as a Zirk String, but
            // erase the separate native copy immediately after copying it for
            // both ordinary and explicitly secret reads.
            let value = Zeroizing::new(value);
            let payload = crate::string::alloc_owned(&value);
            EnvironmentReadOut {
                kind: 0,
                error: 0,
                payload,
            }
        }
        Err(error) => EnvironmentReadOut {
            kind: 1,
            error: error_discriminant(error),
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = outcome };
}

/// Compiler-owned `Environment.list_names()` host leaf. The returned list
/// contains only names authorized by the installed effective grant.
///
/// # Safety
/// `out` must point to writable `EnvironmentReadOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_environment_list_names(out: *mut EnvironmentReadOut) {
    if out.is_null() {
        return;
    }
    if !crate::permissions::request_resource("environment.list_names", "all environment names") {
        unsafe {
            *out = EnvironmentReadOut {
                kind: 1,
                error: error_discriminant(EnvironmentValueError::PermissionDenied),
                payload: std::ptr::null_mut(),
            }
        };
        return;
    }
    let result = with_effective_grant_for_names(list_names);
    let outcome = match result {
        Ok(names) => {
            let element_size = std::mem::size_of::<*mut c_void>();
            let element_align = std::mem::align_of::<*mut c_void>();
            let list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
            let mut rooted_list = list;
            let mut current = std::ptr::null_mut();
            let mut roots = [
                (&mut rooted_list as *mut *mut c_void).cast(),
                (&mut current as *mut *mut c_void).cast(),
            ];
            unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
            for name in names {
                current = crate::string::alloc_owned(&name);
                unsafe {
                    crate::list::zirk_rt_list_add(
                        rooted_list,
                        (&current as *const *mut c_void).cast(),
                        element_size,
                        element_align,
                        true,
                    )
                };
            }
            crate::zirk_rt_pop_frame();
            EnvironmentReadOut {
                kind: 0,
                error: 0,
                payload: rooted_list,
            }
        }
        Err(error) => EnvironmentReadOut {
            kind: 1,
            error: error_discriminant(error),
            payload: std::ptr::null_mut(),
        },
    };
    unsafe { *out = outcome };
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirk_runtime_core::permission_grant::PermissionScopes;

    static GRANT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn install_runtime_permission_scope(capability: &str, resource: &str) {
        let mut scopes = PermissionScopes::default();
        match capability {
            "environment.read" => {
                scopes.environment_read.insert(resource.to_owned());
            }
            "environment.list_names" => scopes.environment_list_names = true,
            _ => panic!("unsupported test capability {capability}"),
        }
        crate::permissions::replace_current_scopes_for_test(scopes);
    }

    #[test]
    fn windows_lookup_zero_results_preserve_missing_empty_and_platform_errors() {
        assert_eq!(classify_windows_zero_result(203), Ok(None));
        assert_eq!(
            classify_windows_zero_result(0),
            Ok(Some(std::ffi::OsString::new()))
        );
        assert_eq!(
            classify_windows_zero_result(5),
            Err(EnvironmentValueError::Platform)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_scope_comparison_uses_the_native_ordinal_table() {
        assert!(environment_names_equal("Σervice", "σERVICE"));
        assert!(!environment_names_equal("ß", "SS"));
    }

    #[test]
    fn windows_lookup_bounds_native_buffer_before_allocation() {
        assert_eq!(
            checked_windows_environment_buffer_len((MAX_ENVIRONMENT_VALUE_BYTES + 1) as u32),
            Ok(MAX_ENVIRONMENT_VALUE_BYTES + 1)
        );
        assert_eq!(
            checked_windows_environment_buffer_len((MAX_ENVIRONMENT_VALUE_BYTES + 2) as u32),
            Err(EnvironmentValueError::ValueTooLarge)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_lookup_distinguishes_missing_empty_and_nonempty_values() {
        use windows_sys::Win32::System::Environment::SetEnvironmentVariableW;

        struct Cleanup(Vec<u16>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe {
                    SetEnvironmentVariableW(self.0.as_ptr(), std::ptr::null());
                }
            }
        }

        let name = format!("ZIRK_TEST_ΣERVICE_TOKEN_{}", std::process::id());
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let case_variant = name.to_lowercase();
        let _cleanup = Cleanup(wide_name.clone());
        unsafe {
            SetEnvironmentVariableW(wide_name.as_ptr(), std::ptr::null());
        }
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            get(&name, false, &grant),
            Err(EnvironmentValueError::Missing)
        );
        assert_eq!(
            get(&case_variant, false, &grant),
            Err(EnvironmentValueError::Missing)
        );

        let empty = [0u16];
        assert_ne!(
            unsafe { SetEnvironmentVariableW(wide_name.as_ptr(), empty.as_ptr()) },
            0
        );
        assert_eq!(get(&name, false, &grant), Ok(String::new()));
        assert_eq!(get(&case_variant, false, &grant), Ok(String::new()));
        assert_eq!(contains(&name, false, &grant), Ok(true));
        assert_eq!(contains(&case_variant, false, &grant), Ok(true));

        let value: Vec<u16> = "zirk".encode_utf16().chain(std::iter::once(0)).collect();
        assert_ne!(
            unsafe { SetEnvironmentVariableW(wide_name.as_ptr(), value.as_ptr()) },
            0
        );
        assert_eq!(get(&name, false, &grant), Ok(String::from("zirk")));
        assert_eq!(get(&case_variant, false, &grant), Ok(String::from("zirk")));
        assert_eq!(contains(&name, false, &grant), Ok(true));

        unsafe {
            SetEnvironmentVariableW(wide_name.as_ptr(), std::ptr::null());
        }
        assert_eq!(
            get(&name, false, &grant),
            Err(EnvironmentValueError::Missing)
        );
        assert_eq!(contains(&name, false, &grant), Ok(false));
    }

    #[test]
    fn native_leaf_fails_closed_before_host_lookup_without_a_grant() {
        let _guard = GRANT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::permissions::replace_current_scopes_for_test(PermissionScopes::default());
        revoke_effective_grant();
        let name = crate::string::alloc_owned("ZIRK_TEST_ENVIRONMENT_UNGRANTED");
        let mut out = EnvironmentReadOut {
            kind: -1,
            error: -1,
            payload: std::ptr::null_mut(),
        };
        unsafe { zirk_environment_get(name, &mut out) };
        assert_eq!(out.kind, 1);
        assert_eq!(out.error, 2);
        assert!(out.payload.is_null());
        unsafe { zirk_environment_contains(name, &mut out) };
        assert_eq!(out.kind, 1);
        assert_eq!(out.error, 2);
        assert!(out.payload.is_null());
    }

    #[test]
    fn explicit_exact_grant_can_read_a_host_value() {
        let (name, expected) = std::env::vars_os()
            .find_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .expect("the test runner supplies a UTF-8 environment entry");
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(get(&name, false, &grant), Ok(expected));
    }

    #[test]
    fn explicit_exact_grant_checks_presence_without_returning_the_value() {
        let name = std::env::vars_os()
            .find_map(|(name, _)| name.into_string().ok())
            .expect("the test runner supplies a UTF-8 environment entry");
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(contains(&name, false, &grant), Ok(true));

        let absent = format!("ZIRK_TEST_ABSENT_{}", std::process::id());
        let absent_grant = EffectiveEnvironmentGrant::from_verified_scopes(
            [absent.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(contains(&absent, false, &absent_grant), Ok(false));
    }

    #[test]
    fn contains_ffi_returns_only_the_boolean_for_authorized_names() {
        let _guard = GRANT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let present = std::env::vars_os()
            .find_map(|(name, _)| name.into_string().ok())
            .expect("the test runner supplies a UTF-8 environment entry");
        let absent = format!("ZIRK_TEST_ABSENT_{}", std::process::id());
        let mut runtime_scopes = PermissionScopes::default();
        runtime_scopes.environment_read.insert(present.clone());
        runtime_scopes.environment_read.insert(absent.clone());
        crate::permissions::replace_current_scopes_for_test(runtime_scopes);
        install_effective_grant(EffectiveEnvironmentGrant::from_verified_scopes(
            [present.clone(), absent.clone()],
            std::iter::empty(),
            false,
        ));
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            for (name, expected) in [(present, 1), (absent, 0)] {
                let name_handle = crate::string::alloc_owned(&name);
                let mut out = EnvironmentReadOut {
                    kind: -1,
                    error: -1,
                    payload: std::ptr::null_mut(),
                };
                unsafe { zirk_environment_contains(name_handle, &mut out) };
                assert_eq!(out.kind, 0);
                assert_eq!(out.error, expected);
                assert!(out.payload.is_null());
            }
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        revoke_effective_grant();
        crate::permissions::replace_current_scopes_for_test(PermissionScopes::default());
    }

    #[test]
    fn secret_reads_require_and_accept_secret_specific_authority() {
        let (name, expected) = std::env::vars_os()
            .find_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .expect("the test runner supplies a UTF-8 environment entry");
        let ordinary = EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(
            get(&name, true, &ordinary),
            Err(EnvironmentValueError::PermissionDenied)
        );
        let secret =
            EffectiveEnvironmentGrant::from_verified_scopes([name.clone()], [name.clone()], false);
        assert_eq!(get(&name, true, &secret), Ok(expected));
    }

    #[test]
    fn list_names_uses_narrow_grant_and_does_not_return_values() {
        let (name, _) = std::env::vars_os()
            .find_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .expect("the test runner supplies a UTF-8 environment entry");
        let grant = EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        );
        assert_eq!(list_names(&grant), Ok(vec![name]));
    }

    #[test]
    fn list_names_ffi_builds_a_gc_tracked_zirk_list() {
        let _guard = GRANT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (name, _) = std::env::vars_os()
            .find_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .expect("the test runner supplies a UTF-8 environment entry");
        install_runtime_permission_scope("environment.list_names", "all environment names");
        install_effective_grant(EffectiveEnvironmentGrant::from_verified_scopes(
            [name.clone()],
            std::iter::empty(),
            false,
        ));
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            let mut out = EnvironmentReadOut {
                kind: -1,
                error: -1,
                payload: std::ptr::null_mut(),
            };
            unsafe { zirk_environment_list_names(&mut out) };
            assert_eq!(out.kind, 0);
            let length = unsafe { crate::list::zirk_rt_list_length(out.payload) };
            assert!(length > 0);
            let mut found_expected_name = false;
            for index in 0..length {
                let slot = unsafe {
                    crate::list::zirk_rt_list_element(
                        out.payload,
                        i64::try_from(index).expect("environment list index fits in i64"),
                        8,
                    )
                };
                let name_handle = unsafe { *(slot as *const *mut c_void) };
                let actual = unsafe { crate::string::borrow(name_handle) }.unwrap();
                let actual = unsafe { actual.as_str() };
                assert!(!actual.contains('='));
                assert!(!actual.starts_with("ZIRK_RUNTIME_"));
                found_expected_name |= actual == name;
            }
            assert!(found_expected_name);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        revoke_effective_grant();
        crate::permissions::replace_current_scopes_for_test(PermissionScopes::default());
    }
}
