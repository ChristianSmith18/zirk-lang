## Context

`fase-4b-excepciones` closed `throw`/`try`/`catch`/`finally` with real
execution, deferring `Resource<E>`/`match with` because they needed exactly
that: `finally` running on every exit path is the same mechanism a resource's
automatic close needs, applied to one implicit binding instead of a written
block. `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 4 is the
authorial source of truth this pass narrows from; `openspec/specs/zirk-resources/spec.md`
(already archived, from an earlier documentation-only pass) states the full
target shape across five requirements — this pass implements the first one,
partially.

## Goals / Non-Goals

**Goals:**

- `interface Resource<E from Error> { fn close(): Result<Void,E>; fn is_closed(): Boolean; }`,
  registered the same "inject the tables directly" way `Iterable<T>`/
  `Iterator<T>` are.
- `implements Resource<SomeError>` on an application class, verified by the
  same conformance machinery every other `implements` uses.
- `match scrutinee with binding { ... }` grammar: scrutinee first, `with
  binding` optional after it, before `{`.
- Checker validation: the scrutinee must be `Result<R,Err>`; exactly one arm's
  own pattern must bind `binding`'s name; that binding's type must implement
  `Resource<E>`.
- Real execution: the arm that acquires the resource closes it — calls
  `close()`, discarding its `Result` — on normal completion, `return`,
  `break`, `continue`, and a propagating exception.

**Non-Goals:**

- Grouped acquisition (`match a with x, b with y`) and its right-to-left
  unwind on a later acquisition's failure.
- `ResourceFailure<BodyError,CloseError>` — a close failure's own `Result` is
  discarded, not merged with the body's outcome.
- `suppressed` populated from a close failure during exception propagation —
  needs `List<T>` (Phase 7) and `ResourceFailure`, both out of scope.
- `TransferableResource`/`transfer()` and the escape/use-after-transfer
  analysis — a bespoke, simplified borrow-checker-shaped data-flow analysis,
  too large for a first cut.
- Dependent-resource lifetime, `take` on a non-cloneable resource inside a
  container — need collections (Phase 7).
- Cancellation-aware cleanup — no structured concurrency exists yet (Phase 5).

## Decisions

**D1 — `match ... with binding` reuses `fase-4b-excepciones`'s `try`/`finally`
machinery wholesale: the arm that acquires the resource is lowered as if
written `try { <arm body> } finally { binding.close(); }`, with no `catch` of
its own.** `zirk-ir::FunctionLowering::try_stack` already answers exactly the
question a resource's automatic close asks — "what runs on every exit path
out of this scope, however it leaves" — for an ordinary `finally` block
(design D3 of `fase-4b-excepciones/design.md`). A resource frame is pushed
right before the owning arm's body is lowered and popped right after, with
`catches: Vec::new()` and `finally: Some(<synthetic block>)`. Every mechanism
that already walks `try_stack` — `run_finally_through` for `return`/`break`/
`continue`, `lower_pending_exception_dispatch` for a propagating exception —
needed zero changes: it processes this frame exactly as it would a real one,
since nothing about it distinguishes a real `finally` from a synthetic one.
Only the normal-completion path (falling off the end of the arm) needed new
code in `lower_match`, mirroring `lower_try`'s own post-body check.

**D2 — the synthetic `finally` block is a real `ast::Block` built in
`zirk-ir/lower.rs`, not a new IR instruction.** `binding.close()` is
synthesized as an ordinary `ast::Expr::Call` over an `ast::Expr::Field` —
`Ident::new(binding_name, span).close()` — wrapped in a one-statement
`ast::Block`, then lowered through the exact same path an ordinary method
call takes (`Self::lower_expr_for_effect`'s `method_of` arm), which resolves
the receiver by its own slot in `self.scopes` and the method by the
receiver's own concrete class — no dispatch through the `Resource`-typed
reference, no checker-side table consulted, since none was built for this
synthetic node (the checker never sees it: it exists purely inside
`zirk-ir`). This is why `Resource<E>` needed no contract-dispatch-table
specialization the way `Iterable<T>`/`Iterator<T>` do (roadmap task 13.5) —
`close()`/`is_closed()` are always reached by ordinary static dispatch on a
concrete class, matching `Checker::resolve_implements_args`'s own
`NOT_LOWERED` gate, widened to exempt `Resource<E>` for exactly this reason.

**D3 — `Resource<E>`'s own `E` constraint (`from Error`) is expressed with
`TypeParamInfo::constraints` directly, the same field a user-declared
interface's own `from` clause resolves into, rather than left unconstrained
the way `Iterable<T>`'s `T` is.** `Iterable<T>`'s own `T` was registered
before any constraint-checking machinery existed to enforce one; `Error`
already exists once `register_native_exception_hierarchy` has run, so
`register_native_resource_contract` runs after it and constrains `E` for
real — `implements Resource<SomeType>` where `SomeType` does not implement
`Error` is rejected by the same `satisfies_constraint` check an ordinary
`implements Iterable<SomeType>`-shaped generic contract already uses for its
own constrained parameters.

**D4 — `Result<Void,E>` needed a narrow, pre-existing gap in
`zirk-codegen-llvm` fixed, not worked around.** `Resource<E>::close()`'s
fixed signature (`Result<Void,E>`) is the first place the compiler ever had
to build the LLVM type of an enum with a `Void`-typed field — nothing before
this phase constructed one. `emit.rs`'s `enum_struct`/`InstKind::BuildEnum`/
`InstKind::LoadField` all assumed every field has a real LLVM representation
and panicked otherwise (`.expect("an enum field is not Void")`). Rather than
change `close()`'s signature to dodge the gap — which would depart from
`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`'s own exact pseudocode for no
reason of principle, only of convenience — the gap is fixed narrowly: a
`Void` field occupies a genuine zero-sized LLVM struct (`{}`) rather than
being skipped, so every other field's index into the flattened struct still
matches `EnumLayout.variants`'s own unchanged numbering; `BuildEnum` skips
inserting a value for it (there is none — a `Void`-producing instruction
never registers one in `self.values` either); `LoadField` returns `None`
(no value) when the field being read is `Void`, the same shape every other
`None`-producing instruction already uses. `value_struct`/`object_struct`/
closure-capture layouts are untouched: the checker never lets a field or a
capture be declared `Void` (`VOID_VARIABLE`), so nothing exercises those
paths regardless — narrower than a general "`Void` is a real zero-sized
value everywhere" change would have been, and sufficient for what this phase
needs.

**D5 — `match ... with` requires a `Result<R,Err>` scrutinee, rejecting
anything else, rather than accepting an arbitrary enum whose *some* arm binds
a `Resource`-implementing value.** `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
section 4 states acquisition "normally returns `Result<Resource,OpenError>`"
— normally, not exclusively, leaving room for a different acquisition shape
later. Narrowing to exactly `Result<R,Err>` for this pass keeps the checker's
own validation (`Self::check_resource_match_scrutinee`) simple and matches
every example the spec itself gives; widening it later, if a real use case
needs to, is additive.

## Risks / Trade-offs

- **A resource frame pushed onto `try_stack` is indistinguishable from a real
  `try`'s to every mechanism that walks it** (D1) — this is the whole point,
  but it does mean a `catch` written inside the arm that acquires the
  resource sees one fewer enclosing `try` than `try_stack.len()` alone would
  suggest at a glance; nothing in this pass needed to tell the two apart, so
  no such distinction exists anywhere in the code.
- **Only a single resource per `match ... with` is supported** — grouped
  acquisition needs right-to-left unwind ordering across multiple frames,
  which the current one-frame-per-arm shape does not build; accepted as
  explicit scope, `proposal.md`'s own non-goals.
- **A close failure is silently discarded** — `close()`'s own `Result` is
  never consumed, which would ordinarily be `DISCARDED_RESULT` (`E0433`) had
  it gone through the checker; it does not, since the synthetic call is
  built directly in `zirk-ir`, bypassing the checker entirely for this one
  node. A real program that needs to observe a close failure cannot, in this
  pass — accepted, since `ResourceFailure` is out of scope regardless.
- **The `Void`-field codegen fix (D4) is narrow by design, not exhaustive**:
  a `Void` value used any other way `zirk-codegen-llvm` has never had to
  support (a `Void` object field, a `Void` closure capture) still panics.
  Nothing in the checker admits either today (`VOID_VARIABLE`), so this is
  dead code, not a live gap — but it is not proven unreachable by
  construction, only by every current caller of `declare_class`/
  `declare_closure` going through the checker first.

## Migration Plan

Additive over a pipeline that already compiles and runs end to end. Nothing
existing changes behavior. Rollback: revert the merge, nothing later depends
on this yet.

## Open Questions

- None outstanding — scope narrowed against `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
  section 4 and `openspec/specs/zirk-resources/spec.md`'s own five
  requirements, following `fase-4b-excepciones`'s own precedent for how much
  of a large spec section one phase should take.
