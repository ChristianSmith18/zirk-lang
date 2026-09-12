//! Worker pool for `parallel` CPU regions (roadmap Phase 5,
//! `parallel-cpu-regions`, task group 2;
//! `docs/decisions/ADR-019-parallel-and-multithreaded-gc.md`).
//!
//! # Two schedulers (ADR-019 D1)
//!
//! The cooperative executor (`crate::executor`, ADR-017) stays single-threaded
//! and keeps owning every I/O branch. This module is the *second* scheduler: a
//! fixed pool of OS worker threads that runs the work a `parallel` region
//! splits off. The branch that enters a region ([`run_region`]) submits the
//! region's chunks and then blocks **cooperatively** — the executor keeps
//! servicing other branches ([`crate::executor::block_current_on_region`]) —
//! until every chunk has joined.
//!
//! # Garbage collection
//!
//! Worker threads allocate onto the one shared heap (`crate::collector`). A
//! collection that becomes due mid-region cannot run inline: it raises the
//! stop-the-world flag, every worker parks at its next safepoint
//! ([`safepoint_poll_rust`], called after each chunk and — via
//! `zirk_rt_safepoint_poll` — at every `parallel` loop back-edge), and the
//! executor thread coordinates one collection over every thread's roots
//! (`crate::collector::safepoint_poll`).
//!
//! # Rooting contract
//!
//! A chunk closure that captures collector-managed references MUST root them
//! with `zirk_rt_push_frame` / `zirk_rt_pop_frame` for as long as it holds
//! them — codegen emits this (`parallel-cpu-regions` task 6). The push/pop
//! routes to the worker thread's own shadow-stack chain, which the coordinator
//! walks. The source collection a region iterates stays rooted in the region
//! branch's frame for the region's whole duration.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// How a `parallel; cores: …` header resolves against the detected core count
/// (ADR-019 D3, `specs/parallel-regions` "The `cores` option").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoreBudget {
    /// `cores: N`, `N > 0` — exactly `N` worker threads.
    Exact(usize),
    /// `cores: -N` — the detected core count minus `N`.
    AllMinus(usize),
    /// `cores: A..=B` — the runtime picks a count in `[A, B]`.
    Range(usize, usize),
    /// No `cores` option — all available cores.
    All,
}

/// A `parallel` region could not start because its `cores` option does not
/// resolve to a positive worker count on this machine (ADR-019 D3;
/// `specs/parallel-regions` "not enough cores").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegionError {
    pub(crate) message: String,
}

/// The detected hardware concurrency, never less than 1.
pub(crate) fn detected_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

/// The configured worker-pool size. Defaults to hardware concurrency
/// (`parallel-cpu-regions` task 3.5); `ZIRK_PARALLEL_CORES` overrides it, and
/// `ZIRK_PARALLEL_CORES=1` is the documented single-threaded fallback that
/// keeps every `parallel` region running inline on the executor thread —
/// behaviourally identical to pre-`parallel` execution.
pub(crate) fn configured_pool_size() -> usize {
    static SIZE: OnceLock<usize> = OnceLock::new();
    *SIZE.get_or_init(|| {
        std::env::var("ZIRK_PARALLEL_CORES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or_else(detected_cores)
    })
}

/// Regions currently running on the worker pool, process-wide. Read by the
/// collector from *any* thread (a worker deciding whether a threshold crossing
/// must defer to a safepoint), so it cannot be the thread-local
/// [`ACTIVE_REGION_BUDGET`].
static ACTIVE_REGIONS: AtomicUsize = AtomicUsize::new(0);

/// Whether a multi-threaded `parallel` region is running right now — the
/// condition under which a garbage collection must go through the
/// stop-the-world safepoint rather than run inline (`ADR-019` D4).
pub(crate) fn pool_is_multithreaded() -> bool {
    configured_pool_size() > 1 && ACTIVE_REGIONS.load(Ordering::SeqCst) > 0
}

/// Resolves a [`CoreBudget`] to a concrete worker count against `detected`
/// (usually [`detected_cores`]).
pub(crate) fn resolve_budget(budget: CoreBudget, detected: usize) -> Result<usize, RegionError> {
    let detected = detected.max(1);
    let resolved = match budget {
        CoreBudget::Exact(n) => n,
        CoreBudget::AllMinus(n) => {
            if n >= detected {
                return Err(RegionError {
                    message: format!(
                        "parallel region asked for `cores: -{n}` but only {detected} core(s) were detected"
                    ),
                });
            }
            detected - n
        }
        CoreBudget::Range(a, b) => {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            hi.min(detected).max(lo.min(detected)).max(1)
        }
        CoreBudget::All => detected,
    };
    if resolved == 0 {
        return Err(RegionError {
            message: "parallel region resolved to a zero-core budget".to_string(),
        });
    }
    Ok(resolved)
}

thread_local! {
    /// The worker budget of the enclosing `parallel` region on *this* thread,
    /// if any. A nested region clamps its own budget to this (ADR-019 "Risks"
    /// — nested regions share the outer pool).
    static ACTIVE_REGION_BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// The budget the innermost active `parallel` region on this thread is running
/// with, or `None` outside every region.
pub(crate) fn active_region_budget() -> Option<usize> {
    ACTIVE_REGION_BUDGET.with(std::cell::Cell::get)
}

// --- the pool ------------------------------------------------------------

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Pool {
    queue: Arc<(Mutex<Vec<Job>>, Condvar)>,
}

impl Pool {
    fn submit(&self, job: Job) {
        let (lock, cv) = &*self.queue;
        lock.lock().unwrap().push(job);
        cv.notify_one();
    }
}

static POOL: OnceLock<Pool> = OnceLock::new();

fn pool() -> &'static Pool {
    POOL.get_or_init(|| {
        let queue: Arc<(Mutex<Vec<Job>>, Condvar)> =
            Arc::new((Mutex::new(Vec::new()), Condvar::new()));
        let workers = configured_pool_size();
        for _ in 0..workers {
            let queue = Arc::clone(&queue);
            std::thread::Builder::new()
                .name("zirk-parallel-worker".to_string())
                .spawn(move || worker_loop(queue))
                .expect("spawning a parallel worker thread");
        }
        Pool { queue }
    })
}

fn worker_loop(queue: Arc<(Mutex<Vec<Job>>, Condvar)>) {
    // Publish this thread's shadow-stack chain so a stop-the-world collection
    // can walk the roots of whatever chunk it is running (`ADR-019` D4).
    crate::collector::register_worker_chain();

    let (lock, cv) = &*queue;
    loop {
        let job = {
            let mut jobs = lock.lock().unwrap();
            loop {
                if let Some(job) = jobs.pop() {
                    break job;
                }
                jobs = cv.wait(jobs).unwrap();
            }
        };
        // The job itself owns `worker_active_enter` / the post-chunk safepoint /
        // `worker_active_leave` / the region's completion decrement, in that
        // order (see `run_region`). A chunk panic must not poison the worker or
        // the queue, but the job's own bookkeeping still runs — it is inside a
        // further `catch_unwind` in `run_region`'s job wrapper.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
    }
}

/// Runs `chunks` as one `parallel` region with the resolved `budget`, returning
/// each chunk's result in submission order.
///
/// - Single-threaded pool, a one-chunk region, or a nested region reached from
///   a worker thread → runs inline on the calling thread, in order, with a
///   safepoint between chunks.
/// - Otherwise → submits the chunks to the worker pool and blocks the calling
///   branch cooperatively until they join.
pub(crate) fn run_region<T, F>(budget: CoreBudget, chunks: Vec<F>) -> Result<Vec<T>, RegionError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let mut workers = resolve_budget(budget, detected_cores())?;
    workers = workers.min(configured_pool_size());
    if let Some(outer) = active_region_budget() {
        workers = workers.min(outer).max(1);
    }

    let previous = ACTIVE_REGION_BUDGET.replace(Some(workers));
    struct RestoreBudget(Option<usize>);
    impl Drop for RestoreBudget {
        fn drop(&mut self) {
            ACTIVE_REGION_BUDGET.with(|c| c.set(self.0));
        }
    }
    let _restore_budget = RestoreBudget(previous);

    let n = chunks.len();
    let on_executor_task = crate::executor::in_task_context();

    if workers <= 1 || n <= 1 || configured_pool_size() <= 1 || !on_executor_task {
        let mut out = Vec::with_capacity(n);
        for chunk in chunks {
            out.push(chunk());
            safepoint_poll_rust();
        }
        return Ok(out);
    }

    // Multi-threaded path.
    let results: Arc<Vec<Mutex<Option<T>>>> = Arc::new((0..n).map(|_| Mutex::new(None)).collect());
    let remaining = Arc::new(AtomicUsize::new(n));

    ACTIVE_REGIONS.fetch_add(1, Ordering::SeqCst);
    struct RestoreRegionCount;
    impl Drop for RestoreRegionCount {
        fn drop(&mut self) {
            ACTIVE_REGIONS.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let _restore_region_count = RestoreRegionCount;

    for (index, chunk) in chunks.into_iter().enumerate() {
        let results = Arc::clone(&results);
        let remaining = Arc::clone(&remaining);
        pool().submit(Box::new(move || {
            // This worker joins the executor's world for the chunk: its
            // allocations go to the published heap and it takes the
            // stop-the-world path at a safepoint.
            let world = crate::collector::enter_executor_world();
            crate::collector::worker_active_enter();
            let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(chunk)).ok();
            if let Some(value) = value {
                *results[index].lock().unwrap() = Some(value);
            }
            // Post-chunk safepoint, then leave the active set and the world —
            // all heap access from this job must be finished *before* the
            // completion decrement, because that decrement is what unblocks
            // `run_region` and, at the end of the run, frees the heap.
            crate::collector::safepoint_poll();
            crate::collector::worker_active_leave();
            drop(world);
            if remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
                crate::executor::notify_pool_event();
            }
        }));
    }

    crate::executor::block_current_on_region(Arc::clone(&remaining));

    // Resumed: every chunk has written its result and decremented the counter.
    // A worker's job closure may not be fully dropped yet (it decrements, then
    // returns), so it can still hold an `Arc` clone of `results` — read each
    // slot out through the shared reference instead of trying to unwrap.
    (0..n)
        .map(|index| {
            results[index]
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| RegionError {
                    message: "a parallel chunk panicked without producing a result".to_string(),
                })
        })
        .collect()
}

/// The Rust-visible safepoint poll: a worker thread or the executor checks the
/// stop-the-world flag here and cooperates if a collection has been requested
/// (`ADR-019` D4).
#[inline]
pub(crate) fn safepoint_poll_rust() {
    crate::collector::safepoint_poll();
}

/// C-ABI safepoint poll emitted by codegen at every loop back-edge inside a
/// `parallel` region (`specs/zirk-native-codegen` "safepoint poll at a region
/// back-edge").
///
/// # Safety
///
/// Callable from generated code at any point inside a `parallel` region body.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_safepoint_poll() {
    safepoint_poll_rust();
}

/// C-ABI region entry: validates the `cores` budget at region entry and marks
/// the region active. `budget_kind` / `a` / `b` encode a [`CoreBudget`]:
/// `0`=All, `1`=Exact(a), `2`=AllMinus(a), `3`=Range(a,b). Returns the resolved
/// worker count, or aborts with a runtime error when the budget does not
/// resolve (ADR-019 D3).
///
/// The chunk split and join are driven from Rust ([`run_region`]); codegen
/// (`parallel-cpu-regions` task 6) calls this to fix the budget and emits the
/// region body as chunk closures passed to `run_region`.
///
/// # Safety
///
/// Callable from generated code at the top of a `parallel` region.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_pool_submit(budget_kind: u8, a: i64, b: i64) -> u64 {
    let budget = match budget_kind {
        1 => CoreBudget::Exact(a.max(0) as usize),
        2 => CoreBudget::AllMinus(a.max(0) as usize),
        3 => CoreBudget::Range(a.max(0) as usize, b.max(0) as usize),
        _ => CoreBudget::All,
    };
    match resolve_budget(budget, detected_cores()) {
        Ok(mut workers) => {
            workers = workers.min(configured_pool_size());
            if let Some(outer) = active_region_budget() {
                workers = workers.min(outer).max(1);
            }
            ACTIVE_REGION_BUDGET.with(|c| c.set(Some(workers)));
            workers as u64
        }
        Err(e) => crate::failure::fatal(&e.message),
    }
}

/// C-ABI region exit: clears the active-region marker for this thread.
///
/// # Safety
///
/// Callable from generated code at the end of a `parallel` region, paired with
/// one [`zirk_rt_pool_submit`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_pool_join() {
    ACTIVE_REGION_BUDGET.with(|c| c.set(None));
}

/// C-ABI parallel `for`: runs `body(capture, i)` for `i` in `0..count`,
/// discarding each call's result — the source loop is used for effect
/// (`specs/parallel-regions` "parallel for spreads iterations"). `kind`/`a`/`b`
/// encode the region's resolved core budget: `0` = all detected cores, `1` = a
/// signed count (`a > 0` exactly `a` cores; `a < 0` detected minus `-a`; `a ==
/// 0` is a runtime error, same as a literal `cores: 0`), `2` = a range
/// `[a, b]`. This tag scheme is deliberately distinct from
/// [`zirk_rt_pool_submit`]'s (which splits "exact" and "all-minus" into
/// separate tags): here the sign of one *runtime* `cores` value decides
/// between them, so codegen never needs to know it statically — it lowers
/// `cores`'s own expression to a plain `i64` (or a `Range`'s two bounds) and
/// passes the tag straight through (`parallel-cpu-regions` task 6.1).
///
/// # Safety
///
/// Callable from generated code for a `parallel { for x in coll { ... } }`
/// region whose collection is an `Array`/`List`. `body` must be a valid
/// function pointer taking the compiler-emitted capture-block pointer (or
/// null) and an index in `0..count`, and must not retain either past its own
/// return — each index runs its own chunk closure exactly once, and the
/// capture block is not rooted once every chunk has returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_parallel_for(
    kind: u8,
    a: i64,
    b: i64,
    count: i64,
    body: extern "C-unwind" fn(*mut std::ffi::c_void, i64) -> usize,
    capture: *mut std::ffi::c_void,
) {
    let budget = match kind {
        1 if a < 0 => CoreBudget::AllMinus((-a) as usize),
        1 => CoreBudget::Exact(a.max(0) as usize),
        2 => CoreBudget::Range(a.max(0) as usize, b.max(0) as usize),
        _ => CoreBudget::All,
    };

    struct CaptureArg(*mut std::ffi::c_void);
    // SAFETY: mirrors `zirk_rt_spawn`'s `TaskArg` — each index below runs
    // its own chunk closure exactly once, so the capture pointer is never
    // touched by two chunks at the same time.
    unsafe impl Send for CaptureArg {}
    let capture = CaptureArg(capture);

    let n = count.max(0) as usize;
    let chunks: Vec<_> = (0..n)
        .map(|i| {
            let capture = CaptureArg(capture.0);
            move || {
                let capture = capture;
                body(capture.0, i as i64);
            }
        })
        .collect();

    if let Err(e) = run_region(budget, chunks) {
        crate::failure::fatal(&e.message);
    }
}

/// A parallel `map`: applies `f` to each element, preserving input order
/// (`specs/parallel-regions` "ordered pipeline preserves input order").
pub(crate) fn parallel_map<T, R, F>(
    budget: CoreBudget,
    input: Vec<T>,
    f: F,
) -> Result<Vec<R>, RegionError>
where
    F: Fn(T) -> R + Sync + Send + Copy + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    let chunks: Vec<_> = input.into_iter().map(|x| move || f(x)).collect();
    run_region(budget, chunks)
}

/// A parallel `reduce` with an associative `combine` and an `identity`
/// (`specs/parallel-regions` "A parallel reduction SHALL require an associative
/// combiner"). Associativity is the caller's obligation — the checker enforces
/// it at compile time (`specs/zirk-type-system`).
pub(crate) fn parallel_reduce<T, F>(
    budget: CoreBudget,
    input: Vec<T>,
    identity: T,
    combine: F,
) -> Result<T, RegionError>
where
    T: Send + Clone + 'static,
    F: Fn(T, T) -> T,
{
    let items = parallel_map(budget, input, |x| x)?;
    Ok(items.into_iter().fold(identity, &combine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_budget_resolves_to_itself() {
        assert_eq!(resolve_budget(CoreBudget::Exact(4), 8), Ok(4));
    }

    #[test]
    fn all_minus_leaves_cores_free() {
        assert_eq!(resolve_budget(CoreBudget::AllMinus(1), 8), Ok(7));
    }

    #[test]
    fn all_minus_too_many_is_an_error() {
        let err = resolve_budget(CoreBudget::AllMinus(2), 2).unwrap_err();
        assert!(err.message.contains("2 core(s)"), "{}", err.message);
    }

    #[test]
    fn range_is_clamped_into_detected() {
        assert_eq!(resolve_budget(CoreBudget::Range(2, 4), 8), Ok(4));
        assert_eq!(resolve_budget(CoreBudget::Range(2, 16), 8), Ok(8));
    }

    #[test]
    fn absent_budget_is_all_detected() {
        assert_eq!(resolve_budget(CoreBudget::All, 6), Ok(6));
    }

    #[test]
    fn parallel_map_preserves_order_inline() {
        // Outside a task context this runs inline; order must still hold.
        let out = parallel_map(CoreBudget::All, vec![1, 2, 3, 4], |x| x * 10).unwrap();
        assert_eq!(out, vec![10, 20, 30, 40]);
    }

    #[test]
    fn parallel_reduce_matches_sequential_for_an_associative_combiner() {
        let sum = parallel_reduce(CoreBudget::All, vec![1, 2, 3, 4, 5], 0, |a, b| a + b).unwrap();
        assert_eq!(sum, 15);
    }

    #[test]
    fn run_region_on_the_worker_pool_preserves_order_and_result() {
        // Inside a real executor task, a multi-chunk region goes to the pool.
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let chunks: Vec<_> = (0..64u64).map(|i| move || i * i).collect();
            let out = run_region(CoreBudget::All, chunks).unwrap();
            for (i, v) in out.iter().enumerate() {
                assert_eq!(*v, (i as u64) * (i as u64));
            }
            out.len()
        });
        match outcome {
            crate::task::TaskOutcome::Value(n) => assert_eq!(n, 64),
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }
    }

    /// `parallel-cpu-regions` task 3.4 — the stop-the-world safepoint under
    /// load. An allocation-heavy `parallel` reduce with the GC threshold
    /// forced very low so a collection fires again and again *during* the
    /// region, on and across worker threads. The result must be deterministic,
    /// no live object may be reclaimed (each chunk checks its own rooted
    /// objects survive), and nothing may crash or hang.
    ///
    /// Runs on this build's target triple; the other three triples in the task
    /// are covered by `cargo test --workspace` in CI.
    #[test]
    fn stop_the_world_safepoint_under_an_allocation_heavy_parallel_reduce() {
        use crate::collector::test_support;
        use std::ffi::c_void;

        let _guard = test_support::test_guard();
        test_support::reset_state_for_tests();

        const CHUNKS: u64 = 48;
        const PER_CHUNK: usize = 500;
        let header = crate::collector::HEADER_BYTES;

        let outcome = crate::executor::Executor::new().run_with_root(move || {
            // Force a collection roughly every few allocations — on and across
            // worker threads — for the whole region. Set on *this run's*
            // published heap, from inside the run.
            test_support::set_threshold_for_tests(2 * 1024);
            let descriptor = test_support::descriptor_with_fields_for_tests(&[]);
            let descriptor_addr = descriptor.as_ptr() as usize;

            let chunks: Vec<_> = (0..CHUNKS)
                .map(|_| {
                    move || {
                        let descriptor = descriptor_addr as *mut c_void;
                        // Slots that hold every object this chunk allocates,
                        // rooted through one pushed frame for the chunk's
                        // whole life — a collection any other worker triggers
                        // must not reclaim them.
                        let mut alive: Vec<*mut c_void> = vec![std::ptr::null_mut(); PER_CHUNK];
                        let mut roots: Vec<*mut c_void> = (0..PER_CHUNK)
                            .map(|i| unsafe { alive.as_mut_ptr().add(i) as *mut c_void })
                            .collect();
                        unsafe {
                            crate::collector::zirk_rt_push_frame(
                                roots.as_mut_ptr(),
                                PER_CHUNK as i64,
                            )
                        };
                        for slot in alive.iter_mut() {
                            // `zirk_rt_alloc` runs `maybe_collect` first — that
                            // is what raises the stop-the-world flag here.
                            let obj = unsafe { crate::memory::zirk_rt_alloc(header + 8, 8) };
                            unsafe { *(obj as *mut *mut c_void) = descriptor };
                            *slot = obj;
                            crate::pool::safepoint_poll_rust();
                        }
                        // Every object must still carry its descriptor — a
                        // reclaimed-and-reused slot would have clobbered it.
                        let ok = alive
                            .iter()
                            .filter(|&&obj| unsafe {
                                crate::collector::object_descriptor(obj) == descriptor
                            })
                            .count() as u64;
                        crate::collector::zirk_rt_pop_frame();
                        ok
                    }
                })
                .collect();

            let partials = run_region(CoreBudget::All, chunks).unwrap();
            partials.iter().sum::<u64>() as usize
        });

        match outcome {
            crate::task::TaskOutcome::Value(total) => assert_eq!(
                total as u64,
                CHUNKS * PER_CHUNK as u64,
                "a live object was reclaimed during a parallel region"
            ),
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }

        // Everything is unrooted now — a final collection must reclaim it all
        // and leave the heap consistent.
        test_support::reset_state_for_tests();
    }

    /// The exact body shape `zirk_rt_parallel_for` requires: pushes its own
    /// index onto the `Mutex<Vec<i64>>` the capture pointer names.
    extern "C-unwind" fn record_index(capture: *mut std::ffi::c_void, index: i64) -> usize {
        let store = unsafe { &*(capture as *const std::sync::Mutex<Vec<i64>>) };
        store.lock().unwrap().push(index);
        0
    }

    #[test]
    fn zirk_rt_parallel_for_runs_every_index_exactly_once() {
        let store = std::sync::Mutex::new(Vec::new());
        let capture = &store as *const std::sync::Mutex<Vec<i64>> as usize;

        let outcome = crate::executor::Executor::new().run_with_root(move || {
            unsafe {
                zirk_rt_parallel_for(0, 0, 0, 64, record_index, capture as *mut std::ffi::c_void)
            };
            0
        });
        match outcome {
            crate::task::TaskOutcome::Value(_) => {}
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }

        let mut seen = store.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..64i64).collect::<Vec<_>>());
    }

    #[test]
    fn zirk_rt_parallel_for_zero_count_runs_nothing() {
        let store = std::sync::Mutex::new(Vec::new());
        let capture = &store as *const std::sync::Mutex<Vec<i64>> as usize;

        let outcome = crate::executor::Executor::new().run_with_root(move || {
            unsafe {
                zirk_rt_parallel_for(0, 0, 0, 0, record_index, capture as *mut std::ffi::c_void)
            };
            0
        });
        match outcome {
            crate::task::TaskOutcome::Value(_) => {}
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }
        assert!(store.lock().unwrap().is_empty());
    }

    #[test]
    fn zirk_rt_parallel_for_exact_signed_cores_runs_to_completion() {
        // `kind=1, a=1` ("exactly 1 core", the positive branch of the signed
        // encoding) resolves on any machine — unlike a negative `a`, which
        // this test avoids: `resolve_budget`'s own unit tests above already
        // cover the all-minus arithmetic directly, and calling all the way
        // through `zirk_rt_parallel_for` with a value that resolves to zero
        // cores on a 1-core CI runner would abort the whole test process
        // (`crate::failure::fatal` exits, it does not panic).
        let store = std::sync::Mutex::new(Vec::new());
        let capture = &store as *const std::sync::Mutex<Vec<i64>> as usize;
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            unsafe {
                zirk_rt_parallel_for(1, 1, 0, 32, record_index, capture as *mut std::ffi::c_void)
            };
            0
        });
        match outcome {
            crate::task::TaskOutcome::Value(_) => {}
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }
        let mut seen = store.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..32i64).collect::<Vec<_>>());
    }

    #[test]
    fn io_branches_run_while_a_region_is_on_the_pool() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SIBLING_TICKS: AtomicUsize = AtomicUsize::new(0);
        SIBLING_TICKS.store(0, Ordering::SeqCst);

        crate::executor::Executor::new().run_with_root(|| {
            crate::executor::spawn(|| {
                for _ in 0..5 {
                    SIBLING_TICKS.fetch_add(1, Ordering::SeqCst);
                    crate::executor::yield_now();
                }
                0
            });
            let chunks: Vec<_> = (0..32u64)
                .map(|i| {
                    move || {
                        // A little work so the region does not finish instantly.
                        (0..2000u64).fold(i, |a, b| a.wrapping_add(b))
                    }
                })
                .collect();
            let _ = run_region(CoreBudget::All, chunks).unwrap();
            0
        });
        assert!(
            SIBLING_TICKS.load(Ordering::SeqCst) >= 1,
            "the sibling I/O branch made no progress during the parallel region"
        );
    }
}
