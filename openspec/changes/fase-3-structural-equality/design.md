## Context

`record`/`value class` structural equality is fully specified (`ZIRK_LANGUAGE_SPEC.md` §7, the handbook's own structural-equality page) and accepted by the checker, but has no lowering at all — `reject_unstructured_comparison` (`checker.rs:6528-6548`) unconditionally rejects the case it should instead delegate to a real comparison. No field-by-field comparison generator exists anywhere in `zirk-ir`/`zirk-codegen-llvm` today; `lower.rs:8034` only ever routes `Eq`/`NotEq` to a class's own `_equals` method.

## Goals / Non-Goals

**Goals:**
- `==`/`!=` on a `record`/`value class` with no user `_equals` compares every field, ANDing the results, and recurses correctly into a nested `record`/`value class` field.
- A reference-typed field inside a value type compares by the correct rule (confirmed, not assumed, during design — see D2).

**Non-Goals:** hashing consistency, user-overridable equality for these two kinds, cross-type structural comparison (see proposal's "Explicitly out of scope").

## Decisions

### D1: Lower to a conjunction of per-field comparisons, generated at the call site, not a separate compiled function per type

`x == y` where both are the same `record`/`value class` type lowers directly to `x.f1 == y.f1 && x.f2 == y.f2 && ... && x.fn == y.fn` in IR terms — a chain of per-field `Binary { op: Eq }` instructions ANDed together with the existing short-circuit logical-AND IR shape (confirmed against the actual current IR shape for `&&` during implementation, not assumed) — rather than synthesizing a dedicated `_equals`-like function per type that every comparison site calls into.

Alternative considered: generate one compiled comparison function per `record`/`value class` type (mirroring how a `class`'s own `_equals` is an ordinary method) and lower every `==` to a call into it. Rejected for this change's scope — inline expansion at the call site avoids introducing a new per-type synthesized-function mechanism (a new category of compiler-generated method, distinct from both the compiler-derived `Clone` and a user's own `_equals`) for what is, per field count in realistic types, a small and non-recursive-at-the-call-level expansion; a future change can convert to an out-of-line generated function if code-size becomes a real concern (not evidenced yet).

### D2: A reference-typed field inside a value type compares by whatever the language's existing rule already says for that field's own declared type, not a value-type-specific special case

If a `record`/`value class` field is itself a `class` reference, its own `==` semantics apply recursively (identity via `is`, or the referenced class's own `_equals` if it exists, per the already-existing, unrelated-to-this-change rule for `class` equality) — this change does not invent a new rule for "a reference field reached from inside a value type," it only makes the *value type's own* top-level comparison exist at all, by combining whatever each field's own comparison already means. Confirm the exact current rule for a bare `class`-typed field's `==` (identity vs. `_equals`) against `checker.rs`'s actual current behavior during implementation, since design intentionally does not restate it here to avoid drifting from whatever it turns out to actually be.

### D3: No new diagnostic; the existing `not_lowered` gate is simply removed for this case

Since this delivers a case the checker already accepts type-wise, no new error code is introduced. If a genuinely unsupported field type is found during implementation (e.g., a field type whose own `==` is itself not-yet-lowered for an unrelated reason — a chain of undelivered work, not this change's own gap), that residual case keeps its own existing diagnostic rather than this change inventing a new one for it.

## Risks / Trade-offs

- **[Risk] A `record`/`value class` with many fields produces a correspondingly long chain of ANDed comparisons at every `==` call site** (D1's own inline-expansion choice) — code-size growth proportional to (field count × number of comparison sites), unlike an out-of-line generated function. → Accepted for this change's scope: no evidence yet that any real Zirk type has enough fields for this to matter; flagged so a future change knows why the trade-off was made and what would motivate revisiting it (real measured code-size pressure, not a hypothetical).
- **[Risk] Short-circuiting via `&&` means a later field is never compared once an earlier one differs** — correct and desirable for performance, but worth confirming this doesn't interact badly with a field whose own comparison could have a visible side effect (a `class`-typed field's `_equals`, if a user writes one with a side effect, which would be against the spirit of "equality" but is not currently forbidden by any existing rule). → Accepted: this is not a new problem this change introduces — a `class`'s own `_equals` already has the same short-circuiting risk wherever the language ANDs field comparisons for anything else equality-adjacent (e.g. `Comparable` derivation, if it exists) — not a new hazard specific to value types.
