// Ignored release benchmark for the private cooperative mutex foundation.
// It measures contended executor-task acquisitions, not a language API.
use std::hint::black_box;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

const MUTEX_BENCH_SAMPLES: usize = 7;
const MUTEX_BENCH_TASKS: usize = 8;
const MUTEX_BENCH_ACQUISITIONS_PER_TASK: usize = 256;

#[test]
#[ignore = "opt-in release benchmark for runtime-only cooperative mutex contention"]
fn benchmark_cooperative_mutex_contention() {
    let mut durations = Vec::with_capacity(MUTEX_BENCH_SAMPLES);
    let mut checksum = 0usize;
    for _ in 0..MUTEX_BENCH_SAMPLES {
        let mutex = Arc::new(CooperativeMutex::new(0usize));
        let finished = Arc::new(AtomicUsize::new(0));
        let start = Instant::now();
        let result = run({
            let mutex = mutex.clone();
            let finished = finished.clone();
            move || {
                for _ in 0..MUTEX_BENCH_TASKS {
                    let task_mutex = mutex.clone();
                    let task_finished = finished.clone();
                    spawn(move || {
                        for _ in 0..MUTEX_BENCH_ACQUISITIONS_PER_TASK {
                            task_mutex
                                .with(|value| {
                                    *value += 1;
                                    crate::executor::yield_now();
                                })
                                .unwrap();
                        }
                        task_finished.fetch_add(1, Ordering::Release);
                        0
                    });
                }
                while finished.load(Ordering::Acquire) != MUTEX_BENCH_TASKS {
                    crate::executor::yield_now();
                }
                mutex.with(|value| *value).unwrap()
            }
        });
        assert_eq!(
            result,
            MUTEX_BENCH_TASKS * MUTEX_BENCH_ACQUISITIONS_PER_TASK
        );
        checksum = checksum.wrapping_add(black_box(result));
        durations.push(start.elapsed());
    }
    report_private_bench(
        "cooperative_mutex_contention_runtime_only",
        &durations,
        MUTEX_BENCH_TASKS * MUTEX_BENCH_ACQUISITIONS_PER_TASK,
        checksum as u64,
    );
}

fn report_private_bench(name: &str, durations: &[Duration], work: usize, checksum: u64) {
    let mut nanos = durations.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    println!(
        "CONTEXT benchmark={name} os={} arch={} profile=release rustc=[{}] samples={} tasks={} acquisitions_per_task={} allocator=system thread_ceiling=one_executor_thread rss_peak_bytes={} rss_metric=process_lifetime_peak",
        std::env::consts::OS,
        std::env::consts::ARCH,
        private_bench_rustc(),
        durations.len(),
        MUTEX_BENCH_TASKS,
        MUTEX_BENCH_ACQUISITIONS_PER_TASK,
        private_bench_peak_rss_bytes()
            .map_or_else(|| "unavailable".into(), |bytes| bytes.to_string()),
    );
    println!(
        "BENCH name={name} work_per_sample={work} min_ns={} median_ns={} max_ns={} checksum={checksum}",
        nanos[0],
        nanos[nanos.len() / 2],
        nanos[nanos.len() - 1],
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn private_bench_peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied structure on success.
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
fn private_bench_peak_rss_bytes() -> Option<u64> {
    None
}

fn private_bench_rustc() -> String {
    std::process::Command::new("rustc")
        .arg("-V")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into())
}
