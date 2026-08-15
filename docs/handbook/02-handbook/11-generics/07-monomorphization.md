# Monomorphization

Monomorphization produces target-specific implementations for concrete generic instantiations. Zirk's portable typed IR retains enough information for this work during the final application build.

This enables optimization across application, package, standard-library, and runtime code aligned to the same target. It may increase binary size, so the compiler can share code where doing so preserves ABI and semantics.

Monomorphization is not source-level copying and does not change type checking. Packages distribute portable IR rather than promising one machine-specific generic implementation for every consumer.

---

**Previous:** [← Specialization](./06-specialization.md) · **Next:** Data Types and Collections *(next handbook units)*
