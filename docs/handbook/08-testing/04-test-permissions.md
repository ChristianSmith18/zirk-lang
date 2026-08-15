# Test Permissions

Tests receive no implicit filesystem, network, process, environment, or native access. Declare only the capabilities required by the tested boundary and separate privileged E2E suites from pure unit tests.

CI must fail rather than grant a missing permission interactively.

---

**Previous:** [← `assert` and `expect`](./03-assert-and-expect.md) · **Next:** [Concurrent Test Safety →](./05-concurrent-test-safety.md)
