//! Manual, deterministic compiler timing sample; intentionally excluded from
//! the normal test suite. Run with:
//! `cargo test -p zirk-cli --test compile_time_bench --release -- --ignored --nocapture`

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zirk_cli::driver::{CodegenOptimizationLevel, compile_with_optimization};

const ITERATIONS: usize = 11;
static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zirk_compile_time_bench_{}_{}_{}",
            std::process::id(),
            nonce,
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create unique benchmark directory");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scalar_source() -> &'static str {
    r#"import { stdout } from std.io;
fn main(): Void {
    inmut seed: Int32 = 17;
    mut total: Int64 = 0;
    for i in 0..256 {
        total = total + (seed * i) as Int64;
    }
    stdout.println(total.to_string());
}"#
}

fn aggregate_source() -> &'static str {
    r#"import { stdout } from std.io;
fn main(): Void {
    mut values: Array<Int32> = Array(8);
    mut names: List<String> = List();
    mut seen: Set<Int32> = Set();
    mut counts: Map<Int32, Int32> = Map();
    values[0] = 3; values[1] = 6; values[2] = 9; values[3] = 12;
    values[4] = 15; values[5] = 18; values[6] = 21; values[7] = 24;
    names.add("alpha"); names.add("beta"); names.add("gamma"); names.add("delta");
    seen.add(1); seen.add(2); seen.add(3); seen.add(5); seen.add(8);
    counts.set(1, 11); counts.set(2, 22); counts.set(3, 33); counts.set(4, 44);
    inmut checksum: Int32 = values[0] + values[1] + values[2] + values[3]
        + values[4] + values[5] + values[6] + values[7]
        + (counts.get_or_null(1) ?? 0) + (counts.get_or_null(2) ?? 0)
        + (counts.get_or_null(3) ?? 0) + (counts.get_or_null(4) ?? 0);
    stdout.println(checksum.to_string());
    stdout.println(names.length.to_string());
    stdout.println(seen.length.to_string());
}"#
}

fn runtime_workload() -> &'static str {
    r#"import { stdout } from std.io;
fn main(): Void {
    mut total: Int64 = 0;
    for i in 0..250000 {
        inmut wide: Int64 = i as Int64;
        total = total + ((wide * wide) % 97);
    }
    mut counts: Map<Int32, Int32> = Map();
    mut seen: Set<Int32> = Set();
    counts.set(1, 12000);
    counts.set(2, 18000);
    seen.add(1);
    seen.add(2);
    seen.add(3);
    stdout.println(total.to_string());
    stdout.println(counts.length.to_string());
    stdout.println(seen.length.to_string());
}"#
}

fn expose_runtime_to_driver() -> Option<PathBuf> {
    let executable = std::env::current_exe().expect("benchmark executable path");
    let dependency_dir = executable.parent().expect("dependency directory");
    let target_dir = dependency_dir.parent().expect("target directory");
    let filename = if cfg!(windows) {
        "zirk_runtime.lib"
    } else {
        "libzirk_runtime.a"
    };
    let source = target_dir.join(filename);
    assert!(
        source.is_file(),
        "build `zirk-runtime` before this benchmark"
    );
    let destination = dependency_dir.join(filename);
    if destination.exists() {
        return None;
    }
    fs::copy(source, &destination).expect("expose runtime archive to compiler driver");
    Some(destination)
}

fn run_build(source: &Path, cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("build")
        .arg(source.file_name().expect("source filename"))
        .current_dir(cwd)
        .output()
        .expect("run zirk build")
}

fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn benchmark(name: &str, source_text: &str) {
    let temp = TempDir::new();
    let source = temp.0.join(format!("{name}.zrk"));
    fs::write(&source, source_text).expect("write fixed source fixture");
    let source_size = fs::metadata(&source).expect("source metadata").len();

    let warmup = run_build(&source, &temp.0);
    assert!(
        warmup.status.success(),
        "{name} warmup failed:\n{}\n{}",
        String::from_utf8_lossy(&warmup.stdout),
        String::from_utf8_lossy(&warmup.stderr)
    );
    let executable_name = String::from_utf8_lossy(&warmup.stdout).trim().to_owned();
    let executable = temp.0.join(&executable_name);
    assert!(
        executable.is_file(),
        "build output missing: {}",
        executable.display()
    );

    let mut durations = Vec::with_capacity(ITERATIONS);
    for iteration in 0..ITERATIONS {
        let start = Instant::now();
        let output = run_build(&source, &temp.0);
        durations.push(start.elapsed());
        assert!(
            output.status.success(),
            "{name} iteration {iteration} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let reported_path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        assert_eq!(reported_path, executable_name, "unstable output path");
    }

    let run = Command::new(&executable)
        .output()
        .expect("run emitted executable");
    assert!(run.status.success(), "{name} executable failed");
    let executable_bytes = fs::read(&executable).expect("read emitted executable");
    let min = durations.iter().min().copied().expect("timing samples");
    let max = durations.iter().max().copied().expect("timing samples");
    println!(
        "case={name} iterations={ITERATIONS} median_ms={:.3} min_ms={:.3} max_ms={:.3} source_bytes={source_size} executable_bytes={} executable_sha256={} output_sha256={} output={:?}",
        median(durations).as_secs_f64() * 1000.0,
        min.as_secs_f64() * 1000.0,
        max.as_secs_f64() * 1000.0,
        executable_bytes.len(),
        checksum(&executable_bytes),
        checksum(&run.stdout),
        String::from_utf8_lossy(&run.stdout).trim()
    );
}

#[test]
#[ignore = "manual deterministic compile-time benchmark"]
fn compile_time_sample() {
    let host = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc --version");
    println!(
        "host={} rustc={} profile={} warmups=1 measured_iterations={ITERATIONS}",
        std::env::consts::ARCH.to_owned() + "-" + std::env::consts::OS,
        String::from_utf8_lossy(&host.stdout).trim(),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    benchmark("small_scalar", scalar_source());
    benchmark("medium_aggregates", aggregate_source());
    println!(
        "compare only on matching host/toolchain; build timing includes object emission and native linking"
    );
}

#[test]
#[ignore = "manual runtime comparison for the experimental LLVM optimization levels"]
fn optimization_levels_runtime_benchmark() {
    const RUNS: usize = 9;
    let host = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc --version");
    println!(
        "optimization benchmark host={} rustc={} profile=release runs={RUNS}",
        std::env::consts::ARCH.to_owned() + "-" + std::env::consts::OS,
        String::from_utf8_lossy(&host.stdout).trim()
    );
    let root = TempDir::new();
    let source = root.0.join("runtime_workload.zrk");
    fs::write(&source, runtime_workload()).expect("write runtime workload");
    let copied_runtime = expose_runtime_to_driver();
    let mut expected: Option<Vec<u8>> = None;

    for (label, level) in [
        ("none", CodegenOptimizationLevel::None),
        ("less", CodegenOptimizationLevel::Less),
        ("default", CodegenOptimizationLevel::Default),
        ("aggressive", CodegenOptimizationLevel::Aggressive),
    ] {
        let output_dir = root.0.join(label);
        let start = Instant::now();
        let compilation = compile_with_optimization(&source, &output_dir, level);
        let compile_elapsed = start.elapsed();
        let executable = compilation.executable.unwrap_or_else(|| {
            panic!(
                "{label} compilation failed:\n{}",
                compilation
                    .sink
                    .render(zirk_diagnostics::RenderStyle::Human)
            )
        });
        let mut durations = Vec::with_capacity(RUNS);
        let mut output = None;
        for _ in 0..RUNS {
            let start = Instant::now();
            let result = Command::new(&executable)
                .output()
                .expect("run benchmark executable");
            durations.push(start.elapsed());
            assert!(result.status.success(), "{label} executable failed");
            if let Some(expected) = &expected {
                assert_eq!(&result.stdout, expected, "{label} output diverged");
            } else {
                expected = Some(result.stdout.clone());
            }
            output = Some(result.stdout);
        }
        let min = durations.iter().min().copied().expect("runtime samples");
        let max = durations.iter().max().copied().expect("runtime samples");
        println!(
            "level={label} compile_ms={:.3} runtime_median_ms={:.3} runtime_min_ms={:.3} runtime_max_ms={:.3} executable_bytes={} output_sha256={}",
            compile_elapsed.as_secs_f64() * 1000.0,
            median(durations).as_secs_f64() * 1000.0,
            min.as_secs_f64() * 1000.0,
            max.as_secs_f64() * 1000.0,
            fs::metadata(&executable)
                .expect("executable metadata")
                .len(),
            checksum(&output.expect("runtime output"))
        );
    }
    if let Some(runtime) = copied_runtime {
        fs::remove_file(runtime).expect("remove copied runtime archive");
    }
}
