# Debug and Release

Debug prioritizes complete symbols, minimal optimization, and source correspondence. Release enables stronger optimization, dead-code removal, LTO, vectorization, and size work without changing language guarantees.

Profiles affect compilation strategy and observability, not arithmetic,
bounds, permissions, cancellation, cleanup, equality, or exception semantics.
A program cannot rely on debug-only overflow or race behavior.

Debug artifacts retain source maps, local/type metadata, expansion provenance,
and assertions needed for development. Release artifacts default to optimized
native code, dead-code elimination, target tuning allowed by the selected
target, and optional LTO. Distribution builds additionally require locked
dependencies and reproducibility metadata.

Profile values participate in cache keys. Mixing release objects with a debug
runtime or incompatible instrumentation is diagnosed before linkage.

---

**Previous:** [← Debugger](12-debugger.md) · **Next:** [ Optimizations](14-optimizations.md)
