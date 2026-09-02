## Why

`ZIRK_LANGUAGE_SPEC.md` section 7 and `docs/handbook/02-handbook/05-operators-and-expressions/03-structural-equality.md` both already state that a `record`/`value class` has derived structural equality (`==` compares every field), unlike an ordinary `class`, which requires an explicit `_equals` method. The checker already accepts `==` on a `record`/`value class` type-wise (`checker.rs:6528-6541`, `reject_unstructured_comparison`), but every such comparison is unconditionally rejected at lowering with `NOT_LOWERED` (E0423, `checker.rs:6542-6547`) — "compare its fields individually for now." `openspec/changes/archive/2026-08-19-phase-3-objects-and-type-system/design.md:199-201` records this as decided-but-undelivered debt with no roadmap task number assigned, unlike the enum/class monomorphization work. Lowering-side confirmation: `lower.rs:8034` only ever maps `Eq`/`NotEq` to a class's own `_equals` method call; no field-by-field comparison generator exists anywhere in `zirk-ir`/`zirk-codegen-llvm`.

## What Changes

- Lower `==`/`!=` on a `record`/`value class` (with no user `_equals`) to a field-by-field structural comparison: AND together each field's own equality (recursing into a nested `record`/`value class` field the same way; a scalar field compares directly; a reference-typed field compares via `is` per the language's existing "reference semantics" rule for identity-bearing referents reached through a value type's own field — confirmed against the actual current field-typing rules during design, not assumed).
- Remove the checker's blanket rejection for this case (`checker.rs:6542-6547`).
- `!=` is the boolean negation of the same comparison — no separate lowering needed beyond however the existing `Eq`/`NotEq` split already reuses one comparison (confirmed against current `lower.rs` structure during implementation).

### Explicitly out of scope

- **Hashing consistent with derived equality** (`Hashable`) — not requested by this change's own scope; if `record`/`value class` values are used as map keys today through some other mechanism, this change does not touch it. A future change should confirm consistency once this lands.
- **User-overridable equality for `record`/`value class`** — the spec's own rule is that derived equality is *not* opted into via `_equals` for these two kinds (only an ordinary `class` uses that reserved method); this change delivers the derived case only, no override surface.
- **Structural equality across two different declared types with the same shape** — `zirk-type-system`'s existing "Two types with the same shape are not the same type" rule already makes this a type error before `==`'s own runtime behavior is reached; unaffected by this change.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-type-system` (or the closest existing equality-related requirement — confirmed during design against current spec text): the existing structural-equality rule for `record`/`value class` is reaffirmed and extended with a lowering-level scenario confirming field-by-field comparison, including a nested `record`/`value class` field.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (remove the `not_lowered` gate at `reject_unstructured_comparison`, ~line 6536-6548), `crates/zirk-ir/src/lower.rs` (new lowering for `Eq`/`NotEq` when the operand type is `record`/`value class` without a user `_equals` — a field-by-field comparison chain, reusing the value type's own field-offset/layout information the same way `fase-4e-clone`'s field-walk work reused the collector's per-class table where applicable), `crates/zirk-codegen-llvm` (emitting the actual per-field comparisons and their conjunction — likely no new IR instruction needed if this lowers to existing `Binary { op: Eq, .. }`/logical-AND IR, confirmed during implementation).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s "Several Phase 3 constructs..." bullet drops "derived structural equality" once delivered. `../zirk-lang-site` sync required.
- No breaking changes: lifting a rejection only allows previously-rejected programs to compile.
