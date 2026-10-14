//! Opt-in concurrency microbenchmarks for delivered runtime surfaces.
//!
//! Run with: `cargo test -p zirk-runtime --test concurrency_bench --release -- --ignored --nocapture`
//! Timings are observations only; assertions cover deterministic work/checksums.
//! `combinators` remain crate-private, so scheduler spawn/join is measured via
//! the public runtime ABI and is not presented as a combinator benchmark.

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use zirk_runtime::{
    AtomicCell, AtomicKind, ServiceState, ServiceSupervisor, zirk_rt_branch_register,
    zirk_rt_job_wait, zirk_rt_run_main, zirk_rt_scope_enter, zirk_rt_scope_exit, zirk_rt_spawn,
};

mod common;

const SAMPLES: usize = 7;
const TASKS_PER_SAMPLE: usize = 256;
const ATOMIC_OPS: usize = 1_000_000;
const POOL_ITEMS: usize = 2048;

fn rustc_context() -> String {
    std::process::Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_else(|| "unavailable".into())
}

fn context(name: &str) {
    let detected_workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let pool_override = std::env::var("ZIRK_PARALLEL_CORES").unwrap_or_else(|_| "unset".into());
    println!(
        "CONTEXT benchmark={name} os={} arch={} target={} profile=release rustc=[{}] samples={SAMPLES} allocator=system detected_workers={detected_workers} thread_ceiling={} pool_override={pool_override}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        option_env!("TARGET").unwrap_or("host-native"),
        rustc_context(),
        detected_workers + 1,
    );
}

fn report(name: &str, durations: &[Duration], work: usize, checksum: u64) {
    let mut ns = durations.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    ns.sort_unstable();
    let min = ns[0];
    let median = ns[ns.len() / 2];
    let max = ns[ns.len() - 1];
    println!(
        "BENCH name={name} samples={} work_per_sample={work} min_ns={min} median_ns={median} max_ns={max} checksum={checksum}",
        ns.len(),
    );
    common::report_process_peak_rss(name);
}

static TASK_ARGUMENT: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn task_body(_: *mut std::ffi::c_void) -> usize {
    TASK_ARGUMENT.fetch_add(1, Ordering::Relaxed) as usize
}

extern "C-unwind" fn scheduler_driver() {
    let scope = unsafe { zirk_rt_scope_enter() };
    let mut handles = Vec::with_capacity(TASKS_PER_SAMPLE);
    for _ in 0..TASKS_PER_SAMPLE {
        let handle = unsafe { zirk_rt_spawn(task_body, std::ptr::null_mut()) };
        unsafe { zirk_rt_branch_register(scope, handle) };
        handles.push(handle);
    }
    let checksum = handles
        .into_iter()
        .map(|handle| unsafe { zirk_rt_job_wait(handle) as u64 })
        .fold(0_u64, u64::wrapping_add);
    DRIVER_CHECKSUM.store(checksum, Ordering::SeqCst);
    assert!(unsafe { zirk_rt_scope_exit(scope) });
}

static DRIVER_CHECKSUM: AtomicU64 = AtomicU64::new(0);

#[test]
#[ignore = "opt-in release concurrency benchmark"]
fn benchmark_executor_spawn_join_overhead() {
    context("executor_spawn_join");
    let mut durations = Vec::with_capacity(SAMPLES);
    let mut total_checksum = 0_u64;
    for _ in 0..SAMPLES {
        TASK_ARGUMENT.store(0, Ordering::SeqCst);
        DRIVER_CHECKSUM.store(0, Ordering::SeqCst);
        let start = Instant::now();
        unsafe { zirk_rt_run_main(scheduler_driver) };
        durations.push(start.elapsed());
        assert_eq!(
            TASK_ARGUMENT.load(Ordering::SeqCst),
            TASKS_PER_SAMPLE as u64
        );
        let checksum = DRIVER_CHECKSUM.load(Ordering::SeqCst);
        assert_eq!(checksum, (0..TASKS_PER_SAMPLE as u64).sum::<u64>());
        total_checksum = total_checksum.wrapping_add(checksum);
    }
    report(
        "executor_spawn_join",
        &durations,
        TASKS_PER_SAMPLE,
        total_checksum,
    );
}

static POOL_CHECKSUM: AtomicU64 = AtomicU64::new(0);

extern "C-unwind" fn pool_item(_: *mut std::ffi::c_void, index: i64) -> usize {
    POOL_CHECKSUM.fetch_add(index as u64 + 1, Ordering::Relaxed);
    0
}

extern "C-unwind" fn pool_driver() {
    // This is the generated-code ABI symbol, intentionally declared here
    // because the runtime keeps its Rust module private.
    unsafe extern "C" {
        #[link_name = "zirk_rt_parallel_for"]
        fn parallel_for(
            kind: u8,
            a: i64,
            b: i64,
            count: i64,
            body: extern "C-unwind" fn(*mut std::ffi::c_void, i64) -> usize,
            capture: *mut std::ffi::c_void,
        );
    }
    unsafe { parallel_for(0, 0, 0, POOL_ITEMS as i64, pool_item, std::ptr::null_mut()) };
}

#[test]
#[ignore = "opt-in release concurrency benchmark"]
fn benchmark_parallel_pool_scheduling_and_queue_throughput() {
    context("parallel_pool_scheduling");
    let mut durations = Vec::with_capacity(SAMPLES);
    let expected = (1..=POOL_ITEMS as u64).sum::<u64>();
    for _ in 0..SAMPLES {
        POOL_CHECKSUM.store(0, Ordering::SeqCst);
        let start = Instant::now();
        unsafe { zirk_rt_run_main(pool_driver) };
        durations.push(start.elapsed());
        assert_eq!(POOL_CHECKSUM.load(Ordering::SeqCst), expected);
    }
    report(
        "parallel_pool_scheduling",
        &durations,
        POOL_ITEMS,
        expected * SAMPLES as u64,
    );
}

#[test]
#[ignore = "opt-in release concurrency benchmark"]
fn benchmark_scalar_atomic_operations() {
    context("scalar_atomic");
    let cell = AtomicCell::new(AtomicKind::U64, 0).expect("host supports 64-bit atomics");
    let mut durations = Vec::with_capacity(SAMPLES);
    let mut checksum = 0_u64;
    for _ in 0..SAMPLES {
        cell.store(0);
        let start = Instant::now();
        for _ in 0..ATOMIC_OPS {
            black_box(cell.fetch_add(1).unwrap());
            black_box(cell.load());
        }
        durations.push(start.elapsed());
        let observed = cell.load();
        assert_eq!(observed, ATOMIC_OPS as u64);
        checksum = checksum.wrapping_add(observed);
    }
    report(
        "scalar_atomic_fetch_add_load",
        &durations,
        ATOMIC_OPS * 2,
        checksum,
    );
}

#[test]
#[ignore = "opt-in release concurrency benchmark"]
fn benchmark_service_admission_and_readiness() {
    context("service_admission");
    const SERVICES: usize = 64;
    const CYCLES: usize = 128;
    let mut durations = Vec::with_capacity(SAMPLES);
    let mut checksum = 0_u64;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for cycle in 0..CYCLES {
            let supervisor = ServiceSupervisor::with_capacity(SERVICES).unwrap();
            let ids = (0..SERVICES)
                .map(|i| {
                    supervisor
                        .register(format!("service-{cycle}-{i}"), true)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(supervisor.begin_startup().unwrap(), ServiceState::Starting);
            for id in &ids {
                supervisor.begin_service(*id).unwrap();
                let state = supervisor.report_ready(*id).unwrap();
                if *id == *ids.last().unwrap() {
                    assert_eq!(state, ServiceState::Ready);
                }
            }
            let stop = supervisor.request_shutdown().unwrap();
            assert_eq!(stop, ids.iter().rev().copied().collect::<Vec<_>>());
            for id in stop {
                supervisor.notify_stopped(id).unwrap();
            }
            assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
            checksum = checksum.wrapping_add(ids.len() as u64 + cycle as u64);
        }
        durations.push(start.elapsed());
    }
    let work = SERVICES * CYCLES;
    assert_eq!(
        checksum,
        (0..SAMPLES as u64)
            .map(|_| {
                (0..CYCLES as u64)
                    .map(|cycle| SERVICES as u64 + cycle)
                    .sum::<u64>()
            })
            .sum::<u64>()
    );
    report(
        "service_registration_start_ready_shutdown",
        &durations,
        work,
        checksum,
    );
}
