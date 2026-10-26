# ABI and IR Compatibility

IR version governs portable package consumption; target ABI governs final linkage and native layout. Public API compatibility is typed and distinct from either. Builds reject mismatched compiler IR, target, architecture, or native ABI explicitly.

Three compatibility layers are checked independently:

1. **Source/public API:** names, visibility, types, generic constraints,
   callable effects, decorators that generate public API, and documented
   compatibility policy.
2. **Portable IR:** serialization version and semantic feature set understood by
   the consuming compiler.
3. **Native ABI:** target triple, widths/alignment, calling convention, symbol
   names, runtime/stdlib ABI, and native library versions.

A compatible public API does not make old portable IR readable, and readable
IR does not make a native object linkable on another target. `.zpkg` retains
portable implementation and public API; final objects are rebuilt for the
application target. C ABI exports/imports use their explicitly documented
layout and ownership boundary rather than Zirk's private native ABI.

Compatibility errors identify the layer, producer/consumer versions, affected
package, and supported remediation such as rebuilding or selecting a compatible
dependency.

Union layouts use versioned portable metadata and a canonical logical-layout
fingerprint. The validator rejects unsupported package/IR schema versions and
fingerprint mismatches with a rebuild diagnostic. Its encoder and validator
have crate-level round-trip tests. For local source dependencies, package
builds emit a versioned union ABI metadata sidecar under `build/`. A consuming
build reads that independently built metadata and validates imported layouts
before object emission; stale fingerprints fail with rebuild guidance.
Source-only dependencies without a sidecar are validated from the verified
compilation graph. This closes the local compiled-package metadata check, but
it does not introduce a portable `.zpkg` loader or serializer. See
[Portable IR](../09-packages/03-portable-ir.md) for the exact metadata
contract.

---

**Previous:** [← Cross-Compilation](16-cross-compilation.md) · **Next:** [ Performance Goals](18-performance-goals.md)
