## Why

Deep `clone()` is a core part of the object model: it is how Zirk code produces an independent copy of a mutable object graph, and the handbook chapter `11-cloning.md` teaches it as "the explicit escape from shared-reference behavior". In the browser interpreter `InstKind::Clone` is classified as excluded, so any program that calls `.clone()` on a class instance fails preflight even though the operation is pure heap computation that needs no host authority. This blocks teaching aliasing versus copying, one of the most common object-oriented learning moments, in any browser-hosted surface.

## What Changes

- Implement `InstKind::Clone` in `zirk-interpreter` as a deep copy over the interpreter's own bounded heap, with the semantics already specified by `zirk-memory-safety` ("Deep clone graph semantics"): new identity for every cloned reference object, preserved internal sharing, and cycle-safe traversal that mirrors the source graph.
- Reclassify `Clone` from excluded to browser-supported in the support matrix once fixtures pass, and keep `WeakFrom`, `WeakUpgrade`, `WeakIsAlive`, pointers, native slices and journals excluded.
- Charge clone work against the existing instruction budget and heap limit so a large or adversarial graph ends in a deterministic `limit-exceeded` result instead of unbounded work.
- Cover `Callable` clones (the native runtime exposes `zirk_rt_clone_callable`) and `record` values, which the lowering already rebuilds structurally.
- Add differential fixtures for tree, shared-child, cyclic, subclass-polymorphic, collection-holding and callable graphs.
- No source-language change, no new host effect, no `apiVersion` change.

## Capabilities

### New Capabilities

### Modified Capabilities

- `zirk-ir-interpreter`: adds a requirement that deep `clone()` executes in the interpreter with the specified graph semantics under the existing resource limits.

## Impact

- `crates/zirk-interpreter/src/lib.rs`: a `Clone` execution arm and a graph-copy routine over the arena heap; `InstKind::Clone` classification in the browser-support function and preflight.
- `crates/zirk-runtime-core` only if a pure helper for graph traversal is worth sharing with the native `crates/zirk-runtime/src/clone.rs`; sharing is optional and decided in design.
- `crates/zirk-browser/fixtures/`: new clone fixtures run through both engines.
- `docs/browser-interpreter-support-matrix.md`, `docs/init/ZIRK_FEATURE_STATUS.md`: status updates. This change affects implementation status, so public content changes.
- Companion repository `../zirk-lang-site` is impacted only through the rebuilt browser package and reviewed status wording; follow the sync procedure in the repository configuration.
- Sibling change `browser-oop-conformance` owns the broader object-model audit; this change can land before or after it, and the audit will pick up the clone fixtures.
