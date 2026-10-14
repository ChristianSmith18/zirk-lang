//! Deterministic cancellation/join model using only the runtime's public C ABI.
//!
//! The runtime does not expose a public wait-any operation yet. Selection is
//! therefore modeled as probing `job_done` and consuming one ready result with
//! `job_wait`, matching the primitives currently available to generated code.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use zirk_runtime::{
    zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_init, zirk_rt_job_done, zirk_rt_job_wait,
    zirk_rt_run_main, zirk_rt_scope_enter, zirk_rt_scope_exit, zirk_rt_shutdown, zirk_rt_sleep,
    zirk_rt_spawn,
};

const TARGETS: usize = 8;
const VALUES: usize = 4;
const ROUNDS: usize = 16;
const SLOTS: usize = TARGETS * ROUNDS;
const CANCELLED: usize = usize::MAX;
const PARK_NANOS: i64 = 86_400_000_000_000;
const MAX_BARRIER_POLLS: usize = 30_000;

static ROUND: AtomicUsize = AtomicUsize::new(0);
static SEED: AtomicU64 = AtomicU64::new(0);
static TARGET_IDS: [AtomicU64; TARGETS] = [const { AtomicU64::new(0) }; TARGETS];
static VALUE_IDS: [AtomicU64; VALUES] = [const { AtomicU64::new(0) }; VALUES];
static TARGET_STARTS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static TARGET_RETURNS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static VALUE_STARTS: [AtomicUsize; VALUES * ROUNDS] =
    [const { AtomicUsize::new(0) }; VALUES * ROUNDS];
static VALUE_RETURNS: [AtomicUsize; VALUES * ROUNDS] =
    [const { AtomicUsize::new(0) }; VALUES * ROUNDS];
static CANCEL_STARTS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static CANCEL_RETURNS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static CANCEL_CALLS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static WAITER_STARTS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static WAITER_RETURNS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static WAITER_RESULTS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static TARGET_BARRIER_STARTS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];
static TARGET_BARRIER_RETURNS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];
static VALUE_BARRIER_STARTS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];
static VALUE_BARRIER_RETURNS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];
static CANCEL_BARRIER_STARTS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];
static CANCEL_BARRIER_RETURNS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];

fn round() -> usize {
    ROUND.load(Ordering::SeqCst)
}

fn slot(index: usize) -> usize {
    round() * TARGETS + index
}

fn value_slot(index: usize) -> usize {
    round() * VALUES + index
}

fn next_seed(mut x: u64) -> u64 {
    // xorshift64*; the fixed seed list below is nonzero and reproducible.
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn expected_value(index: usize) -> usize {
    let seed = SEED.load(Ordering::SeqCst) as usize;
    0x5a00_0000usize ^ (round() << 12) ^ (index << 4) ^ (seed & 0xffff)
}

fn spawn(body: extern "C-unwind" fn(*mut c_void) -> usize, scope: u64) -> u64 {
    // All test callbacks use static state and null captures, so the public ABI
    // does not mistake an arbitrary Rust pointer for a managed GC root.
    let id = unsafe { zirk_rt_spawn(body, std::ptr::null_mut()) };
    unsafe { zirk_rt_branch_register(scope, id) };
    id
}

fn run_target(index: usize) -> usize {
    let s = slot(index);
    TARGET_STARTS[s].fetch_add(1, Ordering::SeqCst);
    // A long timer makes every target stay parked until a cancellation request
    // wakes it. No assertion depends on wall-clock scheduling latency.
    unsafe { zirk_rt_sleep(PARK_NANOS) };
    TARGET_RETURNS[s].fetch_add(1, Ordering::SeqCst);
    0x7000 + index
}

macro_rules! target_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            run_target($index)
        }
    };
}

target_thunk!(target_0, 0);
target_thunk!(target_1, 1);
target_thunk!(target_2, 2);
target_thunk!(target_3, 3);
target_thunk!(target_4, 4);
target_thunk!(target_5, 5);
target_thunk!(target_6, 6);
target_thunk!(target_7, 7);

const TARGET_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; TARGETS] = [
    target_0, target_1, target_2, target_3, target_4, target_5, target_6, target_7,
];

fn run_value(index: usize) -> usize {
    let s = value_slot(index);
    VALUE_STARTS[s].fetch_add(1, Ordering::SeqCst);
    let value = expected_value(index);
    VALUE_RETURNS[s].fetch_add(1, Ordering::SeqCst);
    value
}

macro_rules! value_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            run_value($index)
        }
    };
}

value_thunk!(value_0, 0);
value_thunk!(value_1, 1);
value_thunk!(value_2, 2);
value_thunk!(value_3, 3);

const VALUE_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; VALUES] =
    [value_0, value_1, value_2, value_3];

extern "C-unwind" fn value_barrier(_: *mut c_void) -> usize {
    VALUE_BARRIER_STARTS[round()].fetch_add(1, Ordering::SeqCst);
    for _ in 0..MAX_BARRIER_POLLS {
        let ready =
            (0..VALUES).all(|i| unsafe { zirk_rt_job_done(VALUE_IDS[i].load(Ordering::SeqCst)) });
        if ready {
            VALUE_BARRIER_RETURNS[round()].fetch_add(1, Ordering::SeqCst);
            return 1;
        }
        // Yield cooperatively while waiting; this is a predicate wait, not a
        // timing assumption about how quickly other tasks run.
        unsafe { zirk_rt_sleep(1_000_000) };
    }
    panic!("value barrier exceeded its bounded polling budget");
}

extern "C-unwind" fn target_barrier(_: *mut c_void) -> usize {
    TARGET_BARRIER_STARTS[round()].fetch_add(1, Ordering::SeqCst);
    for _ in 0..MAX_BARRIER_POLLS {
        if (0..TARGETS).all(|i| TARGET_STARTS[slot(i)].load(Ordering::SeqCst) == 1) {
            // Each target increments its start count immediately before
            // parking. Since this barrier can only run after a target yields,
            // all targets are parked when this callback returns.
            TARGET_BARRIER_RETURNS[round()].fetch_add(1, Ordering::SeqCst);
            return TARGETS;
        }
        unsafe { zirk_rt_sleep(1_000_000) };
    }
    panic!("target barrier exceeded its bounded polling budget");
}

extern "C-unwind" fn canceled_targets_barrier(_: *mut c_void) -> usize {
    CANCEL_BARRIER_STARTS[round()].fetch_add(1, Ordering::SeqCst);
    for _ in 0..MAX_BARRIER_POLLS {
        let terminal =
            (0..TARGETS).all(|i| unsafe { zirk_rt_job_done(TARGET_IDS[i].load(Ordering::SeqCst)) });
        if terminal {
            CANCEL_BARRIER_RETURNS[round()].fetch_add(1, Ordering::SeqCst);
            return TARGETS;
        }
        unsafe { zirk_rt_sleep(1_000_000) };
    }
    panic!("cancellation barrier exceeded its bounded polling budget");
}

fn run_canceller(index: usize) -> usize {
    let s = slot(index);
    CANCEL_STARTS[s].fetch_add(1, Ordering::SeqCst);
    let seed = SEED.load(Ordering::SeqCst) as usize;
    // Eight is a power of two, and 3 is coprime to it, so each round's mapping
    // is a permutation: every target is canceled by one independently
    // scheduled task. A duplicate request checks idempotence.
    let target = (index * 3 + (seed & (TARGETS - 1))) & (TARGETS - 1);
    let id = TARGET_IDS[target].load(Ordering::SeqCst);
    unsafe {
        zirk_rt_cancel(id);
        CANCEL_CALLS[s].fetch_add(1, Ordering::SeqCst);
        zirk_rt_cancel(id);
        CANCEL_CALLS[s].fetch_add(1, Ordering::SeqCst);
    }
    CANCEL_RETURNS[s].fetch_add(1, Ordering::SeqCst);
    target
}

macro_rules! canceller_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            run_canceller($index)
        }
    };
}

canceller_thunk!(canceller_0, 0);
canceller_thunk!(canceller_1, 1);
canceller_thunk!(canceller_2, 2);
canceller_thunk!(canceller_3, 3);
canceller_thunk!(canceller_4, 4);
canceller_thunk!(canceller_5, 5);
canceller_thunk!(canceller_6, 6);
canceller_thunk!(canceller_7, 7);

const CANCELLER_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; TARGETS] = [
    canceller_0,
    canceller_1,
    canceller_2,
    canceller_3,
    canceller_4,
    canceller_5,
    canceller_6,
    canceller_7,
];

fn run_waiter(index: usize) -> usize {
    let s = slot(index);
    WAITER_STARTS[s].fetch_add(1, Ordering::SeqCst);
    let target = TARGET_IDS[index].load(Ordering::SeqCst);
    let result = catch_unwind(AssertUnwindSafe(|| unsafe { zirk_rt_job_wait(target) }));
    match result {
        Err(_) => {
            WAITER_RESULTS[s].store(CANCELLED, Ordering::SeqCst);
            WAITER_RETURNS[s].fetch_add(1, Ordering::SeqCst);
            CANCELLED
        }
        Ok(value) => {
            WAITER_RESULTS[s].store(value, Ordering::SeqCst);
            WAITER_RETURNS[s].fetch_add(1, Ordering::SeqCst);
            value
        }
    }
}

macro_rules! waiter_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            run_waiter($index)
        }
    };
}

waiter_thunk!(waiter_0, 0);
waiter_thunk!(waiter_1, 1);
waiter_thunk!(waiter_2, 2);
waiter_thunk!(waiter_3, 3);
waiter_thunk!(waiter_4, 4);
waiter_thunk!(waiter_5, 5);
waiter_thunk!(waiter_6, 6);
waiter_thunk!(waiter_7, 7);

const WAITER_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; TARGETS] = [
    waiter_0, waiter_1, waiter_2, waiter_3, waiter_4, waiter_5, waiter_6, waiter_7,
];

extern "C-unwind" fn driver() {
    let seeds = [
        0x243f_6a88_85a3_08d3,
        0x1319_8a2e_0370_7344,
        0xa409_3822_299f_31d0,
        0x082e_fa98_ec4e_6c89,
        0x4528_21e6_38d0_1377,
        0xbe54_66cf_34e9_0c6c,
        0xc0ac_29b7_c97c_50dd,
        0x3f84_d5b5_b547_0917,
        0x9216_d5d9_8979_fb1b,
        0xd131_0ba6_98df_b5ac,
        0x2ffd_72db_d01a_dfb7,
        0xb8e1_afed_6a26_7e96,
        0xba7c_9045_f12c_7f99,
        0x24a1_9947_b391_6cf7,
        0x0801_f2e2_858e_fc16,
        0x6369_20d8_7157_4e69,
    ];

    for (round_index, initial_seed) in seeds.into_iter().enumerate() {
        ROUND.store(round_index, Ordering::SeqCst);
        SEED.store(next_seed(initial_seed), Ordering::SeqCst);
        let scope = unsafe { zirk_rt_scope_enter() };

        let mut target_ids = [0; TARGETS];
        for i in 0..TARGETS {
            let id = spawn(TARGET_BODIES[i], scope);
            TARGET_IDS[i].store(id, Ordering::SeqCst);
            target_ids[i] = id;
        }

        let parked = spawn(target_barrier, scope);
        assert_eq!(unsafe { zirk_rt_job_wait(parked) }, TARGETS);
        assert!(
            target_ids
                .iter()
                .all(|id| !unsafe { zirk_rt_job_done(*id) })
        );

        let mut value_ids = [0; VALUES];
        for i in 0..VALUES {
            let id = spawn(VALUE_BODIES[i], scope);
            VALUE_IDS[i].store(id, Ordering::SeqCst);
            value_ids[i] = id;
        }
        let values_ready = spawn(value_barrier, scope);

        let mut cancel_ids = [0; TARGETS];
        for i in 0..TARGETS {
            cancel_ids[i] = spawn(CANCELLER_BODIES[i], scope);
        }
        for (i, id) in cancel_ids.into_iter().enumerate() {
            let observed_target = unsafe { zirk_rt_job_wait(id) };
            let seed = SEED.load(Ordering::SeqCst) as usize;
            let expected_target = (i * 3 + (seed & (TARGETS - 1))) & (TARGETS - 1);
            assert_eq!(observed_target, expected_target);
        }
        let canceled_ready = spawn(canceled_targets_barrier, scope);
        assert_eq!(unsafe { zirk_rt_job_wait(canceled_ready) }, TARGETS);
        assert_eq!(unsafe { zirk_rt_job_wait(values_ready) }, 1);
        assert_eq!(TARGET_BARRIER_STARTS[round_index].load(Ordering::SeqCst), 1);
        assert_eq!(
            TARGET_BARRIER_RETURNS[round_index].load(Ordering::SeqCst),
            1
        );
        assert_eq!(VALUE_BARRIER_STARTS[round_index].load(Ordering::SeqCst), 1);
        assert_eq!(VALUE_BARRIER_RETURNS[round_index].load(Ordering::SeqCst), 1);
        assert_eq!(CANCEL_BARRIER_STARTS[round_index].load(Ordering::SeqCst), 1);
        assert_eq!(
            CANCEL_BARRIER_RETURNS[round_index].load(Ordering::SeqCst),
            1
        );

        // Every target was parked before cancellation was admitted. Check that
        // all cancellation requests reached a terminal task before any waiter
        // consumes its result.
        for id in target_ids {
            assert!(
                unsafe { zirk_rt_job_done(id) },
                "canceled child did not terminate"
            );
        }
        for i in 0..TARGETS {
            let s = slot(i);
            assert_eq!(CANCEL_STARTS[s].load(Ordering::SeqCst), 1);
            assert_eq!(CANCEL_RETURNS[s].load(Ordering::SeqCst), 1);
            assert_eq!(CANCEL_CALLS[s].load(Ordering::SeqCst), 2);
        }

        // Model a selector over the public ABI's readiness primitive. All four
        // candidates are ready; the seeded starting offset determines which
        // one is chosen and its result is consumed exactly once.
        assert!(value_ids.iter().all(|id| unsafe { zirk_rt_job_done(*id) }));
        let start = (SEED.load(Ordering::SeqCst) as usize) & (VALUES - 1);
        let selected = (0..VALUES)
            .map(|offset| (start + offset) & (VALUES - 1))
            .find(|&i| unsafe { zirk_rt_job_done(value_ids[i]) })
            .expect("at least one result candidate must be ready");
        assert_eq!(
            unsafe { zirk_rt_job_wait(value_ids[selected]) },
            expected_value(selected),
            "selected task result must be delivered unchanged"
        );
        for i in 0..VALUES {
            let s = value_slot(i);
            assert_eq!(VALUE_STARTS[s].load(Ordering::SeqCst), 1);
            assert_eq!(VALUE_RETURNS[s].load(Ordering::SeqCst), 1);
        }

        let mut waiter_ids = [0; TARGETS];
        for i in 0..TARGETS {
            waiter_ids[i] = spawn(WAITER_BODIES[i], scope);
        }
        for id in waiter_ids {
            assert_eq!(unsafe { zirk_rt_job_wait(id) }, CANCELLED);
        }

        // Scope close is the final structured join. It must not return while
        // any registered child or waiter remains active.
        assert!(unsafe { zirk_rt_scope_exit(scope) });
        for i in 0..TARGETS {
            let s = slot(i);
            assert_eq!(TARGET_STARTS[s].load(Ordering::SeqCst), 1);
            assert_eq!(TARGET_RETURNS[s].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_STARTS[s].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_RETURNS[s].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_RESULTS[s].load(Ordering::SeqCst), CANCELLED);
        }
    }
}

#[test]
fn seeded_cancellation_join_and_result_selection_resolve_exactly_once() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(driver);
        zirk_rt_shutdown();
    }
}
