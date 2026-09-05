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

## API

### `Object`

The conceptual semantic root — not a universal heap representation. Widening
to `Object` does not unlock capability-gated members.

#### Members

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Runtime/static type identity; every inhabited value exposes it | specified |
| `o.to_string()` | `String` | Universal member | implemented |

Equality, hashing, ordering, iteration, indexing, arithmetic, and cloning exist
only when the type satisfies the corresponding capability contract — their
presence must not be inferred from `Object`.

#### Examples

```zirk
fn inspect(value: Object): String {
    return "{value.type}: {value.to_string()}";
}
```

### `Null`

`Null` has the single value `null`. It inhabits `T?` (≡ `T | Null`) but cannot
initialize `T` directly. No `undefined`, no null truthiness.

#### Members

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| *(none beyond `type`/`to_string()`)* | — | `null` is absence, not an object | implemented |

Related operators: safe access `?.`, coalescing `??`, equality, and
flow-analysis narrowing.

#### Examples

```zirk
mut selected: String? = null;
inmut name = selected ?? "anonymous";   // String
selected.length;                        // error until narrowed
```

### `Void`

`Void` means a function completes normally without delivering a value. It is
not `Null`, cannot be stored as a missing object, and does not satisfy a
value-returning branch. No members beyond the universal set.

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| *(none)* | — | `Void` is a return-type boundary | implemented |

```zirk
fn log_ready(): Void {
    stdout.println("ready");
}
```

### `Never`

`Never` describes a path that cannot return normally: `fatalError`, a proven
infinite loop, an always-throwing branch. No value is delivered, so a `Never`
branch coexists with any result type. No members.

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| *(none — uninhabited)* | — | Bottom type for flow analysis | implemented |

---

**Previous:** [← String](09-string.md) · **Next:** [ Duration Has Moved](11-duration.md)
