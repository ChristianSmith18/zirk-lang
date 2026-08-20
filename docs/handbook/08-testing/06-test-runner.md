# Test Runner

`zirk test` supports unit/E2E selection, file, tag, seed, job count, and JSON reports. Exit status reflects success, test failure, invalid configuration, or infrastructure failure distinctly.

Filtering must not change test semantics, and machine-readable output remains deterministic.

```bash
zirk test
zirk test --file test/users.spec.zrk
zirk test --tag database --seed 481516 --jobs 4
zirk test --report json
```

`--file` selects source files, `--tag` selects declared metadata, `--seed`
replays randomized ordering/data, and `--jobs` bounds runner parallelism. The
report records discovered, selected, passed, failed, skipped, canceled, and
infrastructure-error cases in stable order even if execution was concurrent.

The runner distinguishes compile/configuration failure, test assertion/error,
timeout/cancellation, teardown/leak failure, and runner infrastructure failure.
Human output may stream progress; JSON emits final structured events without
terminal control sequences or secrets.

Filtering never skips required setup for a selected test and does not alter its
seed. A zero-test selection is explicit and configurable as success or policy
failure; it is never silently mistaken for a passing suite.

---

**Previous:** [← Concurrent Test Safety](05-concurrent-test-safety.md) · **Next:** [ Benchmarks](07-benchmarks.md)
