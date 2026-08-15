# Declaring Functions

Use `fn`, a `snake_case` name, a parameter list, and an explicit result type.

```zirk
fn add(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

The signature is the callable contract. Calls are checked for argument count, labels, types, visibility, effects, and generic constraints before code generation.

A function returning `Void` completes without a value. Public functions should document expected errors, exceptions, permissions, blocking, cancellation, and resource transfer where relevant.

---

**Previous:** [← Functions](./README.md) · **Next:** [Parameters and Return Types →](./02-parameters-and-return-types.md)
