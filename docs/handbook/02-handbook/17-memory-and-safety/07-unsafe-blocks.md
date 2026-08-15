# `unsafe` Blocks

`unsafe {}` enables specific low-level operations; it does not disable type checking, scopes, mutability, permissions, or runtime OS validation.

Every block should document the invariant it assumes and the safe contract it returns. Minimize its size so reviewers can audit the entire trust boundary.

```zirk
unsafe { write_native(pointer, length); }
```

---

**Previous:** [← Dereferencing](./06-dereferencing.md) · **Next:** [Bounds Safety →](./08-bounds-safety.md)
