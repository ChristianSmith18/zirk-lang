# Portable SIMD

Portable vector types describe lane types and operations while the backend selects target instructions. When hardware lacks an equivalent, the implementation provides a safe fallback or diagnoses unsupported explicit requirements.

Bounds, alignment, overflow, and floating-point behavior remain part of the typed contract.

---

**Previous:** [← Intrinsics](./07-intrinsics.md) · **Next:** [Automatic Vectorization →](./09-auto-vectorization.md)
