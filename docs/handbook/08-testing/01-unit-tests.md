# Unit Tests

`@test` is valid exclusively in `.spec.zrk` files.

```zirk
@test
fn creates_user(): Void { assert.equal(actual, expected); }
```

Unit tests should isolate one contract, avoid uncontrolled I/O, and remain deterministic under parallel execution.

---

**Previous:** [← Testing](README.md) · **Next:** [ End-to-End Tests](02-e2e-tests.md)
