# Generic Functions

A generic function introduces type parameters used by its signature and body.

```zirk
fn identity<T>(value: T): T {
    return value;
}
```

The body is checked using only operations guaranteed for `T`. Each call must establish a concrete type that satisfies all constraints.

Generics replace duplicated algorithms without erasing type information. They are preferable to `Object` plus unsafe casts because callers retain the relationship between input and output types.

---

**Previous:** [← Generics](README.md) · **Next:** [ Generic Types](02-generic-types.md)
