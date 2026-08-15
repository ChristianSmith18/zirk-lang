# Concurrent Test Safety

The runner may execute independent tests concurrently. Tests must not share unsynchronized globals, fixed ports, mutable directories, clocks, or random state.

Use unique fixtures and `--seed` for reproducibility. A failure must report the seed and scheduling-relevant context.

---

**Previous:** [← Test Permissions](./04-test-permissions.md) · **Next:** [Test Runner →](./06-test-runner.md)
