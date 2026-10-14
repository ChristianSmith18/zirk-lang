// Ignored release benchmark for the private bounded blocking-pool foundation.
// Measures worker creation, queue admission, drain, and join as one lifecycle.
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn process_peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied rusage structure on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let peak = unsafe { usage.assume_init().ru_maxrss };
    #[cfg(target_os = "macos")]
    let bytes = u64::try_from(peak).ok()?;
    #[cfg(target_os = "linux")]
    let bytes = u64::try_from(peak).ok()?.saturating_mul(1024);
    Some(bytes)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn process_peak_rss_bytes() -> Option<u64> {
    None
}

const POOL_BENCH_SAMPLES: usize = 7;
const POOL_BENCH_WORKERS: usize = 8;
const POOL_BENCH_JOBS: usize = 1024;
const THREAD_RUN_BENCH_SAMPLES: usize = 7;
const THREAD_RUN_BENCH_CALLS: usize = 1024;

unsafe extern "C-unwind" fn benchmark_managed_callback(
    _target: *mut std::ffi::c_void,
    _captures: *mut std::ffi::c_void,
    _result: *mut std::ffi::c_void,
    exception: *mut std::ffi::c_void,
) -> u32 {
    unsafe { *exception.cast::<*mut std::ffi::c_void>() = std::ptr::null_mut() };
    MANAGED_CALL_OK
}

#[test]
#[ignore = "opt-in release benchmark for runtime-only managed blocking pool admission"]
fn benchmark_managed_blocking_pool_admission_and_shutdown() {
    let _serial = lock_pool_tests();
    let mut durations = Vec::with_capacity(POOL_BENCH_SAMPLES);
    let mut checksum = 0usize;
    for _ in 0..POOL_BENCH_SAMPLES {
        let start = Instant::now();
        let mut pool = ManagedBlockingPool::new(POOL_BENCH_WORKERS, POOL_BENCH_JOBS).unwrap();
        let completed = Arc::new(AtomicUsize::new(0));
        for _ in 0..POOL_BENCH_JOBS {
            let completed = Arc::clone(&completed);
            pool.try_submit(move || {
                completed.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        }
        let report = pool.shutdown();
        let observed = completed.load(Ordering::Relaxed);
        assert_eq!(report.completed, POOL_BENCH_JOBS as u64);
        assert_eq!(report.panicked, 0);
        assert_eq!(observed, POOL_BENCH_JOBS);
        checksum = checksum.wrapping_add(black_box(observed));
        durations.push(start.elapsed());
    }
    report_pool_bench(
        "managed_blocking_admission_drain_shutdown_runtime_only",
        &durations,
        POOL_BENCH_JOBS,
        checksum as u64,
    );
}

#[test]
#[ignore = "opt-in release benchmark for source-level Thread.run admission and joined execution"]
fn benchmark_managed_thread_run_admission() {
    let _serial = lock_pool_tests();
    let worker_limit = with_managed_pool(ManagedBlockingPool::worker_count);
    let mut durations = Vec::with_capacity(THREAD_RUN_BENCH_SAMPLES);
    let mut checksum = 0u64;

    for sample in 0..THREAD_RUN_BENCH_SAMPLES {
        let start = Instant::now();
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            let target = black_box(sample);
            let mut exception: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut completed = 0u64;
            for _ in 0..THREAD_RUN_BENCH_CALLS {
                // SAFETY: this callback does not dereference the target and
                // writes only the live exception slot on the executor stack.
                let status = unsafe {
                    run_managed_callback(
                        benchmark_managed_callback,
                        (&target as *const usize).cast_mut().cast(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        (&mut exception as *mut *mut std::ffi::c_void).cast(),
                    )
                };
                assert_eq!(status, MANAGED_CALL_OK);
                assert!(exception.is_null());
                completed += 1;
            }
            black_box(completed as usize)
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(value) if value == THREAD_RUN_BENCH_CALLS));
        durations.push(start.elapsed());
        checksum = checksum.wrapping_add(THREAD_RUN_BENCH_CALLS as u64);
    }

    let mut nanos = durations.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    println!(
        "CONTEXT benchmark=managed_thread_run_admission os={} arch={} profile=release rustc=[{}] samples={} workers={} queue_capacity={} worker_limit={} thread_ceiling={} rss_peak_bytes={} rss_metric=process_lifetime_peak",
        std::env::consts::OS,
        std::env::consts::ARCH,
        pool_bench_rustc(),
        durations.len(),
        worker_limit,
        MAX_QUEUE_CAPACITY,
        MAX_WORKERS,
        worker_limit + 1,
        process_peak_rss_bytes()
            .map_or_else(|| "unavailable".into(), |bytes| bytes.to_string()),
    );
    println!(
        "BENCH name=managed_thread_run_admission calls_per_sample={} min_ns={} median_ns={} max_ns={} checksum={checksum}",
        THREAD_RUN_BENCH_CALLS,
        nanos[0],
        nanos[nanos.len() / 2],
        nanos[nanos.len() - 1],
    );
    shutdown_managed_pool();
}

fn report_pool_bench(name: &str, durations: &[Duration], work: usize, checksum: u64) {
    let mut nanos = durations.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    println!(
        "CONTEXT benchmark={name} os={} arch={} profile=release rustc=[{}] samples={} workers={} queue_capacity={} worker_limit={} allocator=system rss_peak_bytes={} rss_metric=process_lifetime_peak thread_ceiling={} (including benchmark caller)",
        std::env::consts::OS,
        std::env::consts::ARCH,
        pool_bench_rustc(),
        durations.len(),
        POOL_BENCH_WORKERS,
        POOL_BENCH_JOBS,
        MAX_WORKERS,
        process_peak_rss_bytes()
            .map_or_else(|| "unavailable".into(), |bytes| bytes.to_string()),
        POOL_BENCH_WORKERS + 1,
    );
    println!(
        "BENCH name={name} jobs_per_sample={work} min_ns={} median_ns={} max_ns={} checksum={checksum}",
        nanos[0],
        nanos[nanos.len() / 2],
        nanos[nanos.len() - 1],
    );
}

fn pool_bench_rustc() -> String {
    std::process::Command::new("rustc")
        .arg("-V")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into())
}
