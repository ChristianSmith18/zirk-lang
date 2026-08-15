# Exporting Zirk

An exported function exposes a stable C-compatible symbol and signature. It cannot leak Zirk-specific generics, exceptions, object layout, or automatic-memory internals across the boundary.

Catch exceptions before crossing into C, translate failure to the declared ABI, and document who owns every returned buffer or handle.

---

**Previous:** [← Importing C](./02-importing-c.md) · **Next:** [Dynamic Libraries →](./04-dynamic-libraries.md)
