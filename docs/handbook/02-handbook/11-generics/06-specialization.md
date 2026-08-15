# Specialization

Specialization lets the compiler exploit facts about a concrete type while preserving the generic contract. It may select direct calls, inline operations, or optimize representation.

Source code must not observe a different semantic result merely because an instantiation is specialized. Constraints, errors, permissions, cancellation, and safety guarantees remain identical.

The initial language does not promise user-written overlapping specialization rules. Treat specialization primarily as a compiler optimization unless a future normative change adds source-level control.

---

**Previous:** [← Generic Inference](./05-inference.md) · **Next:** [Monomorphization →](./07-monomorphization.md)
