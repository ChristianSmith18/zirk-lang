# Algebraic Enums

An algebraic enum defines variants that may carry different typed data.

```zirk
enum LoadState {
    Idle;
    Loading(progress: Float64);
    Ready(value: Document);
    Failed(error: LoadError);
}
```

The enum represents exactly one variant at a time. `match` narrows the variant and exposes its associated values. This models state machines without nullable fields whose valid combinations must be explained separately.

Consider the object-shaped alternative:

```zirk
record LooseLoadState {
    loading: Boolean;
    progress: Float64?;
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

---

**Previous:** [← Traditional Enums](03-traditional-enums.md) · **Next:** [ Associated Values](05-associated-values.md)
