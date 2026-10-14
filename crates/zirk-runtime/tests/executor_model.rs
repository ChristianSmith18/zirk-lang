//! Deterministic, seeded public-ABI traces for cancellation and completion.
//!
//! This intentionally exercises only the runtime surface generated code uses;
//! it does not inspect private scheduler queues or rely on timing sleeps.

use std::sync::{Mutex, OnceLock};

use zirk_runtime::{
    zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_job_wait, zirk_rt_run_main,
    zirk_rt_scope_enter, zirk_rt_scope_exit, zirk_rt_spawn,
};

#[derive(Default)]
struct Trace {
    expected: Vec<usize>,
    started: Vec<usize>,
    finished: Vec<usize>,
    joined: Vec<(usize, usize)>,
    cancelled_seen: Vec<usize>,
}

static TRACE: OnceLock<Mutex<Trace>> = OnceLock::new();

fn trace() -> &'static Mutex<Trace> {
    TRACE.get_or_init(|| Mutex::new(Trace::default()))
}

fn record(f: impl FnOnce(&mut Trace)) {
    let mut guard = trace().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard);
}

fn next(seed: &mut u64) -> u64 {
    // Small fixed xorshift generator: reproducible across platforms and runs.
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

extern "C-unwind" fn target(arg: *mut std::ffi::c_void) -> usize {
    let id = arg as usize;
    record(|t| t.started.push(id));
    // A cancelled ready task still gets scheduled once; generated cleanup
    // code observes the cancellation state before publishing its result.
    if unsafe { zirk_runtime::zirk_rt_is_cancelled() } {
        record(|t| t.cancelled_seen.push(id));
    }
    let result = id * 17 + 3;
    record(|t| t.finished.push(id));
    result
}

struct JoinArg {
    target: u64,
    logical_id: usize,
}

extern "C-unwind" fn join_target(arg: *mut std::ffi::c_void) -> usize {
    // The test allocates the argument for the duration of run_with_root; the
    // runtime contract retains it until this task has returned.
    let join = unsafe { &*(arg as *const JoinArg) };
    let value = unsafe { zirk_rt_job_wait(join.target) };
    record(|t| t.joined.push((join.logical_id, value)));
    value
}

static mut JOIN_ARGS: [JoinArg; 32] = [const {
    JoinArg {
        target: 0,
        logical_id: 0,
    }
}; 32];

extern "C-unwind" fn driver() {
    let (count, cancel_mask, join_mask) = {
        let t = trace().lock().unwrap_or_else(|e| e.into_inner());
        (t.expected.len(), CANCEL_MASK.load(), JOIN_MASK.load())
    };
    let scope = unsafe { zirk_rt_scope_enter() };
    let mut handles = [0u64; 32];
    for (id, handle_slot) in handles.iter_mut().enumerate().take(count) {
        let handle = unsafe { zirk_rt_spawn(target, id as *mut _) };
        *handle_slot = handle;
        unsafe { zirk_rt_branch_register(scope, handle) };
    }

    // Register independent waiters before yielding. The seeded mask chooses
    // which targets receive a waiter; canceled targets are never consumed.
    for (id, handle) in handles.iter().copied().enumerate().take(count) {
        if join_mask & (1u32 << id) == 0 || cancel_mask & (1u32 << id) != 0 {
            continue;
        }
        let args = std::ptr::addr_of_mut!(JOIN_ARGS);
        unsafe {
            (*args)[id] = JoinArg {
                target: handle,
                logical_id: id,
            };
        }
        let arg = unsafe { std::ptr::addr_of_mut!((*args)[id]) }.cast();
        let waiter = unsafe { zirk_rt_spawn(join_target, arg) };
        unsafe { zirk_rt_branch_register(scope, waiter) };
    }

    for (id, handle) in handles.iter().copied().enumerate().take(count) {
        if cancel_mask & (1u32 << id) != 0 {
            unsafe { zirk_rt_cancel(handle) };
        }
    }
    assert!(unsafe { zirk_rt_scope_exit(scope) });
}

static CANCEL_MASK: AtomicMask = AtomicMask(std::sync::atomic::AtomicU32::new(0));
static JOIN_MASK: AtomicMask = AtomicMask(std::sync::atomic::AtomicU32::new(0));

struct AtomicMask(std::sync::atomic::AtomicU32);
impl AtomicMask {
    fn load(&self) -> u32 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn store(&self, value: u32) {
        self.0.store(value, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn seeded_cancel_completion_and_waiter_traces_preserve_invariants() {
    const MAX: usize = 12;
    for initial_seed in [1, 7, 0x5eed, 0xdecafbad, u64::MAX - 1] {
        let mut seed = initial_seed;
        for round in 0..24 {
            let count = 1 + (next(&mut seed) as usize % MAX);
            let valid = (1u32 << count) - 1;
            let cancel_mask = (next(&mut seed) as u32) & valid;
            let join_mask = (next(&mut seed) as u32) & valid & !cancel_mask;

            CANCEL_MASK.store(cancel_mask);
            JOIN_MASK.store(join_mask);
            record(|t| {
                *t = Trace {
                    expected: (0..count).collect(),
                    ..Trace::default()
                }
            });

            // SAFETY: the ABI entrypoint is called once per fresh executor;
            // callbacks and their encoded integer arguments remain valid for
            // each complete scheduler run.
            unsafe { zirk_rt_run_main(driver) };

            let t = trace().lock().unwrap_or_else(|e| e.into_inner());
            let mut started = t.started.clone();
            let mut finished = t.finished.clone();
            let mut canceled = t.cancelled_seen.clone();
            started.sort_unstable();
            finished.sort_unstable();
            canceled.sort_unstable();
            assert_eq!(started, t.expected, "seed={initial_seed:#x} round={round}");
            assert_eq!(finished, t.expected, "seed={initial_seed:#x} round={round}");
            assert_eq!(
                canceled,
                (0..count)
                    .filter(|i| cancel_mask & (1 << i) != 0)
                    .collect::<Vec<_>>()
            );

            let mut joined = t.joined.clone();
            joined.sort_unstable_by_key(|(id, _)| *id);
            let expected_joined = (0..count)
                .filter(|id| join_mask & (1 << id) != 0)
                .map(|id| (id, id * 17 + 3))
                .collect::<Vec<_>>();
            assert_eq!(
                joined, expected_joined,
                "seed={initial_seed:#x} round={round}"
            );
        }
    }
}
