# Circular Dependencies

Module dependency cycles make initialization, name resolution, incremental invalidation, and API ownership harder to reason about. The compiler must diagnose unsupported cycles with the complete dependency path.

Break a cycle by extracting a shared contract, reversing a dependency through an interface, or moving orchestration outward. Do not hide the cycle with global state or runtime lookup.

> **Specification status:** The final documents require deterministic module resolution but do not yet define every legal type-only cycle. Treat unconfirmed cycles as unsupported.

---

**Previous:** [← Public API](06-public-api.md) · **Next:** [ Projects](../../03-projects/README.md)
