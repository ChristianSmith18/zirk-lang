# Void, Never, Null, and Object

These special/root types describe boundaries rather than interchangeable empty
values.

## Null

`Null` has the single value `null`. It inhabits `T?`, equivalent to `T | Null`,
but cannot initialize `T` directly. Safe access `?.`, coalescing `??`, equality
and flow analysis narrow nullable values.

```zirk
mut selected: String? = null;
inmut name = selected ?? "anonymous"; // String
selected.length; // error until narrowed
```

There is no `undefined` and no null truthiness.

## Void

`Void` means a function completes normally without delivering a value:

```zirk
fn log_ready(): Void {
    stdout.println("ready");
}
```

It is not `Null`, cannot be stored as a missing object, and does not satisfy a
value-returning branch.

## Never

`Never` describes a path that cannot return normally: `fatalError`, a proven
infinite loop, or an always-throwing branch. Because no value is delivered, a
`Never` branch can coexist with any result type and helps definite-return/flow
analysis.

## Object

`Object` is the conceptual root, not a universal heap representation. Every
value provides `type` and `to_string()`. Equality, hashing, cloning, ordering,
iteration and arithmetic remain contract-gated; widening to `Object` does not
make those operations universally available.

---

**Previous:** [← String](09-string.md) · **Next:** [ Duration Has Moved](11-duration.md)
