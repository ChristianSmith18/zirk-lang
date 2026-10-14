//! Opt-in native source-level benchmark for `Concurrent.each`.
//!
//! Run with:
//! `cargo test -p zirk-cli --test concurrent_each_source_bench --release -- --ignored --nocapture`
//!
//! Compilation and one warmup run are outside the measured samples. Timings
//! are observations only; deterministic output and checksum are asserted.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ITEMS: usize = 256;
const LIMIT: usize = 16;
const SAMPLES: usize = 11;
const EXPECTED_CHECKSUM: u64 = 560_000;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zirk_concurrent_each_source_bench_{}_{}",
            std::process::id(),
            nonce
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

fn expose_runtime_to_cli() -> Option<PathBuf> {
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

fn source() -> String {
    let values = (1..=ITEMS)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"import {{ stdout }} from std.io;

fn main(): Void {{
    inmut::strict values: List<Int32> = [{values}];
    mut results = Concurrent.each(values, (value: Int32): Int32 => value * 17 + 3, {LIMIT});
    mut checksum: Int64 = 0;
    for value in results {{
        checksum = checksum + value as Int64;
    }}
    stdout.println(checksum.to_string());
}}
"#
    )
}

fn build(source: &Path, cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("build")
        .arg(source.file_name().expect("source filename"))
        .current_dir(cwd)
        .output()
        .expect("run zirk build")
}

fn run(executable: &Path) -> (Duration, String) {
    let start = Instant::now();
    let output = Command::new(executable)
        .output()
        .expect("run compiled Concurrent.each program");
    let elapsed = start.elapsed();
    assert!(
        output.status.success(),
        "compiled program failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("program output is UTF-8");
    assert_eq!(
        stdout,
        format!("{EXPECTED_CHECKSUM}\n"),
        "every warmup and sample must produce the deterministic checksum"
    );
    (elapsed, stdout)
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

#[test]
#[ignore = "opt-in release source-level Concurrent.each benchmark"]
fn benchmark_native_concurrent_each_source() {
    let runtime_copy = expose_runtime_to_cli();
    let temp = TempDir::new();
    let source_path = temp.0.join("concurrent_each_benchmark.zrk");
    fs::write(&source_path, source()).expect("write deterministic benchmark program");

    let compiled = build(&source_path, &temp.0);
    assert!(
        compiled.status.success(),
        "Zirk compilation failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executable_name = String::from_utf8_lossy(&compiled.stdout).trim().to_owned();
    let executable = temp.0.join(executable_name);
    assert!(executable.is_file(), "compiled executable was not created");

    let (_, warmup_output) = run(&executable);
    let mut durations = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let (elapsed, output) = run(&executable);
        assert_eq!(output, warmup_output, "sample output differs from warmup");
        durations.push(elapsed);
    }

    let min = durations.iter().min().copied().expect("timing samples");
    let max = durations.iter().max().copied().expect("timing samples");
    let rustc = Command::new("rustc")
        .arg("-Vv")
        .output()
        .expect("rustc -Vv");
    assert!(rustc.status.success(), "rustc -Vv failed");
    let rustc_details = String::from_utf8_lossy(&rustc.stdout);
    let target = rustc_details
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc -Vv reports its host target");
    let toolchain = rustc_details.lines().collect::<Vec<_>>().join(";");
    println!(
        "CONTEXT benchmark=native_concurrent_each_source host={}-{} target={target} rustc=[{}] profile=release items={ITEMS} concurrency_limit={LIMIT} warmups=1 samples={SAMPLES}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        toolchain,
    );
    println!(
        "BENCH name=native_concurrent_each_source items={ITEMS} concurrency_limit={LIMIT} samples={SAMPLES} min_ns={} median_ns={} max_ns={} checksum={EXPECTED_CHECKSUM}",
        min.as_nanos(),
        median(durations).as_nanos(),
        max.as_nanos(),
    );

    drop(runtime_copy);
}
