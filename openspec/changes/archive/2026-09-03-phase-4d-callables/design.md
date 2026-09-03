## Context

Phase 4d is the last remaining callable-related gate in the single-threaded foundation. The language already has non-capturing and single-capturing closures, but it does not yet support general callable polymorphism: the same `Fn(P...) => R` binding cannot hold closures with different capture sets, and escaping captures are not boxed. This change scopes `fase-4-cierre-completo` to only those callable issues, leaving 4b/4c/4e work for separate changes.

## Goals / Non-Goals

**Goals:**

1. Deliver general callable-type polymorphism (D13) with a uniform boxed capture representation.
2. Define `MakeCallable` and `CallCallable` IR instructions.
3. Allow a `Fn(P...) => R` binding to accept closures with different capture sets when parameter/result types match.
4. Lower escaping or stored captured closures to a two-word `{function pointer, capture-block pointer}` value.
5. Support `.clone()` on callables when all captures are `Clone`.
6. Update escape and alias analysis to treat stored/returned `Fn(...)` values as heap-escaping.
7. Add tests for polymorphic assignment, returned/stored closures, and cloning.

**Non-Goals:**

- Exceptions, resources, dependent references, pinning, or `inmut::strict` (other Phase 4 sub-features).
- Phase 5 concurrency primitives (`Task`, `Channel`, `Atomic`, `Mutex`, etc.).
- New collection types (`Array<T>`, `List<T>`, `Map`, `Set`).
- Generators, pipes, or functional combinators.
- New syntax beyond what existing keywords already provide.

## Decisions

### 1. Boxed callable representation

**Decision:** Any `Fn(P...) => R` value that may hold differently-captured closures is represented as a two-word `{function pointer, capture-block pointer}` pair. The capture block is a heap-allocated, GC-tracked object with a descriptor that lists the captured slots and their mutability. Named functions and capture-less lambdas are promoted to the same representation at the boundary (a static capture block with no slots).

**Rationale:** This is the only known design that lets the same variable hold arbitrary captured closures without monomorphizing by capture set.

**Alternatives considered:**

- Monomorphize by capture set. Rejected because it explodes call sites and cannot support returned or stored closures.
- Trampoline with a typed environment pointer. Rejected because it would require a runtime type tag on every call.

### 2. Callable subtyping

**Decision:** A `Fn(P...) => R` binding accepts two or more closures with different capture sets at different assignments, as long as the parameter and result types match exactly. The checker does not unify capture sets across assignments.

**Rationale:** Capture sets are an implementation detail; the callable contract is the parameter/result type.

**Alternatives considered:**

- Require explicit capture-set annotations. Rejected because it would leak implementation details into the type system.

### 3. Callable `.clone()`

**Decision:** A callable supports `.clone()` when every captured value implements `Clone`. The clone produces an independent deep copy of the capture block by allocating a new capture block and cloning each slot.

**Rationale:** Callables are first-class values and must be duplicable like other `Clone` values when their captures allow it.

**Alternatives considered:**

- Always clone by reference. Rejected because mutably captured state would be shared, breaking value semantics.

## Risks / Trade-offs

- `[Risk]` The boxed callable representation introduces a new allocation on every capture. `Mitigation`: reuse the existing `clone()` heap-boxing path and optimize later with small-capture inlining once Phase 5 is stable.
- `[Risk]` Callable subtyping interacts with escape analysis because a `Fn(...)` stored in a field or returned must be treated as heap-escaping. `Mitigation`: update the checker escape/alias pass explicitly for boxed callable values and add targeted tests.
- `[Risk]` `.clone()` on callables may be expensive for captures with many references. `Mitigation`: rely on `Clone` trait semantics; document that cloning large captures is a `Clone` cost, not a callable cost.
