# LLVM Backend

LLVM is Zirk 1.x's only backend. It lowers portable IR to target objects while preserving controlled errors, identity, safety, and debug mappings. An internal backend boundary permits future work without changing language semantics.

## Lowering boundary

The backend receives verified typed IR, an explicit target, build profile, and
resolved native inputs. It selects LLVM integer/float widths, layouts, calling
conventions, symbols, and object format without inventing source semantics.
Runtime checks for overflow, invalid shifts/casts, bounds, and forbidden NaN
results remain observable unless an earlier proof removes them safely.

Emission produces an object first. Linking is a separate step that combines the
object with the target runtime, standard library, native dependencies, SDK, and
system linker. Diagnostics distinguish emission failure, missing toolchain,
incompatible object, unresolved symbol, and linker failure.

## Debug and release

Debug lowering favors source correspondence and complete metadata. Release may
inline, specialize, devirtualize, eliminate dead code, vectorize, and use LTO,
but cannot change language guarantees. Portable intrinsics and SIMD select a
target instruction when available and a semantically equivalent fallback
otherwise. Textual inline assembly is outside Zirk 1.x.

> **Implementation status:** LLVM 20 object emission and native linking exist
> for the implemented subset. Later language/runtime features require their own
> verified IR and backend paths before being marked delivered.

---

**Previous:** [← Portable IR](05-portable-ir.md) · **Next:** [ Diagnostics](07-diagnostics.md)
