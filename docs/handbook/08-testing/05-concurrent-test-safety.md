# Concurrent Test Safety

The runner may execute independent tests concurrently. Tests must not share unsynchronized globals, fixed ports, mutable directories, clocks, or random state.

Use unique fixtures and `--seed` for reproducibility. A failure must report the seed and scheduling-relevant context.

`--jobs` controls how many independent test cases run at once; it does not
change concurrency inside one test. The runner may serialize tests whose
declared fixtures identify an exclusive resource, but hidden global coupling is
still a test bug.

Deterministic facilities provide seeded random values, a controllable monotonic
test clock, isolated environment values, and allocated ports/paths. Advancing a
test clock wakes eligible timers in defined order without sleeping the host.
Cancellation waits for child tasks and resource cleanup before the runner
starts teardown.

Timeout failure reports unfinished tasks, held synchronization objects, open
resources, and the seed. Leaked tasks, handles, or temporary files are separate
test failures even when assertions passed.

---

**Previous:** [← Test Permissions](04-test-permissions.md) · **Next:** [ Test Runner](06-test-runner.md)
