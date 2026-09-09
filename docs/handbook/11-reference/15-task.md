# `Task<T>`

`Task<T>` is the compiler-known handle returned by `task`. Its element type is
the exact type produced by `await`.

```zirk
fn square(value: Int32): Int32 { return value * value; }

fn main(): Void {
    mut work: Task<Int32> = task square(9);
    mut result: Int32 = await work;
    stdout.println(result);
}
```

## Current contract

- `task expression` and `task { ... }` create a handle.
- `await handle` consumes the handle and yields `T`.
- A local handle may be awaited once; a second await or later use is an error.
- Ignoring a task expression is an error unless `_ =` explicitly discards it.
- Task results currently occupy one ABI word: references, `String`, `Char`,
  `Boolean`, `Duration`, `Void`, and integers or binary floats up to 64 bits.
  Nullable and aggregate results await the future boxed-result representation.

`task scope`, cancellation, `await ... timeout`, aggregation, channels, and
task status APIs remain deferred. See [Tasks](../02-handbook/18-concurrency/02-tasks.md)
and [await](../02-handbook/18-concurrency/03-await.md) for the guided material.

---

**Previous:** [← Temporal Reference](14-temporal-reference.md) · **Next:** [ Explanations](../12-explanations/README.md)
