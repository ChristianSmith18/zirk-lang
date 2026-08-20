# Portable SIMD

Portable vector types describe lane types and operations while the backend selects target instructions. When hardware lacks an equivalent, the implementation provides a safe fallback or diagnoses unsupported explicit requirements.

Bounds, alignment, overflow, and floating-point behavior remain part of the typed contract.

Vector types have a fixed lane count and scalar lane type. Construction,
arithmetic, comparison, masks, selection, load/store, and reductions are typed;
mixed lane widths require explicit conversion. Checked integer and no-NaN float
semantics remain Zirk semantics even when the hardware instruction needs an
additional check.

Safe loads/stores use arrays or validated native views and verify bounds.
Unaligned access is explicit when supported. Gather/scatter, raw pointers, and
target-specific features require stronger contracts and may be unsafe.

Portable code asks for the operation, not `AVX`/`NEON` directly. The backend
uses target features selected for the build and emits an equivalent scalar or
lower-width fallback. Benchmarks decide whether explicit SIMD is worthwhile;
ordinary loops remain the preferred source when automatic vectorization can
prove the same result.

---

**Previous:** [← Intrinsics](07-intrinsics.md) · **Next:** [ Automatic Vectorization](09-auto-vectorization.md)
