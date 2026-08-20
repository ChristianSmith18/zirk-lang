# Incremental Compilation

Content-addressed caches and a fine dependency graph reuse tokens, syntax, types, IR, objects, and packages. Keys include compiler, flags, target, dependency API, IR version, and configuration so stale results cannot masquerade as valid.

An edit first invalidates its source content and syntax. Signature changes
invalidate importers and callers; body-only changes need not re-type unrelated
modules. Decorator expansion keys additionally include decorator code,
arguments, typed target, observed build inputs, requester dependency graph, and
approved permissions.

No-change builds verify cheap fingerprints and reuse validated artifacts.
Changing target, profile, compiler/IR version, lockfile integrity, public API,
native dependency, manifest configuration, or authority fingerprint invalidates
the affected layer. Cache entries are untrusted inputs: decoding and hashes are
validated before reuse, and corruption causes safe rebuild or a typed error.

`zirk check` reuses frontend layers without constructing LLVM. Cache inspection
and clean commands expose what is removed; cleaning is never required for a
correct build.

---

**Previous:** [← Optimizations](14-optimizations.md) · **Next:** [ Cross-Compilation](16-cross-compilation.md)
