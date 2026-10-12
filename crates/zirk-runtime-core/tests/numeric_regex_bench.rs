//! Opt-in release microbenchmarks for public runtime-core numeric and regex APIs.
//!
//! Run with: cargo test -p zirk-runtime-core --test numeric_regex_bench --release -- --ignored --nocapture

use std::hint::black_box;
use std::time::{Duration, Instant};

use zirk_runtime_core::{decimal, regex};

const ROUNDS: usize = 3;
const DECIMAL_OPERATIONS: usize = 250_000;
const REGEX_VALIDATIONS: usize = 100_000;
const REGEX_MATCHES: usize = 500_000;

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn report_host() {
    println!("host: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc must be available when running this benchmark");
    println!(
        "toolchain: {}",
        String::from_utf8_lossy(&rustc.stdout).trim()
    );
    println!("profile: release (cargo --release; workspace profile sets LTO and codegen-units=1)");
    println!("rounds: {ROUNDS}; median is the middle elapsed duration");
}

fn measure(mut work: impl FnMut() -> u128) -> (Duration, u128) {
    let start = Instant::now();
    let checksum = black_box(work());
    (start.elapsed(), checksum)
}

#[test]
#[ignore = "manual release microbenchmark; run with --release --ignored --nocapture"]
fn numeric_regex_release_microbenchmark() {
    report_host();

    let left = decimal::parse("123456789.0123456789").unwrap();
    let right = decimal::parse("9876.5432109876").unwrap();
    let mut times = Vec::with_capacity(ROUNDS);
    let mut expected = None;
    for round in 0..ROUNDS {
        let (elapsed, checksum) = measure(|| {
            let mut value = left;
            let mut checksum = 0u128;
            for i in 0..DECIMAL_OPERATIONS {
                value = decimal::add(value, right).unwrap();
                value = decimal::sub(value, right).unwrap();
                let product = decimal::mul(left, right).unwrap();
                let quotient = decimal::div(product, right).unwrap();
                let rem = decimal::rem(left, right).unwrap();
                checksum = checksum
                    .wrapping_add(value.coef.unsigned_abs())
                    .wrapping_add(quotient.coef.unsigned_abs())
                    .wrapping_add(rem.coef.unsigned_abs())
                    .wrapping_add(i as u128);
            }
            checksum
        });
        assert_eq!(
            expected.get_or_insert(checksum),
            &checksum,
            "Decimal checksum changed between rounds"
        );
        println!(
            "decimal round {}: operations={DECIMAL_OPERATIONS} (add+sub+mul+div+rem each), elapsed={elapsed:?}, checksum={checksum}",
            round + 1
        );
        times.push(elapsed);
    }
    println!(
        "decimal median: {:?}; checksum={}",
        median(times),
        expected.unwrap()
    );

    let pattern = r"^(?:[A-Z]{2}-)?[0-9]{4}(?:\.[0-9]{1,6})?$";
    let matching = "AB-2048.123456";
    let nonmatching = "ab-2048.123456";
    let mut times = Vec::with_capacity(ROUNDS);
    let mut expected = None;
    for round in 0..ROUNDS {
        let (elapsed, checksum) = measure(|| {
            let mut checksum = 0u128;
            for _ in 0..REGEX_VALIDATIONS {
                checksum += u128::from(regex::is_valid(black_box(pattern)));
            }
            checksum
        });
        assert_eq!(
            checksum, REGEX_VALIDATIONS as u128,
            "unexpected regex validation result"
        );
        assert_eq!(expected.get_or_insert(checksum), &checksum);
        println!(
            "regex validation round {}: patterns={REGEX_VALIDATIONS} (compile+validate), elapsed={elapsed:?}, checksum={checksum}",
            round + 1
        );
        times.push(elapsed);
    }
    println!(
        "regex validation median: {:?}; checksum={}",
        median(times),
        expected.unwrap()
    );

    let compiled = regex::compile(pattern).unwrap();
    let mut times = Vec::with_capacity(ROUNDS);
    let mut expected = None;
    for round in 0..ROUNDS {
        let (elapsed, checksum) = measure(|| {
            let mut checksum = 0u128;
            for i in 0..REGEX_MATCHES {
                let text = if i % 2 == 0 { matching } else { nonmatching };
                checksum += u128::from(regex::is_match(&compiled, black_box(text)));
            }
            checksum
        });
        let want = (REGEX_MATCHES / 2) as u128;
        assert_eq!(checksum, want, "unexpected regex matching result");
        assert_eq!(expected.get_or_insert(checksum), &checksum);
        println!(
            "regex match round {}: inputs={REGEX_MATCHES}, elapsed={elapsed:?}, checksum={checksum}",
            round + 1
        );
        times.push(elapsed);
    }
    println!(
        "regex match median: {:?}; checksum={}",
        median(times),
        expected.unwrap()
    );
}
