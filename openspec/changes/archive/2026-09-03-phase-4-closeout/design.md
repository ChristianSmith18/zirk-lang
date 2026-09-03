## Context

Phase 4 is the last "single-threaded" foundation before concurrency. The pieces already delivered are `Result<T,E>` (4a), explicit and typed exceptions (4b partial), single-resource `match with` (4c partial), escaping single-capturing closures (4d partial), and the core unsafe/memory toolkit (4e partial). What remains is the cross-cutting completion that lets the compiler support the more advanced features of Phase 5:

- a fully catchable native-failure set and finished exception metadata;
- grouped/transferable resources with dependent lifetimes;
- uniform boxed callables;
- dependent references, pinning, and the cleanup interactions between them.

This change treats those items as one coordinated closeout. Where possible it reuses the existing machinery: the mark-sweep runtime, the `RuntimeError` hierarchy, the `unsafe` journal, the `Fn(P...) => R` type, and the vtable/contract dispatch paths. New work is added only where the existing surfaces are genuinely incomplete.

## Goals / Non-Goals

**Goals:**

1. Make `arithmetic overflow` and `invalid cast` catchable `RuntimeError` subclasses under `try`/`catch`.
2. Add suppressed-failure lists, lazy stack-trace materialization, and deep `Throwable` immutability to the exception runtime.
3. Implement grouped `match with` resource acquisition, surfaced/merged close failures, `TransferableResource` transfer, cancellation-aware cleanup, and dependent-resource lifetime analysis.
4. Deliver general callable-type polymorphism (D13) with a boxed capture representation.
5. Implement `Dependent<T>` references and automatic bounded native pinning (`Pin<T>`).
6. Complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls.
7. Update the roadmap, feature-status, and handbook to mark Phase 4 as complete.

**Non-Goals:**

- Phase 5 concurrency primitives (`Task`, `Channel`, `Atomic`, `Mutex`, etc.).
- `Array<T>`/`List<T>`/`Map`/`Set` collections (Phase 7).
- Generators, pipe, and functional combinators (Phase 7b).
- Self-hosting or package management (Phases 8–12).
- New syntax unrelated to the listed features (existing keywords already cover `Dependent`, `Pin`, `transfer`, etc.; if new surface is needed, it is gated inside this change).

## Decisions

### 1. Catchable overflow and invalid cast

**Decision:** Treat signed/unsigned arithmetic overflow and `as`-casts to an incompatible runtime type as `RuntimeError` subclasses thrown into the nearest `try`/`catch`, using the same lowering pattern as the four already-catchable native failures.

**Rationale:** The runtime already has `throw_native_failure` and the `Throwable` hierarchy; the only missing piece is branching to it from the codegen overflow/cast checks instead of calling `fatalError`.

**Alternatives considered:**
- Use `Result` for these checks. Rejected because the language's native safety model is exception-based, and `Result` requires explicit propagation.
- Keep them as aborts. Rejected because the roadmap explicitly lists them as remaining Phase 4b work.

### 2. Suppressed failures, stack traces, and throwable immutability

**Decision:**
- Add a `suppressed: List<Throwable>` field to the runtime `Throwable` object, populated during cleanup unwinding.
- Materialize stack traces lazily: record the program counter / function id at throw time, and resolve symbolic frames only when `error.stack()` is first read.
- Make thrown objects deep-immutable by marking the allocation with a frozen bit and rejecting any write through a reference reachable from the throwable.

**Rationale:** These are the final pieces of the `Throwable` contract. Lazy traces avoid paying for frame-string construction on every throw; a frozen bit reuses the existing object header space.

**Alternatives considered:**
- Materialize traces eagerly. Rejected because exception-heavy paths (e.g., parser backtracking) would be penalized.
- Per-frame string capture at throw. Rejected because the runtime does not yet keep a debug symbol table; resolving on read lets us improve symbol quality later.

### 3. Grouped resource acquisition and close-failure composition

**Decision:**
- Lower `match r1 with ..., r2 with ...` to a left-to-right acquisition sequence; on failure, close every already-acquired resource right-to-left before entering the error branch.
- Represent a combined body/close failure as a `ResourceFailure<BodyError, CloseError>` enum with `Body`, `Close`, and `BodyAndClose` variants.

**Rationale:** This is exactly the `zirk-resources` spec already written; the compiler's existing `match with` lowering only handles one resource. Grouped acquisition is a straightforward generalization of the same cleanup list.

**Alternatives considered:**
- Acquire in parallel. Rejected because cancellation and deterministic failure ordering require sequential acquisition first.

### 4. Resource transfer and dependency analysis

**Decision:**
- `transfer(r)` lowers to a new `ResourceTransfer` IR instruction that invalidates the source slot and returns the same resource object in a fresh, owned slot.
- Dependent resources are checked by tracking a parent slot in the resource descriptor; the compiler rejects any use that can outlive the parent (return, field store, closure capture, task spawn).

**Rationale:** A dedicated IR instruction keeps the ownership transfer explicit and lets the runtime set a `moved` flag for debug builds.

**Alternatives considered:**
- Purely static tracking with no runtime flag. Rejected because `use after transfer` should produce a clean runtime failure, not undefined behavior.

### 5. General callable polymorphism (D13)

**Decision:**
- Any `Fn(P...) => R` value that may hold differently-captured closures is represented as a two-word `{function pointer, capture-block pointer}` pair.
- The capture block is a heap-allocated, GC-tracked object with a descriptor that describes the captured slots.
- Named functions and capture-less lambdas are promoted to the same representation at the boundary (a static capture block with no slots).

**Rationale:** This is the only known design that lets the same variable hold arbitrary captured closures without monomorphizing by capture set.

**Alternatives considered:**
- Monomorphize by capture set. Rejected because it explodes call sites and cannot support returned/stored closures.
- Trampoline with a typed environment pointer. Rejected because it would require a runtime type tag on every call.

### 6. `Dependent<T>` references

**Decision:**
- A `Dependent<T>` is a two-word reference `{object pointer, base pointer}` where the base pointer is the GC-managed object the dependent value lives inside.
- The GC treats `Dependent<T>` as a strong edge to the base object, and the base object keeps the dependent object alive.
- A dependent reference is invalidated when the base object is mutated in a way that could move or deallocate the dependent storage.

**Rationale:** This matches the spec's intent without requiring a full region or lifetime system; it reuses the existing GC and object-layout machinery.

**Alternatives considered:**
- Pure compile-time lifetime tracking. Rejected because it requires a borrow checker the project has not adopted.

### 7. Automatic bounded native pinning (`Pin<T>`)

**Decision:**
- `Pin<T>` wraps a `T` and, for the duration of the pin, prevents the GC from moving the underlying object and prevents any operation that could invalidate interior pointers.
- For native slices and `Pointer<T>`, `Pin<T>` is created automatically by the checker when an interior address is exposed, and is bounded by the lifetime of the enclosing `unsafe`/`commit` block.
- The runtime records pinned objects in a per-thread list and removes them at the corresponding unwind/commit/rollback point.

**Rationale:** Pinning is required for safe native interop; integrating it with `unsafe` journals and `commit` gives us bounded, deterministic unpinning for free.

**Alternatives considered:**
- Manual `pin()`/`unpin()` only. Rejected because the language's safety goal is automatic bounded pinning.

### 8. `inmut::strict` completion

**Decision:**
- Extend the existing strict-alias analysis from local/parameter rebindings and field projections to field declarations themselves and to mutating method calls reached through a strict reference.
- A `mut` method call on an `inmut::strict` receiver is rejected; an `inmut::strict` field declaration makes the field unwritable through any projection.

**Rationale:** These are the two remaining gaps noted in `ZIRK_ROADMAP.md` for the `inmut::strict` feature.

## Risks / Trade-offs

- `[Risk]` Combining four large sub-phases in one change may create a diff that is hard to review and prone to regressions. `Mitigation`: keep each sub-feature on independent task groups, commit per group, and run the full test suite after each group lands.
- `[Risk]` The boxed callable representation introduces a new allocation on every capture. `Mitigation`: reuse the existing `clone()` heap-boxing path and optimize later with small-capture inlining once Phase 5 is stable.
- `[Risk]` `Dependent<T>` and `Pin<T>` interact with the GC shadow stack and `unsafe` journals in subtle ways. `Mitigation`: add dedicated runtime tests for "dependent base collected during pin", "pin released on journal rollback", and "dependent invalidated on base mutation".
- `[Risk]` Catchable overflow/cast may change the observable behavior of programs that previously aborted. `Mitigation`: document this in the change and handbook as an intended semantic change; existing tests that expect abort must be updated to expect exceptions.
- `[Risk]` Resource transfer invalidation needs runtime support that does not exist yet. `Mitigation`: implement the runtime `Resource` object header and `moved` flag first, before the checker rules.

## Open Questions

1. Should `Pin<T>` be a distinct type constructor or an attribute on `T`? A distinct type keeps `Pin<Pointer<T>>` explicit, but an attribute is closer to the existing `inmut::strict` design.
2. Should `Dependent<T>` support only `class` bases, or also `record`/`value class`? Value bases have no stable identity; likely only `class` and `Array`/`List` (Phase 7).
3. Does `RuntimeError` catchability for overflow/cast need a compiler flag to preserve abort semantics for debug builds? Probably not, but this should be confirmed before merging.
4. What is the canonical runtime layout of the boxed callable's capture descriptor? This will be decided once the IR design is validated.
