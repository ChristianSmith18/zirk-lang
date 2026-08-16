# Generic Inference

The compiler can infer type arguments from ordinary arguments and contextual result types when one unambiguous solution satisfies all constraints.

```zirk
inmut name = identity("Zirk"); // T is String
```

Inference never chooses an arbitrary type when evidence conflicts. In that case, supply type arguments or revise the API so the intended relationship is visible.

Public behavior cannot depend on which internal inference path happened to win. Diagnostics should identify the conflicting positions and unmet constraints.

---

**Previous:** [← Multiple Type Parameters](04-multiple-type-parameters.md) · **Next:** [ Specialization](06-specialization.md)
