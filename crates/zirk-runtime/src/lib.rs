//! # zirk-runtime
//!
//! **Responsibility:** the runtime linked into every binary Zirk produces. It
//! provides the application lifecycle and, later on, memory, scheduler,
//! channels and resources.
//!
//! **Boundary:** this crate is **not** part of the compiler and no compiler
//! crate depends on it. It is compiled to a `staticlib` for the target of the
//! compiled program, not for the host `zirkc` runs on.
//!
//! # C ABI boundary
//!
//! Every symbol intended for generated code is declared `extern "C"` without
//! mangling. It is the same boundary `ZIRK_LANGUAGE_SPEC.md` section 13
//! requires for native interoperability: not temporary scaffolding, but the
//! final boundary put to use early (see
//! `docs/decisions/ADR-002-runtime-staticlib.md`).
//!
//! These symbols are a compatibility surface: changing them breaks already
//! compiled binaries.
//!
//! # State
//!
//! The lifecycle of `ZIRK_RUNTIME_SPEC.md` section 2 is:
//!
//! ```text
//! validate .zkinit and permissions -> load minimal runtime -> init globals
//!   -> main() -> concurrency scopes -> close resources -> flush -> exit
//! ```
//!
//! In Phase 0 only the ends of that sequence exist, with empty bodies. They
//! are defined now on purpose: they fix the shape onto which Phase 4 (memory)
//! and Phase 5 (concurrency) hook without refactoring codegen.
//!
//! # Scheduler terminology
//!
//! A *task* in this crate is an internal stackful coroutine scheduled by the
//! executor. It is not the removed language-level `Task<T>` value; the pending
//! concurrency-surface changes reuse these scheduler primitives.

mod array;
mod atomic;
pub mod background;
mod channel;
mod char;
mod clone;
mod collector;
// Internal allocation/tracing hooks for the GC-managed Mutex foundation.
#[allow(unused_imports)]
pub(crate) use collector::gc_mutex;
#[allow(dead_code)] // Internal foundation until the language ABI is lowered.
mod combinators;
mod context;
mod decimal;
mod duration;
pub mod environment;
mod exceptions;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
#[cfg(feature = "http3")]
mod http3_transport;
pub mod http_api;
pub mod http_runtime;
pub mod local_transport;
// The cooperative executor (`ADR-017`): `zirk_rt_run_main` drives it, and
// `fase-5-structured-tasks` will add the `task` / `await` C-ABI surface.
#[allow(dead_code)]
// spawn/await/sleep/is_done are used only by tests until the language surface lands
mod executor;
mod failure;
// Bounded event queues for project-global subscriptions.
mod global_watch;
mod host_signal;
mod io;
mod journal;
mod list;
mod map;
mod memory;
// Bounded native-work pool foundation; it accepts Rust-owned jobs only and is
// not yet the launcher for managed Zirk `Thread.run` closures.
#[allow(dead_code)]
mod managed_blocking;
mod native_slice;
pub mod network_api;
#[allow(dead_code)] // Runtime spike; public transport bindings are delivered in later tasks.
mod network_future;
pub(crate) mod network_interfaces;
pub mod network_transport;
mod object;
pub mod permissions;
// The `parallel` CPU-region worker pool (`ADR-019`, `parallel-cpu-regions`).
// Staged at pool size 1 until the stop-the-world safepoint collector lands;
// the C-ABI surface (`zirk_rt_pool_submit` / `_join` / `zirk_rt_safepoint_poll`)
// is exercised by generated code from the start.
#[allow(dead_code)]
mod pool;
mod range;
mod regex;
mod resource;
mod scalar;
pub mod services;
mod set;
mod string;
#[allow(dead_code)] // Runtime-only foundation; no language or ABI surface yet.
mod sync_mutex;
#[allow(dead_code)] // Runtime-only foundation; no language or ABI surface yet.
mod sync_primitives;
#[allow(dead_code)] // Internal foundation until the language ABI is lowered.
mod sync_rwlock;
// `task` and `timer` back `crate::executor`; a few of their items (cancel
// flags, `TimedOut`, `WaitReason::Timer` inspection) are only read once the
// language surface lands in `fase-5-structured-tasks`.
#[allow(dead_code)]
mod task;
mod temporal;
#[allow(dead_code)]
mod timer;

pub use array::*;
pub use atomic::{
    AtomicCell, AtomicError, AtomicKind, AtomicOrder, supports as atomic_supports,
    valid_compare_exchange_orderings, zirk_rt_atomic_compare_exchange, zirk_rt_atomic_exchange,
    zirk_rt_atomic_fetch_add, zirk_rt_atomic_fetch_sub, zirk_rt_atomic_free, zirk_rt_atomic_load,
    zirk_rt_atomic_managed_compare_exchange, zirk_rt_atomic_managed_exchange,
    zirk_rt_atomic_managed_exchange_ordered, zirk_rt_atomic_managed_fetch_add,
    zirk_rt_atomic_managed_fetch_add_ordered, zirk_rt_atomic_managed_fetch_sub,
    zirk_rt_atomic_managed_fetch_sub_ordered, zirk_rt_atomic_managed_load,
    zirk_rt_atomic_managed_load_ordered, zirk_rt_atomic_managed_new, zirk_rt_atomic_managed_store,
    zirk_rt_atomic_managed_store_ordered, zirk_rt_atomic_new, zirk_rt_atomic_store,
};
pub use background::{
    AdmissionError, BackgroundDiagnostic, BackgroundFailure, BackgroundJobId, BackgroundSupervisor,
    DrainReport,
};
pub use channel::*;
pub use char::*;
pub use collector::{
    UnionRoot, zirk_rt_global_root_add, zirk_rt_global_root_remove, zirk_rt_pop_frame,
    zirk_rt_push_frame, zirk_rt_push_union_roots,
};
pub use executor::{
    BACKGROUND_JOB_CAPACITY, BACKGROUND_SHUTDOWN_GRACE, background_diagnostics_current,
    spawn_background, spawn_background_with_capture_root,
};
// Stackful task contexts (ADR-017). Not yet a C-ABI surface — the executor
// (roadmap Phase 5, task group 3) is what will drive these; exported now so the
// module and its ping-pong tests are part of the build.
pub use context::{
    DEFAULT_TASK_STACK_BYTES, Run, Suspender, TaskContext, spawn_default, suspend_current,
};
pub use failure::{
    zirk_rt_allocation_failed, zirk_rt_division_by_zero, zirk_rt_fatal_error,
    zirk_rt_index_out_of_bounds, zirk_rt_invalid_union_tag, zirk_rt_overflow,
};
pub use io::{zirk_io_print, zirk_io_println};
pub use journal::{
    zirk_rt_journal_begin, zirk_rt_journal_commit, zirk_rt_journal_record, zirk_rt_journal_rollback,
};
pub use list::*;
pub use managed_blocking::{
    MANAGED_CALL_OK, MANAGED_CALL_PANICKED, MANAGED_CALL_PENDING, MANAGED_CALL_THROWN,
    ManagedCallThunk, zirk_rt_thread_run,
};
pub use memory::{zirk_rt_alloc, zirk_rt_dependent_base, zirk_rt_pin_object, zirk_rt_unpin_object};
pub use object::*;
pub use pool::{zirk_rt_pool_join, zirk_rt_pool_submit, zirk_rt_safepoint_poll};
pub use range::{
    zirk_range_element_at, zirk_range_end, zirk_range_inclusive, zirk_range_length, zirk_range_new,
    zirk_range_slice, zirk_range_start, zirk_range_step,
};
pub use regex::{
    zirk_regex_find, zirk_regex_find_all, zirk_regex_from_pattern, zirk_regex_is_match,
    zirk_regex_match_group_name, zirk_regex_match_group_pos, zirk_regex_replace, zirk_regex_split,
    zirk_regex_to_string,
};
pub use resource::{zirk_rt_is_cancelled, zirk_rt_resource_close_group, zirk_rt_resource_transfer};
pub use services::{
    DEFAULT_SERVICE_CAPACITY, ServiceContext, ServiceError, ServiceFailure, ServiceId,
    ServiceOperation, ServiceState, ServiceSupervisor,
};
pub use string::{
    zirk_str_concat, zirk_str_eq, zirk_str_from_bool, zirk_str_from_f32, zirk_str_from_f64,
    zirk_str_from_i8, zirk_str_from_i16, zirk_str_from_i32, zirk_str_from_i64, zirk_str_from_i128,
    zirk_str_from_u8, zirk_str_from_u16, zirk_str_from_u32, zirk_str_from_u64, zirk_str_from_u128,
    zirk_str_from_utf8, zirk_str_grapheme_len_at, zirk_str_grapheme_slice, zirk_str_hash,
    zirk_str_is_ascii, zirk_str_repeat, zirk_str_set, zirk_str_slice,
};

/// Initializes the runtime before running `main`.
///
/// Corresponds to the "load minimal runtime" and "init globals" steps of
/// `ZIRK_RUNTIME_SPEC.md` section 2.
///
/// # Safety
///
/// Invoked by Zirk-generated code exactly once, before any other runtime
/// function. Calling it more than once, or after [`zirk_rt_shutdown`], is not
/// supported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_init() {
    // Reset the portable host notification slot before starting the run.
    host_signal::reset();
    executor::reset_host_shutdown_signal();
    // Phase 0: no other subsystems to initialize.
    //
    // `ZIRK_RUNTIME_SPEC.md` section 1 asks for lazy subsystem initialization,
    // so this point will likely never do heavy work: it marks the start of the
    // lifecycle, it does not build the whole runtime.
}

/// Runs the Zirk entrypoint as the executor's root task.
///
/// Corresponds to the "`main()` → concurrency scopes" step of
/// `ZIRK_RUNTIME_SPEC.md` section 2. The generated C `main` calls this instead
/// of calling the Zirk `main` directly: `zirk_main` becomes the body of task 0
/// on the single-threaded cooperative executor (`ADR-017`), and this call
/// returns only once task 0 and every task it spawned has completed or been
/// cleaned. For a program with no `task` / `await` yet, that is exactly one
/// run of `zirk_main` to completion — the executor is in place for when `main`
/// can spawn.
///
/// A pending Zirk exception is not a Rust panic, so it flows out normally and
/// the generated C `main` checks [`zirk_rt_has_pending_exception`](exceptions)
/// afterwards, as before. A genuine Rust panic inside the runtime is
/// re-raised.
///
/// # Safety
///
/// `zirk_main` must be the compiler-emitted Zirk entrypoint: an `extern "C"`
/// function taking no arguments. Invoked by generated code exactly once,
/// after [`zirk_rt_init`] and before [`zirk_rt_shutdown`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_run_main(zirk_main: extern "C-unwind" fn()) {
    let _ = run_main_with_deadline_policy(zirk_main, false);
}

/// Runs the Zirk entrypoint with an explicit host policy for an expired
/// background drain. Policy `0` waits safely for native work; policy `1`
/// exits with status 124. Unknown values are rejected before starting Zirk.
///
/// # Safety
/// Has the same requirements as [`zirk_rt_run_main`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_run_main_with_deadline_policy(
    zirk_main: extern "C-unwind" fn(),
    policy: u32,
) {
    let terminate = match policy {
        SHUTDOWN_DEADLINE_WAIT => false,
        SHUTDOWN_DEADLINE_EXIT_124 => true,
        _ => panic!("invalid shutdown deadline policy: {policy}"),
    };
    let shutdown_signal = run_main_with_deadline_policy(zirk_main, terminate);
    if terminate && let Some(signal) = shutdown_signal {
        std::process::exit(128 + signal.min(127));
    }
}

/// Runs a generated CLI entrypoint, terminating with status 124 if background
/// work remains after the configured shutdown grace. Embedded hosts should use
/// [`zirk_rt_run_main`] or provide their own host-level timeout policy.
///
/// # Safety
/// Has the same requirements as [`zirk_rt_run_main`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_run_main_cli(zirk_main: extern "C-unwind" fn()) {
    let _handlers = host_signal::install_cli_signal_handlers();
    let watchdog = host_signal::CliShutdownWatchdog::start();
    let signal = run_main_with_deadline_policy(zirk_main, true);
    watchdog.disarm();
    drop(_handlers);
    if let Some(signal) = signal {
        std::process::exit(128 + signal.min(127));
    }
}

pub const SERVICE_ABI_OK: u32 = 0;
pub const SERVICE_ABI_SERVICE_CAPACITY: u32 = 1;
pub const SERVICE_ABI_TASK_CAPACITY: u32 = 2;
pub const SERVICE_ABI_INVALID_STATE: u32 = 3;
pub const SERVICE_ABI_CAPACITY: u32 = SERVICE_ABI_SERVICE_CAPACITY;

/// Registers a required source service after all earlier required services.
/// The callback capture block remains GC-rooted until the service is stopped.
///
/// # Safety
/// `status_out` must be writable; callback and pointers must obey the managed
/// executor ABI and remain valid for the registered callback's lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_service_register(
    callback: extern "C-unwind" fn(u64, *mut std::ffi::c_void) -> u64,
    user_data: *mut std::ffi::c_void,
    capture_root: *mut std::ffi::c_void,
    required: u32,
    status_out: *mut u32,
) -> u64 {
    if status_out.is_null() {
        failure::fatal("zirk_rt_service_register requires a non-null status output");
    }
    if required != 1 {
        // SAFETY: checked non-null; caller guarantees writable storage.
        unsafe { *status_out = SERVICE_ABI_INVALID_STATE };
        return u64::MAX;
    }
    let arg = TaskArg(user_data);
    let result = executor::register_service_with_prior_required_current(
        "source service",
        true,
        capture_root,
        move |context| callback(context.id().0, arg.into_raw()) as usize,
    );
    match result {
        Ok(id) => {
            // SAFETY: checked non-null; caller guarantees writable storage.
            unsafe { *status_out = SERVICE_ABI_OK };
            id.0
        }
        Err(services::ServiceError::CapacityReached) => {
            // SAFETY: checked non-null; caller guarantees writable storage.
            unsafe { *status_out = SERVICE_ABI_SERVICE_CAPACITY };
            u64::MAX
        }
        Err(services::ServiceError::TaskCapacityReached) => {
            // SAFETY: checked non-null; caller guarantees writable storage.
            unsafe { *status_out = SERVICE_ABI_TASK_CAPACITY };
            u64::MAX
        }
        Err(_) => {
            // SAFETY: checked non-null; caller guarantees writable storage.
            unsafe { *status_out = SERVICE_ABI_INVALID_STATE };
            u64::MAX
        }
    }
}

#[unsafe(no_mangle)]
/// Starts every registered service whose dependencies are ready.
///
/// # Safety
/// Must be called on the active executor thread from the generated runtime
/// entrypoint; the service registry is executor-local.
pub unsafe extern "C-unwind" fn zirk_rt_services_start() -> u32 {
    match executor::start_services_current() {
        Ok(()) => SERVICE_ABI_OK,
        Err(services::ServiceError::TaskCapacityReached) => SERVICE_ABI_TASK_CAPACITY,
        Err(_) => SERVICE_ABI_INVALID_STATE,
    }
}

#[unsafe(no_mangle)]
/// Publishes readiness for a service on the active executor.
///
/// # Safety
/// `id` must be a service registered in the current executor, and the caller
/// must be that service's active callback.
pub unsafe extern "C-unwind" fn zirk_rt_service_ready(id: u64) -> u32 {
    match executor::service_ready_current(services::ServiceId(id)) {
        Ok(_) => SERVICE_ABI_OK,
        Err(_) => SERVICE_ABI_INVALID_STATE,
    }
}

#[unsafe(no_mangle)]
/// Reports an unexpected failure from the currently running service callback.
///
/// # Safety
/// Must be called on the active executor thread from a registered service
/// callback.
pub unsafe extern "C-unwind" fn zirk_rt_service_failed_current() -> u32 {
    match executor::service_failed_current() {
        Ok(()) => SERVICE_ABI_OK,
        Err(_) => SERVICE_ABI_INVALID_STATE,
    }
}

#[unsafe(no_mangle)]
/// Begins ordered shutdown of registered services on the active executor.
///
/// # Safety
/// Must be called on the active executor thread while its service registry is
/// alive.
pub unsafe extern "C-unwind" fn zirk_rt_services_shutdown() -> u32 {
    match executor::shutdown_services_current() {
        Ok(()) => SERVICE_ABI_OK,
        Err(_) => SERVICE_ABI_INVALID_STATE,
    }
}

// Keep the not-yet-codegenerated service ABI entry points in the native
// staticlib so downstream generated code can resolve them once the source
// lowering gate is removed.
#[allow(dead_code)]
struct ServiceAbiExports {
    register: unsafe extern "C" fn(
        extern "C-unwind" fn(u64, *mut std::ffi::c_void) -> u64,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        u32,
        *mut u32,
    ) -> u64,
    start: unsafe extern "C-unwind" fn() -> u32,
    ready: unsafe extern "C-unwind" fn(u64) -> u32,
    failed: unsafe extern "C-unwind" fn() -> u32,
    shutdown: unsafe extern "C-unwind" fn() -> u32,
}

#[used]
static SERVICE_ABI_EXPORTS: ServiceAbiExports = ServiceAbiExports {
    register: zirk_rt_service_register,
    start: zirk_rt_services_start,
    ready: zirk_rt_service_ready,
    failed: zirk_rt_service_failed_current,
    shutdown: zirk_rt_services_shutdown,
};

/// Values accepted by `zirk_rt_run_main_with_deadline_policy`.
pub const SHUTDOWN_DEADLINE_WAIT: u32 = 0;
pub const SHUTDOWN_DEADLINE_EXIT_124: u32 = 1;

fn run_main_with_deadline_policy(
    zirk_main: extern "C-unwind" fn(),
    terminate_on_deadline: bool,
) -> Option<i32> {
    // A cancelled branch and a branch's unhandled exception travel as a native
    // unwind through the branch's own stack (`concurrent-blocks-and-timers`,
    // ADR-017). Those are ordinary control flow, not runtime bugs — keep the
    // default hook's backtrace noise off stderr for them, but let a genuine
    // runtime panic through untouched.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let internal =
            payload.is::<executor::Cancelled>() || payload.is::<executor::BranchFailure>();
        if !internal {
            previous(info);
        }
    }));

    let outcome = executor::Executor::new().run_with_root_deadline_policy(
        move || {
            zirk_main();
            0
        },
        terminate_on_deadline,
    );
    let _ = std::panic::take_hook();

    let shutdown_signal = executor::take_host_shutdown_signal();
    match outcome {
        task::TaskOutcome::Panicked(payload)
            if shutdown_signal.is_some() && payload.is::<executor::Cancelled>() => {}
        task::TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        task::TaskOutcome::Value(_) => {}
    }
    shutdown_signal
}

/// Queues a host shutdown notification for the cooperative executor.
///
/// Hosts may call this from their own signal bridge. It performs no allocation,
/// locking, logging, cancellation, or process termination. The first valid
/// signal wins until the executor consumes it. Signal codes must be 1..=127;
/// zero is reserved to mean "no signal". This hook does not install OS
/// handlers or forward a signal from the `zirk run` parent to its child.
///
/// Returns zero when queued and one for an invalid or already-pending request.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_request_host_shutdown(signal: i32) -> u32 {
    u32::from(!host_signal::request(signal))
}

/// A raw pointer that crosses into a task body. The executor is single-threaded
/// (`ADR-017`) and, on the thread-backed fallback backend, hands control between
/// task threads by rendezvous so only one ever runs at a time — so moving a
/// `*mut c_void` (a GC-tracked capture-block pointer) into a task closure is
/// sound even though `*mut` is not `Send` on its own.
struct TaskArg(*mut std::ffi::c_void);
// SAFETY: see the type doc — cooperative single-runner scheduling.
unsafe impl Send for TaskArg {}

impl TaskArg {
    fn into_raw(self) -> *mut std::ffi::c_void {
        self.0
    }
}

/// Attempts to start `body(arg)` as a new task. Returns the packed task ID on
/// success and writes status 0; writes status 1 and returns zero when the
/// executor has no active-task capacity. The checked ABI never registers a
/// failed handle or retains the capture block on rejection. Generated Zirk
/// code converts status 1 into `TaskCapacityError` before using the handle.
///
/// **Provisional ABI.** `fase-5-structured-tasks` decides the real `Task<T>`
/// value representation; the packed-`u64` id and `usize` result may change then.
///
/// # Safety
///
/// Must be called from inside a running task (ultimately from a `zirk_main`
/// driven by [`zirk_rt_run_main`]). `body` must be a valid function pointer and
/// `arg` must be the compiler-emitted callable capture-block pointer (or null).
/// `status_out` must be non-null, aligned, writable, and live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_spawn_checked(
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
    status_out: *mut u32,
) -> u64 {
    if status_out.is_null() {
        failure::fatal("zirk_rt_spawn requires a non-null status output");
    }
    let arg = TaskArg(arg);
    let capture_root = arg.0;
    let child = move || {
        // `let arg = arg;` forces edition-2024 disjoint captures to move the
        // whole `Send` `TaskArg`, not its `!Send` `*mut` field.
        let arg = arg;
        body(arg.0)
    };
    match executor::try_spawn_with_capture_root(child, capture_root) {
        Ok(task) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 0 };
            task.to_bits()
        }
        Err(crate::task::TaskAdmissionError::CapacityExceeded) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 1 };
            0
        }
    }
}

/// Compatibility entry point for existing native consumers. Generated Zirk
/// code uses `zirk_rt_spawn_checked` so quota exhaustion becomes a Throwable.
///
/// # Safety
///
/// Has the same caller, thunk, and capture-pointer requirements as
/// [`zirk_rt_spawn_checked`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_spawn(
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
) -> u64 {
    let mut status = 0;
    // SAFETY: `status` is a writable local slot and the caller provides the
    // same thunk/capture preconditions as this compatibility ABI.
    let job = unsafe { zirk_rt_spawn_checked(body, arg, &mut status) };
    if status != 0 {
        failure::fatal("executor task capacity exceeded through legacy spawn ABI");
    }
    job
}

/// Starts detached work owned by the executor's bounded background supervisor.
///
/// Returns `0` when admitted, `1` when the 256-job background quota is full,
/// `2` once shutdown has started, `3` for duplicate/ID exhaustion, and `4`
/// when the executor's separate active-task quota is full.
/// Detached jobs are cooperatively cancelled when the root task completes.
///
/// # Safety
///
/// Must be called from a running task. `body` and `arg` have the same validity
/// requirements as [`zirk_rt_spawn`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_background_spawn(
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
) -> u32 {
    let arg = TaskArg(arg);
    let capture_root = arg.0;
    let child = move || {
        let arg = arg;
        body(arg.0)
    };
    match executor::spawn_background_with_capture_root(child, capture_root) {
        Ok(_) => 0,
        Err(crate::background::AdmissionError::CapacityReached) => 1,
        Err(crate::background::AdmissionError::Draining) => 2,
        Err(crate::background::AdmissionError::DuplicateJob)
        | Err(crate::background::AdmissionError::IdExhausted) => 3,
        Err(crate::background::AdmissionError::TaskCapacityReached) => 4,
    }
}

/// Consumes the result of the task named by `id` exactly once, suspending the
/// current task until that task is terminal. A second `await` of the same task,
/// or an `await` of a task that no longer exists, is a fatal error.
///
/// # Safety
///
/// Must be called from inside a running task. `id` must come from
/// [`zirk_rt_spawn`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_job_wait(id: u64) -> usize {
    executor::await_task(task::TaskId::from_bits(id))
}

/// Cooperatively waits for one of `count` tasks to become terminal and returns
/// the input index of the task with the earliest serialized completion. If
/// multiple entries have the same completion sequence, the lowest input index
/// wins. Returns `u64::MAX` when cancellation is pending for the caller, so
/// generated code can perform cancellation and join cleanup.
///
/// `ids` must point to `count` packed task IDs produced by [`zirk_rt_spawn`].
/// The count must be in `2..=8`. Before suspending, the runtime validates that
/// all IDs are live, distinct, not the current task, and have not already been
/// awaited or claimed by another waiter. An invalid ID list is a fatal runtime
/// error (exit code 70); it is never reported as caller cancellation. This
/// function is only valid inside an executor task. An out-of-range count is
/// rejected with the `u64::MAX` sentinel before `ids` is inspected. When
/// `count` is in range, invalid pointers are not inspected defensively.
///
/// # Safety
///
/// If `count` is in `2..=8`, `ids` must be non-null, aligned for `u64`, and
/// readable for `count` values. Every value must originate from
/// [`zirk_rt_spawn`]; invalid, stale, duplicate, self, or already-awaited IDs
/// terminate through the runtime's fatal-error path before this call suspends.
/// When `count` is outside that range, the function returns `u64::MAX` without
/// reading `ids`. The caller must be running inside an executor task when
/// `count` is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_job_wait_any(ids: *const u64, count: u64) -> u64 {
    if !(2..=8).contains(&count) {
        return u64::MAX;
    }
    // SAFETY: the public ABI contract requires a valid pointer when count is
    // in range; this path has just checked the 2..=8 bound.
    unsafe { job_wait_any_impl(ids, count) }
}

/// Compiler-only wait-any entry point that includes one private deadline task
/// after at most eight source jobs. The ordinary `zirk_rt_job_wait_any` ABI
/// keeps its stable `2..=8` bound.
///
/// Out-of-range counts return `u64::MAX` before `ids` is inspected. A valid
/// count follows the same ID validation, completion ordering, and cancellation
/// sentinel contract as `zirk_rt_job_wait_any`.
///
/// # Safety
/// If `count` is in `2..=9`, `ids` must be non-null, aligned for `u64`, and
/// readable for `count` values. Every value must be a distinct live task ID
/// created by `zirk_rt_spawn`, and the final value must be the compiler-owned
/// deadline task associated with the pipeline. The caller must be an executor
/// task.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_job_wait_any_with_timer(ids: *const u64, count: u64) -> u64 {
    if !(2..=9).contains(&count) {
        return u64::MAX;
    }
    // SAFETY: the caller's contract requires a valid pointer in this range.
    unsafe { job_wait_any_impl(ids, count) }
}

/// Shared implementation for the two verifier-bounded wait-any ABIs.
///
/// # Safety
/// `ids` must be readable for `count` aligned u64 values.
unsafe fn job_wait_any_impl(ids: *const u64, count: u64) -> u64 {
    // SAFETY: pointer validity and readability for `count` elements are
    // preconditions checked by the ABI wrapper; their range checks bound it.
    let count = count as usize;
    let ids = unsafe { std::slice::from_raw_parts(ids, count) };
    let targets = ids
        .iter()
        .copied()
        .map(task::TaskId::from_bits)
        .collect::<Vec<_>>();
    executor::validate_wait_any_current(&targets);
    if executor::current_task_cancel_requested() {
        return u64::MAX;
    }
    executor::wait_any_current(&targets);

    if executor::current_task_cancel_requested() {
        return u64::MAX;
    }

    executor::select_first_completion_index(targets.iter().enumerate().filter_map(
        |(index, target)| executor::completion_sequence(*target).map(|sequence| (index, sequence)),
    ))
    .map_or(u64::MAX, |index| index as u64)
}

/// Consumes an already-terminal task outcome exactly once and transfers its
/// managed payload into caller-owned slots. Returns `0` for fulfilled and
/// writes the result-box pointer to `value_out`; `1` for a Zirk rejection and
/// writes its `Throwable` pointer to `throwable_out`; or `2` for cancellation,
/// with both slots null. The unused slot is always cleared. Native panic
/// payloads are fatal internal failures and are never exposed as rejections.
///
/// This operation does not wait. It fails closed for stale, nonterminal,
/// current-task, already-consumed, or concurrently claimed handles.
///
/// # Safety
///
/// Must be called from a running executor task. `id` must be a task handle
/// produced by [`zirk_rt_spawn`]. Both output pointers must be non-null,
/// aligned, distinct, writable pointer-sized slots. **Before this call**, the
/// exact addresses of both slots must be registered as reference roots in the
/// caller's shadow-stack frame using [`zirk_rt_push_frame`]. The slots and
/// their containing root frame must remain live until the transferred pointer
/// has been stored into the compiler's correctly typed, recursively traced
/// `Outcome<T>` representation. Code generation must keep these slots
/// initialized to null and rooted across the call; a native local or an
/// unregistered foreign slot is insufficient. The runtime verifies that both
/// addresses occur in the active caller's root chain, but cannot validate
/// their allocation, writability, or lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_job_outcome_take(
    id: u64,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> u32 {
    executor::job_outcome_take_current(task::TaskId::from_bits(id), value_out, throwable_out) as u32
}

/// Cooperatively waits for a spawned job to become terminal and transfers its
/// outcome into caller-owned, already-rooted output slots. Returns `0` for a
/// value, `1` for an unhandled Zirk `Throwable`, `2` when the child was
/// cancelled, or `3` when the waiting caller was cancelled. Caller
/// cancellation leaves the target outcome unconsumed so cleanup can join or
/// retry it. Native panic payloads remain fatal runtime failures.
///
/// The two output slots have the same rooting, alignment, lifetime, and
/// validity requirements as [`zirk_rt_job_outcome_take`]. They are cleared
/// before the caller suspends. The target handle is claimed while waiting, so
/// no second waiter can consume it concurrently.
///
/// # Safety
///
/// Must be called from a running executor task. `id` must identify a live job
/// created by [`zirk_rt_spawn`]. Both output pointers must satisfy the exact
/// requirements documented for [`zirk_rt_job_outcome_take`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_job_outcome_wait_take(
    id: u64,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> u32 {
    executor::job_outcome_wait_take_current(task::TaskId::from_bits(id), value_out, throwable_out)
        as u32
}

/// Waits for the earliest-completing unconsumed child in a collect-all scope
/// and transfers its outcome into caller-owned rooted slots. Returns 0 for a
/// fulfilled value, 1 for a language rejection, 2 for child cancellation, 3
/// for caller cancellation (no child result is consumed), and 4 when every
/// child result has been consumed. For status 4, `ordinal_out` is `u64::MAX`.
/// Otherwise `ordinal_out` is the child's zero-based registration ordinal.
///
/// The scope is bounded to 256 registered children. Its entire branch set is
/// validated before output pointers are inspected. The two reference output
/// slots must be registered roots in the caller's active shadow-stack chain;
/// values are published there before the child TCB releases its result root.
///
/// # Safety
/// Must run inside an executor task. `scope` must be a live collect-all scope
/// returned by [`zirk_rt_scope_enter_collect_all`]. All three output pointers
/// must be non-null, aligned, writable, live for this call, and pairwise
/// distinct; `value_out` and `throwable_out` must satisfy the root contract of
/// [`zirk_rt_job_outcome_take`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_scope_outcome_wait_take(
    scope: u64,
    ordinal_out: *mut u64,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> u32 {
    executor::scope_outcome_wait_take_current(
        task::ScopeId::from_bits(scope),
        ordinal_out,
        value_out,
        throwable_out,
    ) as u32
}

/// Whether the task named by `id` has reached a terminal state. Safe on a stale
/// id (returns `false`).
///
/// # Safety
///
/// Must be called from inside a running task.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_job_done(id: u64) -> bool {
    executor::is_done(task::TaskId::from_bits(id))
}

/// Opens a structured-concurrency scope in the running executor and returns its
/// packed [`task::ScopeId`].
///
/// # Safety
///
/// Must be called from inside a running task driven by [`zirk_rt_run_main`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_scope_enter() -> u64 {
    executor::scope_enter_current().to_bits()
}

/// Opens a child scope that joins every child without fail-fast cancellation
/// or propagation of child failures. Each child's terminal outcome remains
/// available through its job handle after the scope closes.
///
/// # Safety
///
/// Must be called from inside a task driven by [`zirk_rt_run_main`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_scope_enter_collect_all() -> u64 {
    executor::scope_enter_collect_all_current().to_bits()
}

/// Requests cooperative cancellation of all branches in a live scope.
///
/// # Safety
/// Must run inside an executor task. `scope` must be a live scope handle
/// obtained from one of the `zirk_rt_scope_enter*` functions and must not have
/// been closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_scope_cancel(scope: u64) {
    executor::scope_cancel_current(task::ScopeId::from_bits(scope));
}

/// Runs the join-or-cancel-and-clean protocol for `scope`, yielding until every
/// registered branch is terminal, then re-raises a primary branch failure into
/// the pending-exception slot and returns.
///
/// # Safety
///
/// Must be called from inside a running task. `scope` must come from
/// [`zirk_rt_scope_enter`] and not already have been closed.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_scope_exit(scope: u64) -> bool {
    let scope = task::ScopeId::from_bits(scope);
    loop {
        match executor::scope_exit_poll(scope) {
            None => executor::yield_now(),
            Some(primary_failure) => {
                if let Some(bits) = primary_failure {
                    // Re-raise the branch's exception into this frame; the
                    // enclosing function's own pending-exception check picks
                    // it up right after the `concurrent` block.
                    unsafe { exceptions::zirk_rt_throw(bits as *const std::ffi::c_void) };
                }
                return true;
            }
        }
    }
}

/// Closes a collect-all scope after every child is terminal and cleanup has
/// completed. Child rejection and cancellation are retained as child outcomes
/// and do not cancel siblings or unwind into the caller. Cancellation of the
/// caller retains the executor's normal cancellation and cleanup behavior.
///
/// # Safety
///
/// Must be called inside a running task with a live handle returned by
/// [`zirk_rt_scope_enter_collect_all`], exactly once. A null, stale, or already
/// closed handle is an invalid runtime call and terminates through the fatal
/// runtime path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_scope_exit_collect_all(scope: u64) -> bool {
    let scope = task::ScopeId::from_bits(scope);
    loop {
        match executor::scope_exit_collect_all_poll(scope) {
            None => executor::yield_now(),
            Some(()) => return true,
        }
    }
}

/// A `concurrent` branch thunk calls this after its body returned with the
/// pending-exception slot set: it carries the taken exception pointer into an
/// unwind the executor records as a branch failure, so the owning scope
/// cancels the siblings and re-raises it on close.
///
/// # Safety
///
/// Must be called from inside a running branch task. `exception` must be the
/// non-null object pointer just taken from the pending slot.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_branch_fail(exception: *const std::ffi::c_void) -> ! {
    if exception.is_null() {
        failure::fatal("branch failure requires a non-null Throwable");
    }
    std::panic::panic_any(executor::BranchFailure(exception as usize));
}

/// Registers `branch` as a joinable child of `scope`.
///
/// # Safety
///
/// Must be called from inside a running task. `scope` must come from
/// [`zirk_rt_scope_enter`] and `branch` from [`zirk_rt_spawn`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_branch_register(scope: u64, branch: u64) {
    executor::branch_register_current(
        task::ScopeId::from_bits(scope),
        task::TaskId::from_bits(branch),
    );
}

/// Requests cooperative cancellation of `job`, idempotently.
///
/// # Safety
///
/// Must be called from inside a running task. `job` must come from
/// [`zirk_rt_spawn`] (a stale id is ignored).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_cancel(job: u64) {
    executor::cancel_current(task::TaskId::from_bits(job));
}

/// Opts `job` into promoting Throwables raised during its cancellation cleanup
/// to a rejected managed outcome, then requests ordinary cooperative cancel.
///
/// # Safety
/// Must be called from a running task. `job` must be an ID returned by
/// [`zirk_rt_spawn`] (a stale ID is ignored).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_cancel_with_cleanup_failure(job: u64) {
    executor::cancel_with_cleanup_failure_current(task::TaskId::from_bits(job));
}

/// Suspends the current branch until a monotonic deadline `delay_nanos` from
/// now — a cooperative-cancellation safe point. Returns 0 on success, 2 when
/// timer capacity is exhausted, or 3 for a negative duration.
///
/// # Safety
///
/// Must be called from inside a running task.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_sleep_checked(delay_nanos: i64) -> u32 {
    match executor::try_sleep(delay_nanos) {
        Ok(()) => 0,
        Err(crate::timer::TimerError::CapacityExceeded) => 2,
        Err(crate::timer::TimerError::NegativeDuration) => 3,
    }
}

/// Configures the cooperative grace used to cancel and drain `App.background`
/// when the root task completes. Returns 0 on success and 1 for a negative
/// duration or if draining has already started. The default remains five seconds.
///
/// # Safety
/// Must be called from inside the root task driven by `zirk_rt_run_main`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_app_set_shutdown_grace(delay_nanos: i64) -> u32 {
    if executor::set_background_shutdown_grace(delay_nanos) {
        0
    } else {
        1
    }
}

/// Registers the app-scoped managed background error handler.
///
/// The callback is invoked only by an internal executor task. `capture` is a
/// traced callable capture block and remains rooted until delivery completes.
/// `target` is the LLVM callable entry point emitted with the matching layout.
///
/// # Safety
/// Must be called from a running task. `thunk`, `target`, and `capture` must
/// come from the compiler-emitted callable with one matching layout; `target`
/// must be non-null and executable, and `capture` must remain rooted in the
/// caller's active managed frame until this function returns. The runtime
/// installs its own root before that frame can leave.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_app_set_background_error(
    thunk: executor::ManagedBackgroundErrorHandlerThunk,
    target: *mut std::ffi::c_void,
    capture: *mut std::ffi::c_void,
) {
    assert!(
        !target.is_null(),
        "background handler target cannot be null"
    );
    executor::set_current_managed_background_error_handler(
        executor::ManagedBackgroundErrorHandler {
            thunk,
            target: target as usize,
            capture: capture as usize,
        },
    );
}

/// Records a thrown error from the managed background error callback. The
/// Throwable is retained in the executor root set through task completion;
/// recursive delivery is suppressed by the active-delivery state.
///
/// # Safety
/// Must be called only from the compiler-emitted background-handler thunk
/// running as an executor task. A non-null `throwable` must point to a valid
/// managed Throwable object that is rooted in the current task frame until
/// this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_background_handler_failed(throwable: *mut std::ffi::c_void) {
    if !throwable.is_null() {
        executor::report_current_background_handler_failure(throwable as usize);
    }
}

/// Compatibility entry point for native consumers that do not use typed
/// admission statuses. Generated Zirk code uses `zirk_rt_sleep_checked`.
///
/// # Safety
///
/// Must be called from inside a running task, as required by
/// [`zirk_rt_sleep_checked`].
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_sleep(delay_nanos: i64) {
    let status = unsafe { zirk_rt_sleep_checked(delay_nanos) };
    if status != 0 {
        failure::fatal("timer sleep failed through legacy sleep ABI");
    }
}

/// Arms a one-shot ambient timer job in `scope`. The callback runs after the
/// delay unless the owning scope closes first. Returns its job ID with status
/// 0; statuses 1, 2, and 3 report task capacity, timer capacity, and negative
/// duration respectively. No task or callback root is retained on failure.
///
/// # Safety
///
/// Must be called from inside a running task. `scope` must come from
/// [`zirk_rt_scope_enter`]; `body` must be a valid thunk and `arg` its
/// compiler-emitted capture-block pointer (or null). `status_out` must be
/// non-null, aligned, writable, and live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timer_after_checked(
    scope: u64,
    delay_nanos: i64,
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
    status_out: *mut u32,
) -> u64 {
    if status_out.is_null() {
        failure::fatal("zirk_rt_timer_after requires a non-null status output");
    }
    let arg = TaskArg(arg);
    let capture_root = arg.0;
    let timer = executor::try_spawn_timer_with_capture_root(
        move || {
            let arg = arg;
            executor::sleep(delay_nanos);
            if executor::current_task_cancel_requested() {
                return 0;
            }
            body(arg.0)
        },
        capture_root,
        delay_nanos,
    );
    let timer = match timer {
        Ok(timer) => timer,
        Err(executor::TimerTaskAdmissionError::TaskCapacity) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 1 };
            return 0;
        }
        Err(executor::TimerTaskAdmissionError::TimerCapacity) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 2 };
            return 0;
        }
        Err(executor::TimerTaskAdmissionError::NegativeDuration) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 3 };
            return 0;
        }
    };
    executor::ambient_timer_register_current(task::ScopeId::from_bits(scope), timer);
    // SAFETY: the ABI requires a non-null, writable status slot.
    unsafe { *status_out = 0 };
    timer.to_bits()
}

/// # Safety
///
/// Has the same scope, thunk, and capture-pointer requirements as
/// [`zirk_rt_timer_after_checked`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timer_after(
    scope: u64,
    delay_nanos: i64,
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
) -> u64 {
    let mut status = 0;
    // SAFETY: local status output and preserved legacy argument contract.
    let job = unsafe { zirk_rt_timer_after_checked(scope, delay_nanos, body, arg, &mut status) };
    if status != 0 {
        failure::fatal("timer admission failed through legacy timer_after ABI");
    }
    job
}

/// Arms a fixed-delay ambient timer job in `scope`. Each delay is armed only
/// after the preceding callback returns, and cancellation is observed by the
/// `sleep` safe point before the next callback can run. Status values match
/// [`zirk_rt_timer_after_checked`].
///
/// # Safety
///
/// Must be called from inside a running task. `scope` must come from
/// [`zirk_rt_scope_enter`]; `body` must be a valid thunk and `arg` its
/// compiler-emitted capture-block pointer (or null). `status_out` must be
/// non-null, aligned, writable, and live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timer_every_checked(
    scope: u64,
    delay_nanos: i64,
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
    status_out: *mut u32,
) -> u64 {
    if status_out.is_null() {
        failure::fatal("zirk_rt_timer_every requires a non-null status output");
    }
    let arg = TaskArg(arg);
    let capture_root = arg.0;
    let timer = executor::try_spawn_timer_with_capture_root(
        move || {
            let arg = arg;
            loop {
                executor::sleep(delay_nanos);
                if executor::current_task_cancel_requested() {
                    return 0;
                }
                body(arg.0);
            }
        },
        capture_root,
        delay_nanos,
    );
    let timer = match timer {
        Ok(timer) => timer,
        Err(executor::TimerTaskAdmissionError::TaskCapacity) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 1 };
            return 0;
        }
        Err(executor::TimerTaskAdmissionError::TimerCapacity) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 2 };
            return 0;
        }
        Err(executor::TimerTaskAdmissionError::NegativeDuration) => {
            // SAFETY: the ABI requires a non-null, writable status slot.
            unsafe { *status_out = 3 };
            return 0;
        }
    };
    executor::ambient_timer_register_current(task::ScopeId::from_bits(scope), timer);
    // SAFETY: the ABI requires a non-null, writable status slot.
    unsafe { *status_out = 0 };
    timer.to_bits()
}

/// # Safety
///
/// Has the same scope, thunk, and capture-pointer requirements as
/// [`zirk_rt_timer_every_checked`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_timer_every(
    scope: u64,
    delay_nanos: i64,
    body: extern "C-unwind" fn(*mut std::ffi::c_void) -> usize,
    arg: *mut std::ffi::c_void,
) -> u64 {
    let mut status = 0;
    // SAFETY: local status output and preserved legacy argument contract.
    let job = unsafe { zirk_rt_timer_every_checked(scope, delay_nanos, body, arg, &mut status) };
    if status != 0 {
        failure::fatal("timer admission failed through legacy timer_every ABI");
    }
    job
}

/// Shuts the runtime down after `main` returns.
///
/// Corresponds to the "ordered shutdown of resources and managed threads" and
/// the "stream flush" of `ZIRK_RUNTIME_SPEC.md` section 2.
///
/// # Safety
///
/// Invoked by Zirk-generated code exactly once, after `main` and before the
/// process ends. Using any runtime function after this call is not supported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_shutdown() {
    // Step 6 of the ordered shutdown in `ZIRK_RUNTIME_SPEC.md` section 11.
    // The managed blocking pool drains and joins before stream flush so its
    // worker registrations cannot outlive the published heap/runtime.
    managed_blocking::shutdown_managed_pool();
    io::flush();
}

/// Allocates a boxed callable's capture block (roadmap Phase 4d,
/// `phase-4d-callables`, task 5).
///
/// The returned block is a normal GC object: its header carries the supplied
/// descriptor so the collector can trace any captured managed references, and
/// `zirk_rt_clone` can deep-copy them for `.clone()`.
///
/// # Safety
///
/// `descriptor` must be a compiler-emitted closure descriptor and `size` must
/// be the payload size in bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_alloc_callable(
    descriptor: *const std::ffi::c_void,
    size: u64,
) -> *mut std::ffi::c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let total = (size as usize).saturating_add(crate::collector::HEADER_BYTES);
    let pointer = unsafe { crate::memory::zirk_rt_alloc(total, std::mem::size_of::<usize>()) };
    // The descriptor word is left zero by `zirk_rt_alloc`.  No collection can
    // run between the return and this write (single-threaded, collections only
    // start at the top of `zirk_rt_alloc`), so the momentary null is safe.
    unsafe { *(pointer as *mut *const std::ffi::c_void) = descriptor };
    pointer
}

/// Clones a boxed callable's capture block (roadmap Phase 4d,
/// `phase-4d-callables`, task 6).
///
/// # Safety
///
/// `callable` must be a capture block pointer returned by `zirk_rt_alloc_callable`
/// or a previous `zirk_rt_clone_callable`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_clone_callable(
    callable: *const std::ffi::c_void,
) -> *mut std::ffi::c_void {
    if callable.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { crate::clone::zirk_rt_clone(callable as *mut std::ffi::c_void) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimal_lifecycle_is_callable() {
        unsafe {
            zirk_rt_init();
            zirk_rt_shutdown();
        }
    }

    #[test]
    fn run_main_drives_a_background_child_to_completion() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static MAIN_RAN: AtomicUsize = AtomicUsize::new(0);
        static CHILD_DONE: AtomicUsize = AtomicUsize::new(0);

        // Stand-in for the compiler-emitted Zirk entrypoint: it spawns a
        // background task and returns while that task is still running.
        extern "C-unwind" fn fake_zirk_main() {
            MAIN_RAN.store(1, Ordering::SeqCst);
            executor::spawn(|| {
                for _ in 0..8 {
                    executor::yield_now();
                }
                CHILD_DONE.store(1, Ordering::SeqCst);
                0
            });
        }

        MAIN_RAN.store(0, Ordering::SeqCst);
        CHILD_DONE.store(0, Ordering::SeqCst);
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(fake_zirk_main);
            zirk_rt_shutdown();
        }
        assert_eq!(MAIN_RAN.load(Ordering::SeqCst), 1);
        assert_eq!(
            CHILD_DONE.load(Ordering::SeqCst),
            1,
            "zirk_rt_run_main returned before the background task finished"
        );
    }

    #[test]
    fn shutdown_releases_executor_heap_tasks_timers_and_gc_participants() {
        unsafe extern "C-unwind" fn managed_callback(
            _target: *mut std::ffi::c_void,
            _captures: *mut std::ffi::c_void,
            _result: *mut std::ffi::c_void,
            _exception: *mut std::ffi::c_void,
        ) -> u32 {
            MANAGED_CALL_OK
        }

        extern "C-unwind" fn root() {
            let mut result = 0usize;
            let mut exception = 0usize;
            let target = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
            // SAFETY: the tiny callback returns synchronously and ignores its
            // arguments; output slots stay live until managed join returns.
            let status = unsafe {
                zirk_rt_thread_run(
                    managed_callback,
                    target,
                    std::ptr::null_mut(),
                    (&mut result as *mut usize).cast(),
                    (&mut exception as *mut usize).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_OK);
        }

        // SAFETY: `root` is a valid no-argument runtime entrypoint.
        unsafe { zirk_rt_run_main(root) };

        // Serialize the post-run snapshot with any concurrently starting
        // executor so this assertion observes this run's fully released state.
        let _gc_world = executor::lock_gc_world_for_test();
        assert_eq!(
            collector::shutdown_snapshot_for_tests(),
            (false, 0),
            "executor heap or managed GC participant remained published after shutdown"
        );
    }

    #[test]
    fn background_c_abi_admits_work_and_shutdown_cancels_it_cooperatively() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static CLEANED: AtomicBool = AtomicBool::new(false);

        extern "C-unwind" fn worker(_: *mut std::ffi::c_void) -> usize {
            while !executor::current_task_is_cancelled() {
                executor::sleep(60_000_000_000);
            }
            CLEANED.store(true, Ordering::SeqCst);
            0
        }

        extern "C-unwind" fn driver() {
            assert_eq!(
                unsafe { zirk_rt_background_spawn(worker, std::ptr::null_mut()) },
                0
            );
            executor::yield_now();
        }

        CLEANED.store(false, Ordering::SeqCst);
        unsafe { zirk_rt_run_main(driver) };
        assert!(CLEANED.load(Ordering::SeqCst));
    }

    #[test]
    fn background_c_abi_reports_executor_task_capacity_separately() {
        extern "C-unwind" fn unused_worker(_: *mut std::ffi::c_void) -> usize {
            0
        }
        extern "C-unwind" fn driver() {
            for _ in 0..(crate::task::MAX_ACTIVE_TASKS - 1) {
                executor::try_spawn(|| 0).unwrap();
            }
            assert_eq!(
                unsafe { zirk_rt_background_spawn(unused_worker, std::ptr::null_mut()) },
                4,
                "general executor exhaustion must not masquerade as background quota"
            );
        }

        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(driver);
            zirk_rt_shutdown();
        }
    }

    #[test]
    fn the_c_abi_task_surface_spawns_awaits_and_reports_done() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static RESULT: AtomicU64 = AtomicU64::new(0);

        extern "C-unwind" fn child(arg: *mut std::ffi::c_void) -> usize {
            // `arg` carries a plain integer for this test.
            for _ in 0..4 {
                executor::yield_now();
            }
            arg as usize + 1
        }

        extern "C-unwind" fn driver() {
            let id = unsafe { zirk_rt_spawn(child, 41 as *mut std::ffi::c_void) };
            assert!(
                !unsafe { zirk_rt_job_done(id) },
                "child ran before a safe point"
            );
            let value = unsafe { zirk_rt_job_wait(id) };
            assert!(unsafe { zirk_rt_job_done(id) });
            RESULT.store(value as u64, Ordering::SeqCst);
        }

        RESULT.store(0, Ordering::SeqCst);
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(driver);
            zirk_rt_shutdown();
        }
        assert_eq!(RESULT.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn the_c_abi_timer_surface_runs_and_stops_ambient_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static AFTER: AtomicUsize = AtomicUsize::new(0);
        static TICKS: AtomicUsize = AtomicUsize::new(0);

        extern "C-unwind" fn after(_: *mut std::ffi::c_void) -> usize {
            AFTER.fetch_add(1, Ordering::SeqCst);
            0
        }

        extern "C-unwind" fn tick(_: *mut std::ffi::c_void) -> usize {
            TICKS.fetch_add(1, Ordering::SeqCst);
            0
        }

        extern "C-unwind" fn driver() {
            let scope = unsafe { zirk_rt_scope_enter() };
            unsafe {
                zirk_rt_timer_after(scope, 2_000_000, after, std::ptr::null_mut());
                zirk_rt_timer_every(scope, 2_000_000, tick, std::ptr::null_mut());
                // Keep the timer scope alive until the repeating callback has
                // actually re-armed, rather than relying on a fixed sleep that
                // can expire before the executor gets CPU on a loaded runner.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
                while TICKS.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
                    zirk_rt_sleep(2_000_000);
                }
            }
            assert!(unsafe { zirk_rt_scope_exit(scope) });
        }

        AFTER.store(0, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        unsafe { zirk_rt_run_main(driver) };
        let ticks_at_exit = TICKS.load(Ordering::SeqCst);
        assert_eq!(AFTER.load(Ordering::SeqCst), 1);
        assert!(ticks_at_exit >= 2, "Timer.every did not re-arm");
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(TICKS.load(Ordering::SeqCst), ticks_at_exit);
    }
}
