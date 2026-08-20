# End-to-End Tests

E2E tests live in `test/*.e2e.zrk` and use `@e2e`. They exercise project boundaries, processes, files, networking, or complete workflows with explicitly declared permissions.

Isolate temporary state and report reproducible setup, command, output, and teardown failures.

```zirk
import { assert } from std.testing;
import { HTTP } from std.http;

@e2e
fn serves_health(): Task<Result<Void, TestError>> {
    mut response = await HTTP.get(test_server.url("/health"), timeout: 2s);
    assert.is_ok(response);
    return Ok();
}
```

The runner allocates unique temporary directories and ports through fixtures;
tests must not guess fixed global paths or ports. A failed setup prevents the
body from running. Teardown always runs, and teardown failure is preserved
alongside a body failure rather than replacing it.

E2E permissions are a reviewed test profile, not ambient developer authority.
They remain bounded to temporary paths, loopback endpoints, approved child
executables, and named environment inputs whenever possible.

---

**Previous:** [← Unit Tests](01-unit-tests.md) · **Next:** [ assert and expect](03-assert-and-expect.md)
