# Default Parameters

A default expression supplies a value when the caller omits an argument.

```zirk
fn connect(retries: Int32 = 3): Result<Connection, ConnectError> {
    /* ... */
}
```

The default must satisfy the parameter type and is evaluated according to the function-call contract. It does not accept an explicitly incompatible or nullable value.

Defaults belong in APIs when one behavior is genuinely conventional. If choosing the value has permissions, I/O, or surprising cost, require the caller to be explicit.

---

**Previous:** [← Optional Parameters](03-optional-parameters.md) · **Next:** [ Named Arguments](05-named-arguments.md)
