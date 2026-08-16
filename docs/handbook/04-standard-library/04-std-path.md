# `std.path`

`Path` models platform path semantics rather than a raw `String`. It provides join, lexical normalization, components, name, extension, parent, and explicit string conversion.

Construction and lexical operations do not access the filesystem. Absolute/canonical resolution may access it and requires permission. Use `Path` to avoid unsafe textual concatenation.

---

**Previous:** [← std.fs](03-std-fs.md) · **Next:** [ std.process](05-std-process.md)
