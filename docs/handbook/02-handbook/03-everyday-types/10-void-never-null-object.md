# Void, Never, Null, and Object

These types describe important boundaries:

- `Void` is the result of a function that completes without returning a value.
- `Never` is the result of a path that cannot complete normally, such as an irreparable termination.
- `Null` has the single value `null` and participates only in nullable types.
- `Object` is the broad semantic root of values.

```zirk
fn log_ready(): Void { stdout.println("ready"); }
mut selected: String? = null;
```

`Never` helps flow analysis: a branch that cannot return need not produce the value required by later code. `Void` is not `Null`, and `Object` does not allow bypassing the operations of the concrete static type.

---

**Previous:** [← String](./09-string.md) · **Next:** [Duration →](./11-duration.md)
