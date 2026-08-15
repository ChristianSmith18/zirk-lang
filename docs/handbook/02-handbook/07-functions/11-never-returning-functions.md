# Never-Returning Functions

A function with result type `Never` cannot complete normally.

```zirk
fn stop(message: String): Never {
    fatalError(message);
}
```

`Never` participates in flow analysis. A branch that stops permanently can coexist with another branch producing a value because it never reaches the merge point.

Use `Never` only for real non-returning behavior such as fatal termination or an intentional endless process. Throwing a recoverable exception does not automatically make the entire function's public result `Never`.

---

**Previous:** [← No Traditional Overloading](./10-no-traditional-overloading.md) · **Next:** [Generics →](../11-generics/README.md)
