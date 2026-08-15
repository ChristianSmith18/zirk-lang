# Automatic Vectorization

LLVM may vectorize loops when dependency and alias analysis prove it preserves semantics. Release optimization cannot weaken bounds, overflow, equality, ordering, or error guarantees.

Measure before rewriting code around target details. Portable source plus representative benchmarks is the default.

---

**Previous:** [← Portable SIMD](./08-portable-simd.md) · **Next:** [Why No Inline Assembly? →](./10-why-no-inline-assembly.md)
