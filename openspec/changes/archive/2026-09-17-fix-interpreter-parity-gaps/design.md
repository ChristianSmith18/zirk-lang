# Design: fix interpreter parity gaps

## Context

The browser interpreter shares the frontend (lexer, parser, sema, lowering,
verifier) with the native pipeline and diverges only at execution. The audit
found three classes of divergence:

1. **Contract bugs**: the lowering emits `result: Some` on instructions whose
   semantics produce no value (`Void` calls, `ListAdd`, `MapSet`, `SetAdd`, …).
   The verifier accepts them; the interpreter treats `result.is_some()` as
   "must produce a value" and aborts with an internal failure (`ZB198`).
2. **Missing interpreter arms**: `InstKind::Retype` is classified
   `BrowserSupport::Direct` but has no execution arm — it falls into the
   catch-all "not implemented yet". This blocks typed `catch`, `as`/`as?`
   downcasts, and subtype→supertype coercion.
3. **Missing intrinsics**: 171 of 242 declared externs are absent from
   `is_browser_intrinsic`, so they fail preflight with `ZB001` even though most
   are pure computation (`zirk_rt_decimal_*`, `zirk_int_*`, `zirk_float_*`,
   `zirk_str_*`, `zirk_regex_*`, `zirk_duration_*`, pure temporal helpers).
   `zirk-runtime-core` is already a dependency of the interpreter and owns the
   pure logic.

Separately, two language-level issues: `length` is `UInt64` on `Array`/`List`
(and inconsistently `Int32` on `Map`/`Set`), which makes `i < xs.length` a
compile error because comparison requires identical numeric types; and the
checker rejects ordering operators on `Decimal` even though lowering already
emits `zirk_rt_decimal_cmp`.

## Goals / Non-Goals

**Goals**

- Any program the native pipeline accepts and that uses only
  browser-compatible capabilities runs identically in the interpreter.
- The IR contract for "produces a value" vs "effect only" is explicit and
  enforced, so the class of bug cannot silently return.
- Conformance fixtures exist for every fixed gap.

**Non-goals**

- Implementing host-authority externs (wall clock, filesystem, process,
  journal) in the browser. They stay preflight-rejected.
- Filling every one of the 171 missing externs in this change if some require
  disproportionate work; the change classifies all of them and implements the
  pure-computation set. Any deliberate exclusions are documented.
- Changing the interpreter's public API contract (`apiVersion: 1` stays).

## Decisions

### D1. Void calls: fix the interpreter's `ReturnDestination`, and tighten the IR contract

`caller_value` is derived from `instruction.result` only when
`instruction.ty != IrType::Void`. This is the minimal fix that covers direct
calls, method calls, closure/callable calls, and `Void throws` calls under
`try` — all of which share the pattern — without touching every lowering site.

In parallel, the verifier gains a rule: an instruction of type `Void` MUST NOT
declare `result: Some`. The lowering is migrated from `emit` to `emit_effect`
at the `Void` call sites so freshly-lowered modules satisfy the stricter
contract. The interpreter tolerance remains as defense-in-depth for modules
produced before the tightening.

- *Alternative*: fix only lowering. Rejected — there are ~dozens of `emit(...,
  IrType::Void)` sites, the pattern is systemic, and a future site could
  reintroduce it; verifier enforcement is what actually closes the hole.
- *Alternative*: fix only the interpreter. Rejected — leaves invalid IR
  silently accepted, and `result: Some` on `Void` remains ambiguous for other
  consumers.

### D2. Effect instructions: migrate `emit` → `emit_effect` at `Void` sites

Same root cause as D1. `ListAdd`, `ListInsert`, `MapSet`, `SetAdd`, and the
other `Ok(None)` instructions are emitted with `emit_effect` (result `None`).
Covered by the same verifier rule, so any missed site fails fast at verify
time rather than mid-execution.

### D3. `Retype`: implement the execution arm

`Retype(v)` is a compile-time-directed coercion: upcasts are no-ops at runtime
(the value already carries its concrete type), and narrowing succeeds when the
value's runtime type is compatible with the target — the lowering already
emits a runtime type check (`CheckType`/`TypeCheck` arm) for the checkable
cases, so `Retype` itself evaluates to the operand value. The arm resolves the
operand and returns it unchanged; mismatched narrowing is already diagnosed by
the preceding check instruction.

If audit during implementation finds a `Retype` use that carries semantic
weight (e.g., record→contract materialization), that case gets its own
handling rather than a blanket pass-through.

### D4. Pure externs: implement as interpreter intrinsics via `zirk-runtime-core`

The interpreter calls the shared pure helpers (decimal ops, int/float members,
string ops, regex, duration members, pure temporal helpers) directly instead
of dispatching to extern symbols. `is_browser_intrinsic` is extended for the
pure set; `execute_browser_intrinsic` gains the matching arms. This keeps
browser semantics bit-identical to native because both use the same
implementation.

The work is staged in task order: decimal first (unblocks `10 / 3`, decimal
literals, comparisons), then int/float members, then string/regex, then
duration/temporal. Host-authority externs (`zirk_rt_date_today`,
`zirk_rt_*_now_*`, filesystem, process, journal) stay rejected and are listed
in the parity matrix as native-only.

### D5. `length` ergonomics: allow mixed numeric comparisons via widening (option b)

The checker's "same numeric type" rule for ordering operators is replaced by
the arithmetic rule already in the spec: compare in the smallest common type
that represents both operands (`Int32` vs `UInt64` → `Int128`, computed
exactly). `length` keeps its `UInt64` type — it genuinely is non-negative —
and `for mut i = 0; i < xs.length` just works.

- *Alternative (a)*: change `length` to `Int32`/`Int64`. Rejected — shrinks
  the expressible range, is a breaking type change, and `Int32` can't
  represent lengths above ~2B that the runtime otherwise supports.
- *Alternative (c)*: add `count(): Int32` or an `indices` view. Kept as a
  possible follow-up, but it doesn't fix `xs.length` comparisons in existing
  code and leaves `Map`/`Set`'s `Int32` inconsistency in place.

Doc inconsistency (`Map`/`Set` documented `Int32`) is resolved by updating the
handbook so all four families document `UInt64`, matching implementation.

### D6. `Decimal` ordering: add `Base::Decimal` to `expect_comparable`

One-line checker change aligning sema with lowering, which already emits
`zirk_rt_decimal_cmp` for `<`/`>`/`<=`/`>=`. Mixed `Decimal`/integer
comparisons fall out of D5's widening rule (int widens to `Decimal`).
`Decimal`/`Float` comparisons stay rejected (no lossless common type).

### D7. Uncaught throwable: render `message()`

`uncaught_exception` formats `info.value` with `{:?}`, leaking
`Heap(HeapId(0))`. It now extracts the throwable's `message` field via the
runtime representation, falling back to the type name when the message cannot
be read. Category/span/stack are unchanged.

## Risks / Trade-offs

- [New verifier rule rejects previously-accepted modules] → Acceptable: those
  modules carried invalid IR and would have crashed the interpreter anyway;
  the native path ignores `result` on `Void`, so behavior is unchanged there.
- [171 externs is a large surface] → Staged implementation + explicit parity
  matrix (supported-and-tested / not-yet / native-only / out-of-scope), so the
  remaining gap is documented rather than silent.
- [Mixed-comparison widening hides real bugs, e.g. comparing a length against
  a `Float`] → Widening only applies where a lossless common type exists;
  `Decimal`/`Float` stays explicit-cast-only, so no precision surprise.
- [Temporal helpers have large pure surface] → Implement `parse`/format/
  arithmetic first; `today`/`now` remain native-only.

## Migration Plan

- Code changes land behind the normal test suite plus new fixtures; the
  website's pinned WASM package is rebuilt in a follow-up commit — no contract
  change (`apiVersion: 1` unchanged).
- Rollback: revert the commit; no persisted state or schema is affected.

## Open Questions

- Whether `count(): Int32` should also be added as a convenience member even
  though comparisons now work — deferrable follow-up, doesn't affect this
  change's specs.
