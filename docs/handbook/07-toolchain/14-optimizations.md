# Optimizations

Optimization may inline, specialize, devirtualize, remove checks proven redundant, vectorize, and improve layout. It cannot change overflow, equality, identity, cancellation, resource closure, permissions, or error semantics.

Permitted transformations include constant folding under Zirk's checked
numeric rules, dead-code elimination, generic specialization, escape analysis,
stack promotion, devirtualization, inlining, loop/vector optimization, and
eliding bounds or unsafe journals after proof. Reference identity prevents an
allocation or clone from being merged when `is` could observe the difference.

Task ordering, fair selection, cancellation cleanup, close-error composition,
volatile/native effects, and `commit` boundaries restrict reordering. Permission
checks may use a validated fingerprint fast path but cannot be optimized away
across authority changes.

Every optimization has IR verification and debug/release differential tests.
Performance improvements that cannot prove semantic equivalence remain disabled
or require an explicitly unsafe API.

---

**Previous:** [← Debug and Release](13-debug-and-release.md) · **Next:** [ Incremental Compilation](15-incremental-compilation.md)
