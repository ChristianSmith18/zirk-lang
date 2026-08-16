# Dereferencing

Dereferencing reads or writes the pointee after the unsafe code has established validity, alignment, lifetime, and mutation permission.

An invalid pointer must not be converted into silent undefined behavior in safe code. Keep dereference scopes small and convert native results into safe Zirk values immediately.

---

**Previous:** [← Pointers](05-pointers.md) · **Next:** [ unsafe Blocks](07-unsafe-blocks.md)
