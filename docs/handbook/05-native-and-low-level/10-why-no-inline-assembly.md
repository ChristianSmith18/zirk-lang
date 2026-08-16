# Why No Inline Assembly?

Textual inline assembly is excluded from Zirk 1.x because it couples source to one assembler, ABI, register model, optimizer contract, and architecture. Portable intrinsics and SIMD keep operations typed and allow safe fallbacks.

When assembly is unavoidable, place it in an audited external native library behind a C ABI wrapper.

---

**Previous:** [← Automatic Vectorization](09-auto-vectorization.md) · **Next:** [ Metaprogramming](../06-metaprogramming/README.md)
