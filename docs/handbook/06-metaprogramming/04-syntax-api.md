# Syntax API

The public Syntax API is immutable, versioned, and validated. A target block uses `match target.transform` in the fixed order `Inspect -> Augment -> Wrap`.

- `Inspect(context)` validates typed structure and emits diagnostics without committing mutations.
- `Augment(builder)` adds compatible declarations or contracts with target-specific hygienic builders.
- `Wrap(wrapper)` wraps a callable with a body.

Within `Wrap`, `match target.wrap` exposes `Before()`, `After(result)`, `After(result, transform)`, `Catch(error)`, or exclusive `Around(next)`. `After(result)` observes; its second form can invoke the non-escaping one-shot `transform` capability. `Void` uses `After()`. `Catch` may recover or rethrow but cannot hide unhandled errors. `_` consumes an explicit payload position and variant arity remains exact.

Wrappers preserve callable identity, signature, generics, receiver mutability, override slot, documentation, and source maps. New error and permission effects remain visible. Builders cannot delete user API, reduce visibility, silently rename declarations, weaken safety, or create public conflicts. Private generated names are hygienic; public collisions are errors.

---

**Previous:** [← Decorator Targets](03-decorator-targets.md) · **Next:** [ Compile-Time Reflection](05-compile-time-reflection.md)
