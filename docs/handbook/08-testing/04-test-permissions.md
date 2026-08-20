# Test Permissions

Tests receive no implicit filesystem, network, process, environment, or native access. Declare only the capabilities required by the tested boundary and separate privileged E2E suites from pure unit tests.

CI must fail rather than grant a missing permission interactively.

Test-only grants are declared in the project test configuration and still pass
through signed approval. They do not widen the production application grant and
cannot authorize a library. Updating a requester dependency or widening a test
scope triggers the same reapproval rules as normal code.

Prefer runner capabilities over host access:

- runner temporary directories instead of arbitrary filesystem paths;
- isolated loopback servers instead of public network origins;
- injected environment maps instead of the developer's process environment;
- deterministic clocks/random sources instead of changing host state.

Native, shell, broad environment, or external-network tests should be tagged
and separated so CI policy can require a dedicated trusted job. Structured JSON
reports redact permission records and secrets.

---

**Previous:** [← assert and expect](03-assert-and-expect.md) · **Next:** [ Concurrent Test Safety](05-concurrent-test-safety.md)
