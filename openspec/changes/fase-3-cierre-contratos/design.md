# Design: fase-3-cierre-contratos

## Context

Phase-3 contract support is complete for single-level `implements` on
`class`/`record`/`abstract class`, including generic contract dispatch
(`fase-3-dispatch-generico`). What remains are spec-promised surface
features that were never implemented:

- `zirk-contracts` "Contract composition and conflict resolution":
  `interface implements interface`, `trait implements interface/trait`,
  `TraitName.super.method()`.
- `zirk-classes` "Object casts and identity": `as?` returning nullable.
- `zirk-classes` "Explicit overriding and super dispatch": private/static
  methods not virtual (static itself lands in
  `fase-3-miembros-y-defaults`).
- `zirk-generics` "Generic declarations…": trailing type-parameter defaults
  satisfying their constraints.
- `zirk-contracts` "Operator contracts": reserved methods "such as `_add`"
  — comparisons were never mapped.

## Goals / Non-Goals

**Goals:**
- `TraitName.super.method()` callable inside an adopter that takes the
  method from ≥2 traits (and in any overriding body).
- Contract `implements` clauses with cycle detection and incompatible
  same-name signature rejection.
- `x as? T` producing `T?` with `null` on mismatch; unrelated `as?` still a
  compile error.
- `<T = Int32>` defaults used when the argument is omitted and the default
  satisfies the constraints.
- `<`, `>`, `<=`, `>=` dispatch through reserved contract methods on
  user types.

**Non-Goals:**
- `static` members, field default initializers, derivation model (owned by
  `fase-3-miembros-y-defaults`).
- Nested `implements` arguments (`Container<Box<T>>`) — still `NOT_LOWERED`.
- Trait default bodies under generic substitution.
- Multiple class inheritance (spec forbids it).

## Decisions

1. **Qualified trait call is an expression form, not a statement.** Parse
   `TraitName.super.method(args)` wherever `super.method()` is legal;
   resolve `TraitName` against the adopter's implemented traits. The
   receiver remains `this`. Lower to a direct call of the selected trait
   default body — no dynamic dispatch (the spec fixes the choice
   statically).
   - Alternative: select at the vtable. Rejected — the spec defines it as
     lexical conflict resolution, not dynamic dispatch.

2. **Contract `implements` is checked as a DAG.** Build the contract
   inheritance graph at declaration collection; reject cycles with a
   diagnostic naming the cycle, and reject inherited same-name signatures
   that are incompatible. An interface that `implements` another interface
   re-exports its requirements to adopters; a trait default satisfying an
   interface signature counts as provided.

3. **`as?` reuses the checked-cast machinery.** `x as? T` runs the same
   descriptor comparison as `as` but produces `T?` with `null` instead of a
   runtime failure. Type-relatedness is still required at compile time —
   `x as? Unrelated` is an error, not `null`.

4. **Type-parameter defaults substitute before inference fails.** When a
   type argument is omitted and a default exists, use the default; verify it
   against the parameter's `from` constraints at the declaration and at the
   use site like any other argument.

5. **Comparison operators follow the existing reserved-method scheme:**
   `_less`, `_less_equal`, `_greater`, `_greater_equal` (naming confirmed
   against `lower.rs`'s existing `_add`/`_equals` table and the contracts
   spec wording "reserved methods such as"). `<`/`<=`/`>`/`>=` on user types
   dispatch like `+`/`==` do today.

## Risks / Trade-offs

- [Contract composition makes conformance transitive] → Keep the
  satisfaction check structural: an adopter must still declare or inherit
  every required signature; composition only changes where requirements
  originate.
- [`TraitName.super` could bypass the conflict diagnostic] → The conflict
  error still fires when the class does not resolve the ambiguity; explicit
  selection is the resolution, not a loophole.
- [`as?` on non-nullable receivers] → Always allowed (never-null result is
  fine); keep compile-time relatedness.

## Open Questions

- Exact reserved-method names for comparisons if the spec/native table
  fixes different ones (check `lower.rs` reserved-method table first).
