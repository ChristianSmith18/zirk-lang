# Stack and Heap

Stack and heap placement are compiler decisions informed by escape analysis, size, identity, and target constraints. Source syntax does not choose a region merely by constructing a class or value.

Native ABI and unsafe code may expose layout obligations, but ordinary code should reason from type and lifetime contracts. Optimizations may move storage while preserving behavior.

---

**Previous:** [← Memory Model](01-memory-model.md) · **Next:** [ Automatic Memory Management](03-automatic-memory-management.md)
