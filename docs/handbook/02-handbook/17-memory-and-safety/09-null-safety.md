# Null Safety

Only `T?` can contain `null`. Flow analysis, `?.`, `??`, and matching establish a non-null value before ordinary access.

A null failure remains controlled and diagnostic; safe code cannot turn it into arbitrary memory access. Raw native pointers inside `unsafe` require their own validity checks.

---

**Previous:** [← Bounds Safety](./08-bounds-safety.md) · **Next:** [Use-After-Free Prevention →](./10-use-after-free.md)
