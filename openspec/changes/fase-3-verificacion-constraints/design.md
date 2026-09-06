# Design: fase-3-verificacion-constraints

## Context

The parser already accepts `T from A & B` constraints and `in`/`out`
variance annotations on type parameters (`parser.rs` `parse_type_params`,
`ast::Variance`, `ast::TypeParam.constraints`). The checker records
constraints in `TypeParamInfo` (`checker.rs` `enter_type_params`) but never
verifies them, and reports `PENDING_FEATURE` for any declared variance
(`report_declared_variance`, `checker.rs:2826-2842`).

Normatively, `zirk-generics` already requires: use-site verification of
`from` constraints, single body-check against constraints, and declared
variance semantics — while also containing a contradictory "scope" clause
stating the checker SHALL NOT support declared variance. `value class` was
removed by `array-list-tuple-duration-regex` but four specs still mention it.

## Goals / Non-Goals

**Goals:**
- Verify `from` constraints at every use site (type annotations,
  instantiations, call arguments, defaults) with diagnostics naming the type
  parameter and each missing contract.
- Check each generic body exactly once against its constraints: a member
  access on a value of type `T` is legal iff some constraint declares it.
- Verify declared variance: `out T` only in output positions, `in T` only in
  input positions, mutable attribute use requires invariance; `Fn` keeps
  intrinsic variance.
- Reconcile the contradictory `zirk-generics` scope clause and remove
  `value class` remnants from specs.

**Non-Goals:**
- Lowering `implements Contract<T>` for user generic types (owned by
  `fase-3-dispatch-generico`).
- Associated types, higher-kinded parameters (remain out of scope).
- `Fn` intrinsic variance mechanics beyond keeping current behavior.

## Decisions

1. **Enforce declared variance rather than reject it.** The final language
   supports it (`zirk-type-system` last-phase requirement) and the syntax is
   already parsed; rejecting it would create churn for no semantic gain.
   The "Scope of generics in this phase" requirement is modified to drop
   variance from the unsupported list, keeping associated types and
   higher-kinded parameters excluded.
   - Alternative considered: reject `in`/`out` with a stable diagnostic.
     Rejected because the normative requirement already defines the
     semantics and Phase 3 is the right moment to close it.

2. **Variance is checked on the declaration, not at use sites.** Position
   analysis walks each member signature of the generic declaration: output
   positions (return types, immutable field reads) for `out`, input
   positions (parameter types) for `in`, and any mutable attribute whose
   type mentions the parameter forces invariance. A `T` appearing in both
   directions or in a `mut` field contradicts `out`/`in` and is diagnosed at
   the declaration span.

3. **Constraint satisfaction reuses contract conformance.** "Satisfies
   `T from C`" delegates to the existing `implements`/conformance machinery
   (including native contract satisfaction such as `Iterable<T>`), so a type
   that satisfies a contract structurally or nominally satisfies the
   constraint the same way.

4. **Body checking treats `T` as an opaque type whose member set is the
   union of its constraints.** Member resolution on `T` searches the
   declared contract constraints (including combined `A & B`); anything else
   is `E040x` with help naming the missing constraint.

## Risks / Trade-offs

- [Constraint verification may reject previously accepted programs] → They
  were accepted only because verification was missing; the corpus gains
  invalid fixtures and the change is flagged as a fix in the feature-status
  notes.
- [Variance position analysis may be too strict for exotic signatures] →
  Keep the first version conservative: only method parameter/return
  positions and mutable fields are analyzed; ambiguous positions are
  diagnosed rather than silently accepted.
- [Overlap with `fase-3-dispatch-generico` in `checker.rs`] → This change
  does not touch contract-table construction or lowering; the parallel
  change merges on disjoint functions.

## Open Questions

- Whether generic `enum`/`alias` declarations participate in variance
  checking in this pass or keep the pending diagnostic (default: apply the
  same positional rules; document whichever lands).
