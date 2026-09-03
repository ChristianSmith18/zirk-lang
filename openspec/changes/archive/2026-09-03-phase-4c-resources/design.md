<!-- Concrete syntax for grouped `match with`: `match e1 with x1, e2 with x2 { body } error { e => handler }`. The trailing `error { ... }` branch is optional; a single `match e with x { arms }` keeps the existing match-arm form. -->

## Context

Phase 4c completes the single-threaded resource-management substrate that later phases will use for `Task`, `Channel`, `Mutex`, and `Atomic`. The pieces already delivered are single-resource `match with`, the `Resource<E>` contract, and the basic close/failure path. What remains is the cross-cutting completion that turns resources into a fully composable, transferable, and cancellation-aware facility.

This change focuses exclusively on Phase 4c: grouped `match with` acquisition, close-failure merging, `TransferableResource` transfer, dependent-resource lifetime analysis, and cancellation-aware cleanup. It reuses the existing mark-sweep runtime, the `Resource` object header, and the `RuntimeError` hierarchy; new work is limited to the surfaces that are genuinely incomplete.

## Goals / Non-Goals

**Goals:**

1. Implement grouped `match with` resource acquisition with left-to-right acquire and right-to-left unwind.
2. Combine body and close failures into `ResourceFailure<BodyError, CloseError>` with `Body`, `Close`, and `BodyAndClose` variants.
3. Add `transfer(r)` for `TransferableResource` values, invalidating the source and transferring ownership.
4. Implement dependent-resource lifetime analysis that rejects outliving uses through return, field store, closure capture, and task spawn.
5. Make resource cleanup observe the active cancellation token and avoid starting new blocking work when cancelled.
6. Add parser, semantic, lowering, codegen, and test coverage for the above.

**Non-Goals:**

- Phase 4b exception metadata, catchable overflow/invalid cast, or `Throwable` deep immutability.
- Phase 4d general callable polymorphism or boxed closures.
- Phase 4e `Dependent<T>` references, `Pin<T>`, or `inmut::strict` completion.
- Phase 5 concurrency primitives (`Task`, `Channel`, `Atomic`, `Mutex`, etc.).
- Collections (`Array<T>`/`List<T>`/`Map`/`Set`), generators, pipe, self-hosting, or package management.
- New keywords or syntax beyond the existing `match with`, `transfer`, and `Resource` surface.

## Decisions

### 1. Grouped resource acquisition and close order

**Decision:** Lower `match r1 with ..., r2 with ...` to a left-to-right acquisition sequence. On any acquisition failure, close every already-acquired resource in reverse order before entering the error branch. Within the body, close all acquired resources in the same reverse order on normal or exceptional exit.

**Rationale:** This generalizes the existing single-resource `match with` lowering with a cleanup list. Deterministic ordering makes failure handling predictable and avoids nested try/finally duplication.

**Alternatives considered:**
- Acquire in parallel. Rejected because cancellation, deterministic failure ordering, and resource interdependencies require sequential acquisition first.

### 2. Close-failure composition

**Decision:** Represent a combined body/close failure as a `ResourceFailure<BodyError, CloseError>` enum with `Body`, `Close`, and `BodyAndClose` variants. The error branch of grouped `match with` receives this value.

**Rationale:** A single union keeps both failures visible without inventing a tuple convention for every `E`/`CloseE` pair. The variants map directly to the observable cases: body failed, close failed, or both failed.

**Alternatives considered:**
- Return a plain tuple `(body?, close?)`. Rejected because it does not encode the "only one failed" invariants and would push complexity into user code.

### 3. Resource transfer and invalidation

**Decision:** `transfer(r)` lowers to a `ResourceTransfer` IR instruction that invalidates the source slot and returns the same resource object in a fresh, owned slot. The checker rejects any use of `r` after a statically visible `transfer` and restricts the operand to `TransferableResource`.

**Rationale:** A dedicated IR instruction keeps ownership transfer explicit and lets the runtime set a `moved` flag in debug builds, producing a clean runtime failure on `use after transfer`.

**Alternatives considered:**
- Purely static tracking with no runtime flag. Rejected because use-after-transfer should produce a clean failure rather than undefined behavior.

### 4. Dependent-resource lifetime

**Decision:** Track a parent slot in the resource descriptor for every `Dependent<T>` resource. The compiler rejects any dependent use that can outlive the parent: returns, field stores to objects that may live longer, closure captures, and task spawns.

**Rationale:** This matches the spec intent without introducing a full borrow checker; it reuses the existing escape analysis and resource descriptor.

**Alternatives considered:**
- Region-based lifetime inference. Rejected because it would require a substantial new type-system component beyond Phase 4c.

### 5. Cancellation-aware cleanup

**Decision:** Before invoking a resource's `close` action, read the active cancellation token. If cancelled, either skip the close entirely or return a cancellation-specific error depending on the resource contract. Cancellation never starts new blocking work.

**Rationale:** This prevents cleanup from blocking an already-cancelled scope while still giving resources that require cleanup a defined error path.

**Alternatives considered:**
- Always run close regardless of cancellation. Rejected because it can hang cancelled scopes and violates the cooperative cancellation contract.

## Risks / Trade-offs

- `[Risk]` Grouped `match with` changes parser, AST, and lowering paths that single-resource `match with` already uses. `Mitigation`: keep the existing single-resource path as a one-element degenerate case and add fixtures that specifically cover one, two, and three resources.
- `[Risk]` `ResourceFailure<BodyError, CloseError>` introduces a new generic enum into the stdlib type registry. `Mitigation`: define the enum early, lower the three variants in dedicated unit tests, and only then wire the error branch.
- `[Risk]` Transfer invalidation needs runtime support that does not exist yet. `Mitigation`: implement the runtime `Resource` object header and `moved` flag before adding the checker rules.
- `[Risk]` Cancellation-aware cleanup may make it harder to reason about resource invariants. `Mitigation`: document the cancellation contract in the resource spec and test both "skip" and "cancellation error" paths.
- `[Risk]` Restricting dependent resources to the parent lifetime can reject valid patterns. `Mitigation`: keep the analysis conservative, report clear diagnostics, and revisit once Phase 5 callables and tasks are available.
