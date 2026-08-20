# Why No Inline Assembly?

Textual inline assembly is excluded from Zirk 1.x because it couples source to one assembler, ABI, register model, optimizer contract, and architecture. Portable intrinsics and SIMD keep operations typed and allow safe fallbacks.

When assembly is unavoidable, place it in an audited external native library behind a C ABI wrapper.

Inline assembly would also bypass typed provenance, transaction rollback,
permission effect inference, register/stack validation, sanitizer-friendly
lowering, portable packages, and reproducible target analysis. Marking the
block `unsafe` would acknowledge risk but would not provide enough information
for the compiler to preserve surrounding code correctly.

The supported escape route is deliberately explicit:

1. implement and test the operation in a target-specific native artifact;
2. export a narrow C ABI symbol;
3. declare target, integrity, license, and permission metadata;
4. write a small unsafe binding and a safe typed wrapper;
5. provide a portable fallback or an early unsupported-target diagnostic.

This keeps unavoidable assembly auditable without making its textual syntax a
permanent part of Zirk 1.x.

---

**Previous:** [← Automatic Vectorization](09-auto-vectorization.md) · **Next:** [ Metaprogramming](../06-metaprogramming/README.md)
