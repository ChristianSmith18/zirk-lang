# Dereferencing

Dereferencing reads or writes the pointee after the unsafe code has established validity, alignment, lifetime, and mutation permission.

An invalid pointer must not be converted into silent undefined behavior in safe code. Keep dereference scopes small and convert native results into safe Zirk values immediately.

`read()`, `write(value)`, and dereference syntax are unsafe. Volatile variants
exist for device or externally observed memory and require an irreversible
`commit` boundary; volatile access is not synchronization. A validated mutable
native slice can journal its bounded range for rollback, while an unproven raw
write cannot.

---

**Previous:** [← Pointers](05-pointers.md) · **Next:** [ unsafe Blocks](07-unsafe-blocks.md)
