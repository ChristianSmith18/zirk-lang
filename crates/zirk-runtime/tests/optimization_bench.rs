// Repeatable, opt-in microbenchmarks for runtime hardening hot paths.
// Run with: cargo test -p zirk-runtime --test optimization_bench --release -- --ignored --nocapture
// The source modules are compiled into this test crate so private Map/Set and
// clone paths can be measured without changing the runtime ABI or Cargo.toml.
#![allow(dead_code, unused_unsafe)]

#[path = "../src/array.rs"]
mod array;
#[path = "../src/background.rs"]
mod background;
#[path = "../src/channel.rs"]
mod channel;
#[path = "../src/char.rs"]
mod char;
#[path = "../src/clone.rs"]
mod clone;
#[path = "../src/collector.rs"]
mod collector;
#[path = "../src/combinators.rs"]
mod combinators;
#[path = "../src/context.rs"]
mod context;
#[path = "../src/decimal.rs"]
mod decimal;
#[path = "../src/deque.rs"]
mod deque;
#[path = "../src/duration.rs"]
mod duration;
#[path = "../src/exceptions.rs"]
mod exceptions;
#[path = "../src/executor.rs"]
mod executor;
#[path = "../src/failure.rs"]
mod failure;
#[path = "../src/host_signal.rs"]
mod host_signal;
#[path = "../src/io.rs"]
mod io;
#[path = "../src/journal.rs"]
mod journal;
#[path = "../src/list.rs"]
mod list;
#[path = "../src/map.rs"]
mod map;
#[path = "../src/memory.rs"]
mod memory;
#[path = "../src/ordered.rs"]
mod ordered;
// This benchmark compiles selected runtime modules into a deliberately
// minimal test crate. None of its measured task paths own network resources.
mod network_api {
    pub(crate) fn close_task_streams(_: crate::task::TaskId) {}
}
mod http_api {
    pub(crate) fn close_task_streams(_: crate::task::TaskId) {}
    pub(crate) fn close_task_server_controls(_: crate::task::TaskId) {}
    pub(crate) fn close_task_server_producers(_: crate::task::TaskId) {}
    pub(crate) fn close_task_uploads(_: crate::task::TaskId) {}
    pub(crate) fn close_executor_clients(_: u64) {}
}
#[path = "../src/native_slice.rs"]
mod native_slice;
#[path = "../src/object.rs"]
mod object;
#[path = "../src/pool.rs"]
mod pool;
#[path = "../src/range.rs"]
mod range;
#[path = "../src/regex.rs"]
mod regex;
#[path = "../src/resource.rs"]
mod resource;
#[path = "../src/scalar.rs"]
mod scalar;
#[path = "../src/services.rs"]
mod services;
#[path = "../src/set.rs"]
mod set;
#[path = "../src/string.rs"]
mod string;
#[path = "../src/sync_mutex.rs"]
mod sync_mutex;
#[path = "../src/task.rs"]
mod task;
#[path = "../src/temporal.rs"]
mod temporal;
#[path = "../src/timer.rs"]
mod timer;

pub use array::*;
pub use channel::*;
pub use char::*;
pub use collector::{zirk_rt_pop_frame, zirk_rt_push_frame};
pub use context::{
    DEFAULT_TASK_STACK_BYTES, Run, Suspender, TaskContext, spawn_default, suspend_current,
};
pub use failure::{
    zirk_rt_allocation_failed, zirk_rt_division_by_zero, zirk_rt_fatal_error,
    zirk_rt_index_out_of_bounds, zirk_rt_overflow,
};
pub use io::{zirk_io_print, zirk_io_println};
pub use journal::{
    zirk_rt_journal_begin, zirk_rt_journal_commit, zirk_rt_journal_record, zirk_rt_journal_rollback,
};
pub use list::*;
pub use memory::{zirk_rt_alloc, zirk_rt_dependent_base, zirk_rt_pin_object, zirk_rt_unpin_object};
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
pub use string::{
    zirk_str_concat, zirk_str_eq, zirk_str_from_bool, zirk_str_from_f32, zirk_str_from_f64,
    zirk_str_from_i8, zirk_str_from_i16, zirk_str_from_i32, zirk_str_from_i64, zirk_str_from_i128,
    zirk_str_from_u8, zirk_str_from_u16, zirk_str_from_u32, zirk_str_from_u64, zirk_str_from_u128,
    zirk_str_from_utf8, zirk_str_grapheme_len_at, zirk_str_grapheme_slice, zirk_str_hash,
    zirk_str_is_ascii, zirk_str_repeat, zirk_str_set, zirk_str_slice,
};

// The production crate-root wrappers are reproduced locally because this
// standalone harness compiles the same source modules as a test crate.
/// # Safety
/// `descriptor` must refer to a valid static descriptor and `size` must cover
/// the callable payload expected by that descriptor.
pub unsafe fn zirk_rt_alloc_callable(descriptor: *const c_void, size: u64) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let total = (size as usize).saturating_add(HEADER_BYTES);
    let pointer = unsafe { crate::memory::zirk_rt_alloc(total, std::mem::size_of::<usize>()) };
    unsafe { *(pointer as *mut *const c_void) = descriptor };
    pointer
}

/// # Safety
/// `callable` must be null or a live callable allocation owned by this runtime.
pub unsafe fn zirk_rt_clone_callable(callable: *const c_void) -> *mut c_void {
    if callable.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { crate::clone::zirk_rt_clone(callable as *mut c_void) }
}

use std::ffi::c_void;
use std::hint::black_box;
use std::time::{Duration, Instant};

mod common;

const NODES: usize = 4_096;
const GRAPH_ROUNDS: usize = 3;
const CHURN_ITEMS: usize = 8_192;
const CHURN_ROUNDS: usize = 5;
const HEADER_BYTES: usize = 3 * std::mem::size_of::<usize>();

// Descriptor format: [reserved, ancestor_count, contract_count, gc_count,
// reference_field_offset]. The node payload is [next: pointer, id: usize].
static NODE_DESCRIPTOR: [usize; 5] = [0, 0, 0, 1, HEADER_BYTES];

fn report(name: &str, profile: &str, rounds: usize, work: usize, elapsed: Duration, checksum: u64) {
    println!(
        "BENCH name={name} profile={profile} rounds={rounds} work_per_round={work} total_ns={} ns_per_work={:.2} checksum={checksum}",
        elapsed.as_nanos(),
        elapsed.as_nanos() as f64 / (rounds * work) as f64,
    );
    common::report_process_peak_rss(name);
}

unsafe fn allocate_node(id: usize) -> *mut c_void {
    let node = unsafe {
        crate::zirk_rt_alloc_callable(
            NODE_DESCRIPTOR.as_ptr().cast(),
            (2 * std::mem::size_of::<usize>()) as u64,
        )
    };
    assert!(!node.is_null());
    unsafe { ((node as *mut usize).add(4)).write(id) };
    node
}

unsafe fn next(node: *mut c_void) -> *mut c_void {
    unsafe { *((node as *mut *mut c_void).add(3)) }
}

unsafe fn set_next(node: *mut c_void, child: *mut c_void) {
    unsafe { ((node as *mut *mut c_void).add(3)).write(child) };
}

unsafe fn node_id(node: *mut c_void) -> usize {
    unsafe { *((node as *mut usize).add(4)) }
}

fn toolchain() -> String {
    std::process::Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_else(|| "rustc version unavailable".into())
}

fn os_version() -> String {
    std::process::Command::new("sw_vers")
        .args(["-productVersion"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "version-unavailable".into())
}

unsafe fn graph_checksum(root: *mut c_void, expected_nodes: usize) -> u64 {
    let mut checksum = 0_u64;
    let mut node = root;
    let mut visited = 0;
    while !node.is_null() {
        let id = unsafe { node_id(node) };
        checksum = checksum.wrapping_add((id as u64 + 1).wrapping_mul(0x9E37_79B1));
        visited += 1;
        node = unsafe { next(node) };
    }
    assert_eq!(visited, expected_nodes, "graph shape changed");
    checksum
}

#[test]
#[ignore = "opt-in release microbenchmark; see file header for the exact command"]
fn benchmark_gc_marking_and_deep_clone() {
    println!(
        "CONTEXT os={} os_version={} arch={} toolchain=[{}] profile=release opt_level=3 lto=true codegen_units=1 target=host-native nodes={} graph_rounds={} allocator=system",
        std::env::consts::OS,
        os_version(),
        std::env::consts::ARCH,
        toolchain(),
        NODES,
        GRAPH_ROUNDS,
    );

    let mut source_root = std::ptr::null_mut();
    let mut roots = [(&mut source_root as *mut *mut c_void).cast()];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };

    let mut tail: *mut c_void = std::ptr::null_mut();
    for id in 0..NODES {
        let node = unsafe { allocate_node(id) };
        if tail.is_null() {
            source_root = node;
        } else {
            unsafe { set_next(tail, node) };
        }
        tail = node;
    }
    let expected = unsafe { graph_checksum(source_root, NODES) };

    // Allocations exceed the collector's initial live-byte threshold several
    // times while the chain is rooted. The checksum confirms the graph stayed
    // reachable throughout collection pressure.
    let marking_start = Instant::now();
    let mut pressure_checksum = 0_u64;
    for i in 0..GRAPH_ROUNDS * NODES {
        let garbage = unsafe { allocate_node(i) };
        black_box(garbage);
        pressure_checksum = pressure_checksum.wrapping_add(i as u64);
    }
    let marking_elapsed = marking_start.elapsed();
    assert_eq!(unsafe { graph_checksum(source_root, NODES) }, expected);
    assert_eq!(
        pressure_checksum,
        ((GRAPH_ROUNDS * NODES - 1) * (GRAPH_ROUNDS * NODES)) as u64 / 2
    );
    report(
        "gc_mark_pressure",
        "release",
        GRAPH_ROUNDS,
        NODES,
        marking_elapsed,
        expected,
    );

    let mut clones = vec![std::ptr::null_mut::<c_void>(); GRAPH_ROUNDS];
    let mut clone_roots = clones
        .iter_mut()
        .map(|slot| (slot as *mut *mut c_void).cast())
        .collect::<Vec<*mut c_void>>();
    unsafe { crate::zirk_rt_push_frame(clone_roots.as_mut_ptr(), clone_roots.len() as i64) };
    let clone_start = Instant::now();
    for clone in &mut clones {
        *clone = unsafe { crate::clone::zirk_rt_clone(source_root) };
        assert!(!clone.is_null());
        assert_ne!(*clone, source_root);
    }
    let clone_elapsed = clone_start.elapsed();
    let mut clone_checksum = 0_u64;
    for &clone in &clones {
        let observed = unsafe { graph_checksum(clone, NODES) };
        assert_eq!(observed, expected);
        clone_checksum = clone_checksum.wrapping_add(observed);
    }
    report(
        "deep_clone_chain",
        "release",
        GRAPH_ROUNDS,
        NODES,
        clone_elapsed,
        clone_checksum,
    );
    crate::zirk_rt_pop_frame();
    crate::zirk_rt_pop_frame();
    benchmark_map_set_churn();
}

fn benchmark_map_set_churn() {
    println!(
        "CONTEXT os={} os_version={} arch={} toolchain=[{}] profile=release opt_level=3 lto=true codegen_units=1 items={} rounds={} key_range=0..{} allocator=system",
        std::env::consts::OS,
        os_version(),
        std::env::consts::ARCH,
        toolchain(),
        CHURN_ITEMS,
        CHURN_ROUNDS,
        CHURN_ITEMS,
    );

    let mut checksum = 0_u64;
    let map_start = Instant::now();
    for round in 0..CHURN_ROUNDS {
        let mut map = unsafe { crate::map::zirk_rt_map_new() };
        let mut roots = [(&mut map as *mut *mut c_void).cast()];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        for key in 0..CHURN_ITEMS {
            crate::map::zirk_rt_map_set(map, key as i64, (key as i64).wrapping_mul(17) - 3);
        }
        for key in (0..CHURN_ITEMS).step_by(2) {
            assert_eq!(crate::map::zirk_rt_map_remove(map, key as i64), 1);
        }
        assert_eq!(
            crate::map::zirk_rt_map_length(map),
            (CHURN_ITEMS / 2) as u64
        );
        for key in 0..CHURN_ITEMS {
            let value = crate::map::zirk_rt_map_get(map, key as i64);
            let expected = if key % 2 == 0 {
                0
            } else {
                (key as i64) * 17 - 3
            };
            assert_eq!(value, expected);
            checksum = checksum.wrapping_add((value as u64) ^ ((key as u64) << 1) ^ round as u64);
        }
        crate::zirk_rt_pop_frame();
    }
    report(
        "map_insert_remove_lookup",
        "release",
        CHURN_ROUNDS,
        CHURN_ITEMS,
        map_start.elapsed(),
        checksum,
    );

    let mut set_checksum = 0_u64;
    let set_start = Instant::now();
    for round in 0..CHURN_ROUNDS {
        let mut set = crate::set::zirk_rt_set_new();
        let mut roots = [(&mut set as *mut *mut c_void).cast()];
        unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        for value in 0..CHURN_ITEMS {
            crate::set::zirk_rt_set_add(set, value as i64);
        }
        for value in (0..CHURN_ITEMS).step_by(2) {
            assert_eq!(crate::set::zirk_rt_set_remove(set, value as i64), 1);
        }
        assert_eq!(
            crate::set::zirk_rt_set_length(set),
            (CHURN_ITEMS / 2) as u64
        );
        for value in 0..CHURN_ITEMS {
            let present = crate::set::zirk_rt_set_contains(set, value as i64);
            assert_eq!(present, i32::from(value % 2 == 1));
            set_checksum =
                set_checksum.wrapping_add((present as u64) ^ ((value as u64) << 1) ^ round as u64);
        }
        crate::zirk_rt_pop_frame();
    }
    report(
        "set_insert_remove_contains",
        "release",
        CHURN_ROUNDS,
        CHURN_ITEMS,
        set_start.elapsed(),
        set_checksum,
    );
    black_box((checksum, set_checksum));
}

#[test]
#[ignore = "opt-in release benchmark; see file header for the exact command"]
fn benchmark_bounded_each() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ITEMS: usize = 256;
    const LIMIT: usize = 16;
    const SAMPLES: usize = 9;

    println!(
        "CONTEXT benchmark=bounded_each os={} os_version={} arch={} toolchain=[{}] profile=release opt_level=3 lto=true codegen_units=1 target=host-native items={} concurrency_limit={} samples={} allocator=system thread_ceiling=1 rss_metric=process_lifetime_peak",
        std::env::consts::OS,
        os_version(),
        std::env::consts::ARCH,
        toolchain(),
        ITEMS,
        LIMIT,
        SAMPLES,
    );

    let outcome = executor::Executor::new().run_with_root(move || {
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut elapsed = Vec::with_capacity(SAMPLES);
        let mut checksums = Vec::with_capacity(SAMPLES);

        for _ in 0..SAMPLES {
            maximum.store(0, Ordering::Relaxed);
            let active_for_each = Arc::clone(&active);
            let maximum_for_each = Arc::clone(&maximum);
            let start = Instant::now();
            let values = combinators::each_with_limit(0..ITEMS, LIMIT, move |item| {
                let now = active_for_each.fetch_add(1, Ordering::SeqCst) + 1;
                maximum_for_each.fetch_max(now, Ordering::SeqCst);

                // Yield once so the active callback window is exercised under
                // the cooperative scheduler instead of measuring only inline
                // callback completion.
                executor::yield_now();

                let value = (item as u64 + 1).wrapping_mul(0x9E37_79B1);
                active_for_each.fetch_sub(1, Ordering::SeqCst);
                black_box(value)
            })
            .expect("bounded each should complete successfully");
            elapsed.push(start.elapsed());

            assert_eq!(values.len(), ITEMS);
            assert!(values.iter().enumerate().all(|(index, value)| {
                *value == (index as u64 + 1).wrapping_mul(0x9E37_79B1)
            }));
            assert_eq!(active.load(Ordering::SeqCst), 0, "each returned before joining callbacks");
            let observed_max = maximum.load(Ordering::SeqCst);
            assert!(observed_max <= LIMIT, "active callbacks exceeded configured limit");
            assert_eq!(observed_max, LIMIT, "benchmark did not exercise the full callback window");
            checksums.push(values.iter().fold(0_u64, |sum, value| sum.wrapping_add(*value)));
        }

        let checksum = checksums[0];
        assert!(checksums.iter().all(|sample| *sample == checksum));
        assert_eq!(active.load(Ordering::SeqCst), 0, "callbacks remained active after final sample");
        let max_active = maximum.load(Ordering::SeqCst);
        let mut nanos = elapsed.iter().map(Duration::as_nanos).collect::<Vec<_>>();
        nanos.sort_unstable();
        let min = nanos[0];
        let median = nanos[nanos.len() / 2];
        let max = nanos[nanos.len() - 1];
        println!(
            "BENCH name=bounded_each profile=release items={ITEMS} concurrency_limit={LIMIT} samples={SAMPLES} min_ns={min} median_ns={median} max_ns={max} checksum={checksum} max_active_concurrency={max_active} cleanup=all_callbacks_joined"
        );
        common::report_process_peak_rss("bounded_each");
        black_box(checksum as usize)
    });

    match outcome {
        task::TaskOutcome::Value(_) => {}
        task::TaskOutcome::Panicked(payload) => {
            panic!("bounded each benchmark executor failed: {payload:?}")
        }
    }
}
