# Algebraic Enums

An algebraic enum defines variants that may carry different typed data.

```zirk
enum LoadState {
    Idle;
    Loading(progress: Decimal);
    Ready(value: Document);
    Failed(error: LoadError);
}
```

The enum represents exactly one variant at a time. `match` narrows the variant and exposes its associated values. This models state machines without nullable fields whose valid combinations must be explained separately.

Consider the object-shaped alternative:

```zirk
record LooseLoadState {
    loading: Boolean;
    progress: Decimal?;
    document: Document?;
    error: LoadError?;
}
```

It permits contradictory states: loading and failed at once, or ready without a
document. The algebraic enum makes impossible combinations unrepresentable.

```zirk
fn render(state: LoadState): Void {
    match state {
        Idle => show_idle();
        Loading(progress) => show_progress(progress);
        Ready(document) => show_document(document);
        Failed(error) => show_error(error);
    }
}
```

This is the same model used by `Result<T, E>`, iterator steps, application
events, protocol messages, and state machines. A variant may carry zero, one,
or several typed values.

Two enum values are equal when they have the same variant and equal associated values. Equality can be derived only when every payload supports it. Ordering is never inferred from declaration order. Each `match` arm narrows its payload types.

Algebraic enums may be generic. Their variants are ordinary typed constructors.
The enum type exposes the built-in static members `keys()`, `count`, and
`to_string`; `values()`, `from_name()`, and `from_value()` apply to
traditional enums only — a payload case cannot be materialized out of
nothing. An enum cannot declare user-defined methods. Clone and equality are available only when every reachable payload
satisfies the corresponding contract. Payload extraction is legal only inside
`match`; direct enum destructuring is deliberately rejected.

## API

Variants carrying zero, one, or several typed payloads; exactly one variant at
a time. Generic algebraic enums are allowed (generic enum lowering delivered by
`fase-3-generic-enums` for flat single/multi-parameter cases).

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Universal member | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `LoadState.Ready(doc)` | `LoadState` | Variants are ordinary typed constructors | implemented |
| `value.to_string()` | `String` | Universal member | implemented |
| `value.clone()` | `LoadState` | Derived `Clone` when every payload is `Clone` | specified |

> Payload extraction is legal only inside `match`; direct enum destructuring is
> deliberately rejected. There is no `variant_name()` or `is_<variant>()`
> inspection API documented — `match` is the inspection mechanism. If one is
> wanted, `variant_name(): String` would be a consistent invented addition
> (currently `roadmap`, unproposed).

Equality holds when the variant and all associated values are equal; derivable
only when every payload supports it. Ordering is never inferred.

### Examples

```zirk
enum LoadState {
    Idle;
    Loading(progress: Decimal);
    Ready(value: Document);
    Failed(error: LoadError);
}

match state {
    Idle => show_idle(),
    Loading(progress) => show_progress(progress),
    Ready(document) => show_document(document),
    Failed(error) => show_error(error),
}
```

---

**Previous:** [← Traditional Enums](03-traditional-enums.md) · **Next:** [ Associated Values](05-associated-values.md)
