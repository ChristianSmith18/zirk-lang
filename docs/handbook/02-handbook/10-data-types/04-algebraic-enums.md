# Algebraic Enums

An algebraic enum defines variants that may carry different typed data.

```zirk
enum LoadState {
    Idle;
    Loading(progress: Decimal);
    Ready(value: String);
    Failed(error: String);
}
```

The enum represents exactly one variant at a time. `match` narrows the variant and exposes its associated values. This models state machines without nullable fields whose valid combinations must be explained separately.

A dynamic `Object` can model an open payload shape, but it does not enforce
that only one state is present:

```zirk
import { stdout } from std.io;

fn main(): Void {
    inmut document = "report contents";
    inmut state: Object = {
        loading: false,
        progress: "none",
        document: document,
        error: "none"
    };
    stdout.println("dynamic object fields can form contradictory states");
}
main();
```

It permits contradictory states: loading and failed at once, or ready without a
document. The algebraic enum makes impossible combinations unrepresentable.

```zirk
import { stdout } from std.io;

enum LoadState {
    Idle;
    Loading(progress: Decimal);
    Ready(value: String);
    Failed(error: String);
}

fn render(state: LoadState): String {
    return match state {
        LoadState.Idle => "idle",
        LoadState.Loading(progress) => "loading {progress}",
        LoadState.Ready(document) => "ready: {document}",
        LoadState.Failed(error) => "failed: {error}"
    };
}

fn main(): Void {
    stdout.println(render(LoadState.Ready("report contents")));
}
main();
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
a time. Generic algebraic enums support construction and dispatch across
distinct and nested instantiations in local interpreter/native cases. The
named multi-host execution cases passed in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962); see the
[feature-status matrix](../../11-reference/12-feature-status.md).

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Universal member | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `LoadState.Ready(doc)` | `LoadState` | Variants are ordinary typed constructors | implemented |
| `value.to_string()` | `String` | Universal member | implemented |
| `value.clone()` | `LoadState` | Derived `Clone` when every payload is `Clone` | implemented |

> Payload extraction is legal only inside `match`; direct enum destructuring is
> deliberately rejected. There is no `variant_name()` or `is_<variant>()`
> inspection API documented — `match` is the inspection mechanism. If one is
> wanted, `variant_name(): String` would be a consistent invented addition
> (currently `roadmap`, unproposed).

Equality holds when the variant and all associated values are equal; derivable
only when every payload supports it. Ordering is never inferred. Executable
evidence: [enum and union native/interpreter parity](../../../../crates/zirk-cli/tests/object_engine_parity.rs), [Clone composition tests](../../../../crates/zirk-cli/tests/end_to_end.rs), and [enum semantic checks](../../../../crates/zirk-sema/tests/typing.rs).

### Examples

---

**Previous:** [← Traditional Enums](03-traditional-enums.md) · **Next:** [ Associated Values](05-associated-values.md)
