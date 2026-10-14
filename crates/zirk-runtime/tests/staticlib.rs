//! Verifies that `zirk-runtime` produces a linkable static library and that
//! its symbols cross the C ABI boundary without mangling.
//!
//! Symbols are searched for as text inside the file: the symbol table of a `.a`
//! (Unix) and of a `.lib` (Windows) contains the literal name. This avoids
//! depending on `nm`, `objdump` or `dumpbin`, which differ across platforms.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use zirk_runtime::{
    zirk_rt_cancel, zirk_rt_init, zirk_rt_job_wait, zirk_rt_job_wait_any,
    zirk_rt_job_wait_any_with_timer, zirk_rt_request_host_shutdown, zirk_rt_run_main,
    zirk_rt_shutdown, zirk_rt_spawn,
};

// Pin the C ABI prototypes used by generated code. The signature checker
// compares these declarations with the target-lowered runtime definitions.
unsafe extern "C" {
    fn zirk_rt_gc_mutex_alloc(
        payload: *const std::ffi::c_void,
        payload_size: usize,
        payload_align: usize,
        reference_offsets: *const usize,
        reference_count: usize,
        status_out: *mut u32,
    ) -> *mut std::ffi::c_void;
    fn zirk_rt_gc_mutex_acquire(
        handle: *mut std::ffi::c_void,
        payload_out: *mut *mut std::ffi::c_void,
        status_out: *mut u32,
    ) -> u32;
    fn zirk_rt_gc_mutex_release(handle: *mut std::ffi::c_void, status_out: *mut u32) -> u32;
    fn zirk_rt_gc_mutex_with(
        handle: *mut std::ffi::c_void,
        callback: Option<unsafe extern "C-unwind" fn(*mut std::ffi::c_void, *mut std::ffi::c_void)>,
        context: *mut std::ffi::c_void,
    ) -> u32;
}

/// Locates the artifact directory from the test executable, which Cargo places
/// in `<target>/<profile>/deps/`.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("could not locate the test executable");
    exe.parent()
        .and_then(|deps| deps.parent())
        .expect("unexpected Cargo directory layout")
        .to_path_buf()
}

fn staticlib_path() -> PathBuf {
    let dir = profile_dir();
    let (stable_name, hashed_prefix, extension) = if cfg!(windows) {
        ("zirk_runtime.lib", "zirk_runtime-", "lib")
    } else {
        ("libzirk_runtime.a", "libzirk_runtime-", "a")
    };

    let stable = dir.join(stable_name);
    if stable.is_file() {
        return stable;
    }

    // `cargo test` may build the host staticlib as a hashed dependency under
    // `deps/` without placing the un-hashed artifact in the profile root.
    // Cross-target builds live in sibling target directories and are never
    // considered because this path is derived from the running test binary.
    let deps = dir.join("deps");
    let mut candidates = std::fs::read_dir(&deps)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|value| value.to_str()) == Some(extension)
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.starts_with(hashed_prefix))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    candidates.into_iter().next().unwrap_or_else(|| {
        panic!(
            "host static library not found in {} or {}; \
             is crate-type `staticlib` still declared in Cargo.toml?",
            dir.display(),
            deps.display()
        )
    })
}

#[test]
fn a_static_library_is_produced() {
    let lib = staticlib_path();
    let metadata = std::fs::metadata(&lib).expect("could not read the library");

    assert!(
        metadata.len() > 0,
        "the static library is empty: {}",
        lib.display()
    );
}

#[test]
fn the_lifecycle_symbols_carry_no_mangling() {
    let _request_host_shutdown_signature: extern "C" fn(i32) -> u32 = zirk_rt_request_host_shutdown;
    let _gc_mutex_alloc_signature: unsafe extern "C" fn(
        *const std::ffi::c_void,
        usize,
        usize,
        *const usize,
        usize,
        *mut u32,
    ) -> *mut std::ffi::c_void = zirk_rt_gc_mutex_alloc;
    let _gc_mutex_acquire_signature: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *mut *mut std::ffi::c_void,
        *mut u32,
    ) -> u32 = zirk_rt_gc_mutex_acquire;
    let _gc_mutex_release_signature: unsafe extern "C" fn(*mut std::ffi::c_void, *mut u32) -> u32 =
        zirk_rt_gc_mutex_release;
    let _gc_mutex_with_signature: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        Option<unsafe extern "C-unwind" fn(*mut std::ffi::c_void, *mut std::ffi::c_void)>,
        *mut std::ffi::c_void,
    ) -> u32 = zirk_rt_gc_mutex_with;
    let lib = staticlib_path();
    let bytes = std::fs::read(&lib).expect("could not read the library");

    for symbol in [
        "zirk_rt_init",
        "zirk_rt_shutdown",
        "zirk_str_from_utf8",
        "zirk_str_from_i32",
        "zirk_str_from_bool",
        "zirk_str_eq",
        "zirk_io_println",
        "zirk_rt_overflow",
        "zirk_rt_division_by_zero",
        "zirk_rt_alloc",
        "zirk_rt_allocation_failed",
        "zirk_rt_background_spawn",
        "zirk_rt_run_main",
        "zirk_rt_run_main_cli",
        "zirk_rt_run_main_with_deadline_policy",
        "zirk_rt_request_host_shutdown",
        "zirk_rt_service_register",
        "zirk_rt_services_start",
        "zirk_rt_service_ready",
        "zirk_rt_service_failed_current",
        "zirk_rt_services_shutdown",
        "zirk_rt_job_wait_any",
        "zirk_rt_job_wait_any_with_timer",
        "zirk_rt_job_outcome_take",
        "zirk_rt_job_outcome_wait_take",
        "zirk_rt_branch_fail",
        "zirk_rt_scope_enter_collect_all",
        "zirk_rt_scope_exit_collect_all",
        "zirk_rt_scope_outcome_wait_take",
        "zirk_rt_scope_cancel",
        "zirk_rt_atomic_new",
        "zirk_rt_atomic_free",
        "zirk_rt_atomic_load",
        "zirk_rt_atomic_store",
        "zirk_rt_atomic_exchange",
        "zirk_rt_atomic_compare_exchange",
        "zirk_rt_atomic_fetch_add",
        "zirk_rt_atomic_fetch_sub",
        "zirk_rt_gc_mutex_alloc",
        "zirk_rt_gc_mutex_acquire",
        "zirk_rt_gc_mutex_release",
        "zirk_rt_gc_mutex_with",
    ] {
        assert!(
            contains(&bytes, symbol.as_bytes()),
            "symbol `{symbol}` does not appear unmangled in {}",
            lib.display()
        );
    }
}

static SERVICE_ABI_ENTERED: AtomicBool = AtomicBool::new(false);
static SERVICE_ABI_READY_STATUS: AtomicU64 = AtomicU64::new(u64::MAX);
static SERVICE_ABI_REGISTER_STATUS: AtomicU64 = AtomicU64::new(u64::MAX);
static SERVICE_ABI_SHUTDOWN_STATUS: AtomicU64 = AtomicU64::new(u64::MAX);

extern "C-unwind" fn abi_service_callback(id: u64, _user_data: *mut std::ffi::c_void) -> u64 {
    let status = unsafe { zirk_runtime::zirk_rt_service_ready(id) };
    SERVICE_ABI_READY_STATUS.store(u64::from(status), Ordering::SeqCst);
    SERVICE_ABI_ENTERED.store(true, Ordering::SeqCst);
    loop {
        unsafe { zirk_runtime::zirk_rt_sleep(0) };
        if unsafe { zirk_runtime::zirk_rt_is_cancelled() } {
            return 0;
        }
    }
}

extern "C-unwind" fn service_abi_driver() {
    let mut status = u32::MAX;
    let id = unsafe {
        zirk_runtime::zirk_rt_service_register(
            abi_service_callback,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            1,
            &mut status,
        )
    };
    SERVICE_ABI_REGISTER_STATUS.store(u64::from(status), Ordering::SeqCst);
    assert_ne!(id, u64::MAX);
    while !SERVICE_ABI_ENTERED.load(Ordering::SeqCst) {
        unsafe { zirk_runtime::zirk_rt_sleep(0) };
    }
    let status = unsafe { zirk_runtime::zirk_rt_services_shutdown() };
    SERVICE_ABI_SHUTDOWN_STATUS.store(u64::from(status), Ordering::SeqCst);
}

#[test]
fn service_abi_registers_publishes_readiness_and_joins_shutdown() {
    SERVICE_ABI_ENTERED.store(false, Ordering::SeqCst);
    SERVICE_ABI_READY_STATUS.store(u64::MAX, Ordering::SeqCst);
    SERVICE_ABI_REGISTER_STATUS.store(u64::MAX, Ordering::SeqCst);
    SERVICE_ABI_SHUTDOWN_STATUS.store(u64::MAX, Ordering::SeqCst);
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(service_abi_driver);
        zirk_rt_shutdown();
    }
    assert_eq!(
        SERVICE_ABI_REGISTER_STATUS.load(Ordering::SeqCst),
        u64::from(zirk_runtime::SERVICE_ABI_OK)
    );
    assert_eq!(
        SERVICE_ABI_READY_STATUS.load(Ordering::SeqCst),
        u64::from(zirk_runtime::SERVICE_ABI_OK)
    );
    assert_eq!(
        SERVICE_ABI_SHUTDOWN_STATUS.load(Ordering::SeqCst),
        u64::from(zirk_runtime::SERVICE_ABI_OK)
    );
}

static RELEASE_FIRST: AtomicBool = AtomicBool::new(false);
static ORDER_RESULT: AtomicU64 = AtomicU64::new(u64::MAX);

extern "C-unwind" fn gated_first(_: *mut std::ffi::c_void) -> usize {
    while !RELEASE_FIRST.load(Ordering::SeqCst) {
        // A zero-duration timer is an explicit executor handoff, not a
        // wall-clock delay; this lets the gated sibling make progress.
        unsafe { zirk_runtime::zirk_rt_sleep(0) };
    }
    0
}

extern "C-unwind" fn release_then_finish(_: *mut std::ffi::c_void) -> usize {
    RELEASE_FIRST.store(true, Ordering::SeqCst);
    0
}

extern "C-unwind" fn completion_order_driver() {
    RELEASE_FIRST.store(false, Ordering::SeqCst);
    let ids = unsafe {
        [
            zirk_rt_spawn(gated_first, std::ptr::null_mut()),
            zirk_rt_spawn(release_then_finish, std::ptr::null_mut()),
        ]
    };
    let index = unsafe { zirk_rt_job_wait_any(ids.as_ptr(), ids.len() as u64) };
    ORDER_RESULT.store(index, Ordering::SeqCst);
}

#[test]
fn public_wait_any_selects_earliest_serialized_completion() {
    ORDER_RESULT.store(u64::MAX, Ordering::SeqCst);
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(completion_order_driver);
        zirk_rt_shutdown();
    }
    assert_eq!(ORDER_RESULT.load(Ordering::SeqCst), 1);
}

static CANCEL_RESULT: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn cancel_root(_: *mut std::ffi::c_void) -> usize {
    // The executor assigns the run_main root task the first packed task ID.
    unsafe { zirk_rt_cancel(0) };
    0
}

extern "C-unwind" fn complete_immediately(_: *mut std::ffi::c_void) -> usize {
    0
}

extern "C-unwind" fn cancellation_driver() {
    let ids = unsafe {
        [
            zirk_rt_spawn(cancel_root, std::ptr::null_mut()),
            zirk_rt_spawn(complete_immediately, std::ptr::null_mut()),
        ]
    };
    let result = unsafe { zirk_rt_job_wait_any(ids.as_ptr(), ids.len() as u64) };
    CANCEL_RESULT.store(result, Ordering::SeqCst);
}

#[test]
fn public_wait_any_returns_cancellation_sentinel() {
    CANCEL_RESULT.store(0, Ordering::SeqCst);
    unsafe {
        zirk_rt_init();
        // A pending cancellation is propagated when the root task returns;
        // contain that expected runtime unwind so the ABI sentinel can be
        // asserted by this host-side test.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            zirk_rt_run_main(cancellation_driver)
        }));
        zirk_rt_shutdown();
    }
    let result = CANCEL_RESULT.load(Ordering::SeqCst);
    assert_eq!(result, u64::MAX, "got result {result}");
}

#[test]
fn public_wait_any_rejects_out_of_range_counts_before_reading_ids() {
    let ids = [0_u64; 2];
    for count in [0, 1, 9, u64::MAX] {
        // The ABI explicitly rejects these counts before it inspects the
        // pointer, so even an otherwise insufficient backing slice is safe.
        let result = unsafe { zirk_rt_job_wait_any(ids.as_ptr(), count) };
        assert_eq!(result, u64::MAX, "unexpected result for count {count}");
    }
}

static TIMER_WAIT_RESULT: AtomicU64 = AtomicU64::new(u64::MAX);

extern "C-unwind" fn timer_wait_any_driver() {
    let ids: [u64; 9] = unsafe {
        std::array::from_fn(|_| zirk_rt_spawn(complete_immediately, std::ptr::null_mut()))
    };
    let result = unsafe { zirk_rt_job_wait_any_with_timer(ids.as_ptr(), ids.len() as u64) };
    TIMER_WAIT_RESULT.store(result, Ordering::SeqCst);
}

#[test]
fn timer_wait_any_accepts_eight_source_jobs_plus_timer() {
    TIMER_WAIT_RESULT.store(u64::MAX, Ordering::SeqCst);
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(timer_wait_any_driver);
        zirk_rt_shutdown();
    }
    assert!(TIMER_WAIT_RESULT.load(Ordering::SeqCst) < 9);
}

#[test]
fn wait_any_abis_reject_excess_counts_before_reading_ids() {
    let ids = [0_u64; 2];
    assert_eq!(unsafe { zirk_rt_job_wait_any(ids.as_ptr(), 9) }, u64::MAX);
    for count in [0, 1, 10, u64::MAX] {
        assert_eq!(
            unsafe { zirk_rt_job_wait_any_with_timer(ids.as_ptr(), count) },
            u64::MAX,
            "unexpected timer wait-any result for count {count}"
        );
    }
}

extern "C-unwind" fn invalid_handle_driver() {
    let invalid_case = std::env::var("ZIRK_TEST_INVALID_WAIT_ANY").unwrap();
    let live = unsafe { zirk_rt_spawn(complete_immediately, std::ptr::null_mut()) };
    let ids = match invalid_case.as_str() {
        "stale" => [u64::MAX, live],
        "cancelled_stale" => {
            // Invalid handles remain fatal even when the caller is cancelled;
            // the cancellation sentinel is reserved for a valid wait request.
            unsafe { zirk_rt_cancel(0) };
            [u64::MAX, live]
        }
        "duplicate" => [live, live],
        // run_main assigns the root task packed ID zero.
        "self" => [0, live],
        "consumed" => {
            let consumed = unsafe { zirk_rt_spawn(complete_immediately, std::ptr::null_mut()) };
            let _ = unsafe { zirk_rt_job_wait(consumed) };
            [consumed, live]
        }
        other => panic!("unknown invalid wait-any test case: {other}"),
    };
    let _ = unsafe { zirk_rt_job_wait_any(ids.as_ptr(), ids.len() as u64) };
    panic!("invalid wait-any IDs unexpectedly returned");
}

#[test]
fn public_wait_any_rejects_invalid_handles_before_suspending() {
    if let Ok(invalid_case) = std::env::var("ZIRK_TEST_INVALID_WAIT_ANY") {
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(invalid_handle_driver);
        }
        panic!("fatal wait-any validation unexpectedly returned for {invalid_case}");
    }

    for (invalid_case, diagnostic) in [
        ("stale", "wait-any received a task ID that is not live"),
        (
            "cancelled_stale",
            "wait-any received a task ID that is not live",
        ),
        ("duplicate", "wait-any received duplicate task IDs"),
        ("self", "wait-any cannot wait on the current task"),
        (
            "consumed",
            "wait-any received a task whose result is already awaited",
        ),
    ] {
        let mut child = Command::new(std::env::current_exe().expect("test executable path"))
            .args([
                "--exact",
                "public_wait_any_rejects_invalid_handles_before_suspending",
                "--nocapture",
            ])
            .env("ZIRK_TEST_INVALID_WAIT_ANY", invalid_case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn invalid-handle ABI test child");
        let deadline = Instant::now() + Duration::from_secs(3);
        while child.try_wait().expect("poll child status").is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().expect("poll child status").is_none() {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("collect timed-out child output");
            panic!(
                "{invalid_case} wait-any handle suspended instead of failing closed: {output:?}"
            );
        }
        let output = child.wait_with_output().expect("collect child output");
        assert_eq!(
            output.status.code(),
            Some(70),
            "{invalid_case} did not use the runtime fatal ABI path; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "missing {invalid_case} diagnostic: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
