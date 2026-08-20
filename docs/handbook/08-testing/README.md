# Testing

Zirk separates unit, end-to-end, and benchmark workloads. Tests use ordinary public contracts, receive no private access, require explicit capabilities, and never enter release binaries.

Test files are ordinary typed Zirk modules with runner-discovered decorators.
`*.spec.zrk` owns unit tests and `*.e2e.zrk` owns end-to-end tests. The runner
compiles them only for `zirk test`; application/library artifacts do not retain
test declarations or decorator metadata.

The complete API—decorators, assertions, fixtures, filtering, reports, and
benchmark contracts—is owned by [`std.testing`](../04-standard-library/15-std-testing.md).
This unit explains how to apply those contracts safely in projects.

---

**Previous:** [← Performance Goals](../07-toolchain/18-performance-goals.md) · **Next:** [ Unit Tests](01-unit-tests.md)
