# Monomorphization

Monomorphization produces target-specific implementations for concrete generic instantiations. Zirk's portable typed IR retains enough information for this work during the final application build.

This enables optimization across application, package, standard-library, and runtime code aligned to the same target. It may increase binary size, so the compiler can share code where doing so preserves ABI and semantics.

Monomorphization is not source-level copying and does not change type checking. Packages distribute portable IR rather than promising one machine-specific generic implementation for every consumer.

Generic runtime identity is never erased: different type arguments remain
distinguishable to checked casts and retained reflection even when equivalent
machine code is safely shared.

---

**Previous:** [← Specialization](06-specialization.md) · **Next:** [Variance, Recursion, and Runtime Identity →](08-variance-recursion-and-runtime.md)
