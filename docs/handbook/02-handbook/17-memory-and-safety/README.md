# Memory and Safety

Zirk manages memory automatically while safe code guarantees no use-after-free,
double-free, uncontrolled null dereference, data race, or undefined behavior.
This unit separates four levels clearly: ordinary managed references, dependent
native views, reversible unsafe mutation, and irreversible native effects.

Read it in order. The full normative contract lives in
[Memory and Unsafe Semantics](../../../MEMORY_AND_UNSAFE_SEMANTICS.md).

---

**Previous:** [← Why No General defer or Destructors?](../16-resources/06-why-no-defer-or-destructors.md) · **Next:** [ Memory Model](01-memory-model.md)
