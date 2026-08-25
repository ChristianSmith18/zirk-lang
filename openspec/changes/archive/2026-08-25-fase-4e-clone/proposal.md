## Why

`Weak<T>` (merged) proved the collector's shadow stack and header bookkeeping are sound enough to build real reference-graph features on top of. `Clone` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §6, `zirk-memory-safety` spec's "Deep clone contract" requirement) is the next piece the language actually defines but the compiler has zero trace of: today `.clone()` does not exist anywhere in the compiler, and every place the specs already *require* deep independence on a projection read (`zirk-type-system`'s "Projection copy needs Clone" scenario, `zirk-pattern-matching`'s destructuring requirement) currently has nothing to call.

**Spec correction found while scoping this**: the trait's own name is inconsistent across normative documents. `docs/01_plantilla_zirk.md` (`trait Clone<T> { fn clone(): T; }`), `CORE_LANGUAGE_SEMANTICS.md`, `MEMORY_AND_UNSAFE_SEMANTICS.md` §6, `ZIRK_LANGUAGE_SPEC.md`, `ZIRK_STDLIB_SPEC.md`, and `zirk-contracts`/`zirk-generics`/`zirk-memory-safety`/`zirk-pattern-matching`/`zirk-type-system`'s own English scenario (`zirk-type-system/spec.md:555`, "the checker requires `T from Clone`") all say **`Clone`**. Only two isolated scenario mentions say `Cloneable`: `zirk-grammar/spec.md:552`'s own parse example (`implements Auditable, Cloneable`) and `zirk-type-system/spec.md:684`'s Spanish scenario (`un objeto strict implementa Cloneable`). `Clone` is authoritative (7+ normative sources including the primary contract text itself); the two `Cloneable` mentions are corrected as part of this change, same class of fix as the `Byte`/`extern`/`Weak<T>` `Option<T>` corrections earlier this phase.

## What Changes

- New built-in trait `Clone<T>` with `fn clone(): T;`, satisfied automatically when every field of a `class`/`record`/`value class` is itself `Clone` (compiler-derived), or manually implemented otherwise.
- `clone()` on a reference graph creates new identity for every cloned reference object, preserves internal sharing inside the new graph (two fields that alias before cloning still alias after, pointing at one shared new clone — not two independent copies), reproduces cycles without infinite recursion, and never retains an alias to a cloned mutable source node. Concrete invariant: if `a.left is a.right`, then after `mut b = a.clone()`, `b.left is b.right` is true while `b.left is a.left` is false.
- Compile-time rejection when the declared graph reaches a `Resource`, `Pointer<T>`, lock, `Task`, or other non-`Clone` member — `Clone` cannot be derived and an explicit manual implementation that reaches such a member is also rejected, matching `zirk-resources`'s existing "`Resource` SHALL NOT imply `Clone`" requirement.
- Runtime: a graph-traversal clone operation with a visited/memoization map (original address → already-cloned copy address), allocated through the same collector-tracked allocation path every other managed object uses (`zirk_rt_alloc`), so a clone in progress is itself GC-safe (its partially-built graph and its memoization map are both shadow-stack roots for the duration of the operation).
- **Spec correction**: `zirk-grammar/spec.md:552` and `zirk-type-system/spec.md:684` corrected from `Cloneable` to `Clone` (see "Why") — wording fix, no behavior change; both scenarios already described `Clone` semantics under the wrong name.

### Explicitly out of scope

- **Cross-concurrency-boundary cloning** (a clone crossing a `Transfer`/`Share` derivation) — `task`/`thread` are Phase 5.
- **User-authored custom `clone()` bodies that themselves allocate non-graph resources** — the manual-implementation escape hatch exists (matching the trait's own `fn clone(): T` signature), but this change's compiler support is scoped to the compiler-derived case (every field `Clone`) plus correctly executing whatever a manual implementation's body already does; it does not add new compiler machinery for a manual implementation beyond normal method compilation.
- **Cloning through a `NativeSlice<T>`/`Pointer<T>`-typed field** — such a field makes the whole graph non-`Clone` by this change's own compile-time rejection rule; native-view-aware cloning is not designed here.
- **Shallow copy / `Object.clone()`** — `MEMORY_AND_UNSAFE_SEMANTICS.md` explicitly says there is no universal `Object.clone()`; only types implementing `Clone` support cloning at all.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-memory-safety`: its existing "Deep clone contract" requirement text is reaffirmed verbatim (already correct) and delivered by this change.
- `zirk-type-system`: the "Generic projection needs Clone" scenario is reaffirmed; the Spanish `inmut::strict` scenario at line 684 is corrected `Cloneable` → `Clone` (wording fix, see "Why").
- `zirk-grammar`: the parse scenario at line 552 is corrected `Cloneable` → `Clone`.
- `zirk-object-memory`: the collector-tracked graph-clone traversal (memoization map as a GC root for the operation's duration) has no prior requirement — added, following the same pattern `fase-4e-weak` added its weak-clearing-pass requirement.

## Impact

- Affected code: `crates/zirk-sema` (`Clone` trait recognition, derivation eligibility check over field types, compile-time rejection of non-`Clone` members reached transitively), `crates/zirk-ir` (a graph-clone IR shape: allocate-with-memoization-lookup, recurse per field), `crates/zirk-codegen-llvm` (lowering those instructions, memoization map as a runtime call into `zirk-runtime`), `crates/zirk-runtime` (the memoization map itself — an address-keyed table scoped to one `clone()` call, collector-tracked for its duration).
- Public documentation: `docs/init/ZIRK_ROADMAP.md` Phase 4e's "deep clone graph semantics" bullet gets a delivery note; `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md` memory rows updated (`deep clone()` moves from "remaining" to delivered). `../zirk-lang-site` sync required per project convention once this lands.
- No breaking changes: `Clone` is a new trait; nothing that compiles today changes behavior. The `Cloneable`→`Clone` spec wording fix corrects two scenarios that never had compiler backing under either name.
