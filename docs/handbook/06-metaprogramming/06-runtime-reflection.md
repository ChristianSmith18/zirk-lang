# Runtime Reflection

Decorators do not create general runtime reflection. Zirk 1.x has no `runtime fn dec` and no `Reflection.decorators(target)` API. After expansion, decorator declarations, identities, arguments, and applications are erased.

A library requiring runtime structure generates an ordinary typed descriptor or registry during `Augment`. An HTTP framework can generate route records; a serializer can generate a `TypeDescriptor(User)` containing only required fields; an injector can generate typed factories. These values obey ordinary visibility, permissions, documentation, compatibility, and dead-code elimination.

Basic type identity defined elsewhere in the language does not imply retained decorator metadata or arbitrary structural access.

---

**Previous:** [← Compile-Time Reflection](05-compile-time-reflection.md) · **Next:** [ Decorator Erasure and Generated Descriptors](07-reflection-retention.md)
