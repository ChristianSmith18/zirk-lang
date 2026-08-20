# Unit Tests

`@test` is valid exclusively in `.spec.zrk` files.

```zirk
import { assert } from std.testing;

@test
fn creates_user(): Void {
    mut user = User(name: "Ada");
    assert.equal(user.name, "Ada");
}
```

Unit tests should isolate one contract, avoid uncontrolled I/O, and remain deterministic under parallel execution.

A test may return `Void`, `Result<Void,E>`, `Task<Void>`, or
`Task<Result<Void,E>>`. The runner awaits tasks and reports returned errors
without converting expected `Result.Error` values into task rejection. Uncaught
throwables fail the test with their structured trace.

Use fixtures for repeated setup and explicit test data for edge cases. A unit
test has the same visibility as another ordinary module: it cannot access
private declarations merely because it is a test. Put a test beside the public
contract or expose a narrow internal testing contract deliberately.

```bash
zirk test --file test/user.spec.zrk
```

> **Implementation status:** the decorator and runner API is the normative 1.x
> target; consult Feature Status before assuming the current compiler executes
> every testing decorator.

---

**Previous:** [← Testing](README.md) · **Next:** [ End-to-End Tests](02-e2e-tests.md)
