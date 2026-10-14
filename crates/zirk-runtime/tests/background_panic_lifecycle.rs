//! Public-API integration coverage for repeated background panic reporting.
//!
//! This verifies that each fresh executor delivers a failed job diagnostic
//! before its root returns, and that `zirk_rt_run_main` returns after the
//! background drain. The public API does not expose task, timer, or GC-root
//! counts, so this test deliberately makes no baseline/leak assertion for them.

use std::sync::atomic::{AtomicUsize, Ordering};

use zirk_runtime::{
    BackgroundDiagnostic, background_diagnostics_current, spawn_background, zirk_rt_run_main,
    zirk_rt_sleep,
};

static CYCLE: AtomicUsize = AtomicUsize::new(0);
static COOPERATIVE_STARTED: [AtomicUsize; 32] = [const { AtomicUsize::new(0) }; 32];
static COOPERATIVE_CLEANED: [AtomicUsize; 32] = [const { AtomicUsize::new(0) }; 32];

extern "C-unwind" fn root() {
    let cycle = CYCLE.load(Ordering::SeqCst);
    let expected = format!("background panic cycle {cycle}");
    let panic_message = expected.clone();
    spawn_background(move || -> usize { panic!("{panic_message}") })
        .expect("background admission should succeed in a fresh executor");
    let cooperative_cycle = cycle;
    spawn_background(move || -> usize {
        COOPERATIVE_STARTED[cooperative_cycle].fetch_add(1, Ordering::SeqCst);
        // Root completion cancels this long wait. Reaching the following line
        // is the cleanup acknowledgement observed by the parent test.
        unsafe { zirk_rt_sleep(86_400_000_000_000) };
        COOPERATIVE_CLEANED[cooperative_cycle].fetch_add(1, Ordering::SeqCst);
        0
    })
    .expect("cooperative background admission should succeed in a fresh executor");

    // The public timer ABI suspends this task and lets the queued child run;
    // its panic is recorded before the timer resumes this root task.
    unsafe { zirk_rt_sleep(1_000_000) };
    let diagnostics = background_diagnostics_current();
    assert!(matches!(
        diagnostics.as_slice(),
        [BackgroundDiagnostic::JobFailed(failure)] if failure.message == expected
    ));
}

#[test]
fn repeated_executor_runs_report_panics_and_finish_background_drain() {
    const CYCLES: usize = 32;

    for cycle in 0..CYCLES {
        CYCLE.store(cycle, Ordering::SeqCst);
        // SAFETY: `root` is a no-argument runtime callback and every
        // background closure completes by panicking, which the executor
        // captures and reports. The call returns only after executor cleanup.
        unsafe { zirk_rt_run_main(root) };
        assert_eq!(COOPERATIVE_STARTED[cycle].load(Ordering::SeqCst), 1);
        assert_eq!(
            COOPERATIVE_CLEANED[cycle].load(Ordering::SeqCst),
            1,
            "root completion returned before cooperative cancellation and drain"
        );
    }
}
