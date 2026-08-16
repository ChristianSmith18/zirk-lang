# Generic Inference

The compiler can infer type arguments from ordinary arguments, the receiver,
contextual result, expected `Fn` type, and constraints when one unambiguous
solution satisfies all evidence.

```zirk
inmut name = identity("Zirk"); // T is String
```

Inference never chooses an arbitrary type when evidence conflicts. In that case, supply type arguments or revise the API so the intended relationship is visible.

Public behavior cannot depend on which internal inference path happened to win. Diagnostics should identify the conflicting positions and unmet constraints.

Explicit type arguments resolve ambiguity. Defaults apply only to trailing
parameters after inference uses available evidence. Constructors reuse the
type's parameters rather than declaring an independent generic list; use a
generic factory when construction needs new type variables.

---

**Previous:** [← Multiple Type Parameters](04-multiple-type-parameters.md) · **Next:** [ Specialization](06-specialization.md)
