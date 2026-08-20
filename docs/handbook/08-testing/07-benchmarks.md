# Benchmarks

`@bench` runs through `zirk bench` with warmup, multiple samples, statistics, dead-code prevention, and machine-readable output.

Record target, toolchain, profile, hardware, variance, and input. A benchmark is evidence, not a language guarantee independent of hardware.

```zirk
@bench
fn parse_manifest(bench: Bench): Void {
    bench.measure(fn() => parse_manifest(source));
}
```

The runner performs warm-up, calibration, multiple independent samples, and
dead-code prevention. Reports include sample count, median, dispersion,
percentiles, outliers, allocation/resource metrics when available, and the
complete environment needed for comparison.

Benchmarks run in an optimized benchmark profile while preserving language
checks. Setup outside `measure` prepares input; code inside it is the measured
operation. Stateful input must be reset explicitly between samples. I/O and
network benchmarks declare permissions and are labeled because host noise makes
them unsuitable for tight default regression thresholds.

CI compares against a reviewed baseline with statistical and absolute
thresholds. A detected regression reports evidence; it does not automatically
rewrite the baseline.

---

**Previous:** [← Test Runner](06-test-runner.md) · **Next:** [ Packages](../09-packages/README.md)
