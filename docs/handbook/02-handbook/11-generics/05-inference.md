# Generic Inference

The compiler can infer type arguments from ordinary arguments, the receiver,
contextual result, expected `Fn` type, and constraints when one unambiguous
solution satisfies all evidence.

```zirk
import { stdout } from std.io;

fn identity<T>(value: T): T { return value; }

fn main(): Void {
    inmut name = identity("Zirk"); // T is String
    stdout.println(name);
}
main();
```

Inference never chooses an arbitrary type when evidence conflicts. In that case, supply type arguments or revise the API so the intended relationship is visible.

Public behavior cannot depend on which internal inference path happened to win. Diagnostics should identify the conflicting positions and unmet constraints.

Explicit type arguments resolve ambiguity. Defaults apply only to trailing
parameters after inference uses available evidence: `<T = Int32>` is legal on
a type declaration, `Box<>` selects the default explicitly, and the default
is checked against the parameter's `from` constraint. Defaults on the type
parameters of a user-defined generic function support trailing defaults and
are checked at declaration and call sites. Generic function bodies, defaults,
enum dispatch, trait-default substitution, and nested contract applications
have local interpreter/native cases; their complete multi-host execution
matrix remains open. Evidence: [generic native/interpreter parity](../../../../crates/zirk-cli/tests/object_engine_parity.rs), [generic function/Clone end-to-end tests](../../../../crates/zirk-cli/tests/end_to_end.rs), and [generic semantic checks](../../../../crates/zirk-sema/tests/typing.rs). Constructors reuse the type's parameters rather than
declaring an independent generic list; use a generic factory when construction
needs new type variables.

---

**Previous:** [← Multiple Type Parameters](04-multiple-type-parameters.md) · **Next:** [ Specialization](06-specialization.md)
