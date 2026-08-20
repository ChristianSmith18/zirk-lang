# Intrinsics

Intrinsics expose carefully specified low-level operations independent of textual assembly. Each intrinsic defines supported types, target availability, safety, fallback, and observable behavior.

Use them only when ordinary code cannot express the requirement; compiler optimization is preferable when semantics are equivalent.

An intrinsic is a compiler-known typed function, not arbitrary backend text.
Its contract states accepted widths/lane types, result, overflow and floating
behavior, memory ordering/provenance, target feature requirements, and whether
it is safe or requires `unsafe`.

Portable intrinsics such as bit counts or byte swap produce the same result on
every target. LLVM may select one instruction or a sequence. A target-specific
intrinsic requires an explicit target/feature condition and fails during
preparation when unavailable; it is never silently approximated with different
semantics.

Intrinsics that touch volatile/device memory, weak atomics, or raw addresses
also require the matching unsafe/commit and permission boundaries. Compiler
tests compare constant folding and native lowering against the ordinary
specified behavior.

---

**Previous:** [← Native Permissions](06-native-permissions.md) · **Next:** [ Portable SIMD](08-portable-simd.md)
