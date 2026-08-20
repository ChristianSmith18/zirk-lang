# `assert` and `expect`

Assertions cover equality, identity, truth, nullability, results, exceptions, collections, and approximate decimals. Failures show source location, actual and expected values, and a useful diff.

Use one assertion vocabulary consistently; helper APIs should preserve the original source span.

```zirk
assert.equal(actual, expected);
assert.identical(first, alias);
assert.approx(measured, expected, tolerance: 0.001);
assert.is_ok(result);
expect(users).to_contain(expected_user);
expect(fn() => load()).to_throw<LoadError>();
```

`equal` uses Zirk `==`; `identical` uses identity `is` and therefore applies only to
reference-backed values. Approximate comparison requires an explicit finite,
non-negative tolerance—there is no hidden global epsilon. Result assertions use
`Ok/Error`; `Value/End/Error` assertions belong to the specific stream enum.

Collection failures report the first useful mismatch plus bounded context.
String failures use a Unicode-aware diff without exposing secret values.
Throwable assertions execute an explicit closure and may check type, message,
code, cause, or structured provenance.

---

**Previous:** [← End-to-End Tests](02-e2e-tests.md) · **Next:** [ Test Permissions](04-test-permissions.md)
