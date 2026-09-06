# Design: fase-3-dispatch-generico

## Context

Contract dispatch is fully implemented for non-generic adopters:
`ObjectLayout.contracts: Vec<ContractTable>` records per-contract itables,
`lower.rs` builds those tables and emits `CallContract` /
`SafeDispatch::Contract`, and `emit.rs` resolves the itable from the object
descriptor at the call site.

The remaining gap: when a `class`/`record` implements a *generic* contract
(`implements Iterable<T>`) and the contract's methods name the contract's
own type parameters, the checker emits `E0423 NOT_LOWERED`
(`checker.rs:2339-2345`). Native contracts such as `Iterable<T>` are already
satisfied through a special path; user-defined generic contracts are not.
`zirk-generics` already requires this lowering ("Generic contract method
lowering", "Generic contract substitution does not corrupt the vtable").

This change consumes the constraint model exactly as it exists on `develop`
(`from` recorded, not yet verified); `fase-3-verificacion-constraints` owns
constraint verification and merges independently.

## Goals / Non-Goals

**Goals:**
- `class Box<T> implements Iterable<T>` and record equivalents lower and
  dispatch correctly per concrete instantiation.
- Contract method offsets remain stable across instantiations of the same
  generic type (vtable layout stability per spec).
- Trait default bodies lower correctly under generic substitution.
- Recursion in generic substitution produces a controlled diagnostic, not a
  stack overflow.

**Non-Goals:**
- Monomorphization of generic *classes* themselves beyond what already
  exists; only the contract-table/itable path is extended.
- Associated types, higher-kinded parameters.
- Constraint verification (`fase-3-verificacion-constraints`).

## Decisions

1. **Instantiate contract tables per concrete substitution.** When a
   generic adopter is instantiated (`Box<Int32>`), the contract table is
   built from the generic declaration's `implements` clause with the
   substitution applied, and each method entry points at the adopter's
   already-specialized method. This reuses the existing monomorphization
   pass instead of adding a new dispatch mechanism.
   - Alternative considered: a single shared itable with reified type
     parameters. Rejected: Zirk does not carry runtime type parameters; the
     existing design already specializes methods per instantiation.

2. **Substitute contract method signatures at table-build time.** The
   contract's declared signature `fn iterator(): Iterator<T>` is checked
   against the adopter's method after substituting the *contract's*
   parameters with the arguments written in `implements Iterable<T>`; the
   existing signature-compatibility check (`E0403`) runs on the substituted
   signature.

3. **Bounded substitution.** A substitution depth/visited-set guard covers
   mutually recursive generic `implements` clauses; exceeding it emits a
   controlled diagnostic per "Recursion cap on generic substitution".

4. **Keep `E0423` for uncovered shapes.** If a construct still cannot lower
   (e.g., a shape outside this change's scope), the diagnostic stays precise
   rather than falling through to a panic or `unreachable!`.

## Risks / Trade-offs

- [Generic dispatch interacts with abstract-class itable indexing] → Reuse
  the existing index-assignment scheme; add a fixture with two generic
  adopters of the same contract to lock offset stability.
- [Trait defaults under substitution may reference `Self`] → Substitute in
  the declared signature context before lowering the default body.
- [Merge overlap with `fase-3-verificacion-constraints` in `checker.rs`] →
  This change only removes the `E0423` gate and records conformance data;
  constraint verification lives in disjoint functions.

## Open Questions

- Whether generic `enum` adopters of contracts are in scope (default: yes if
  enums can declare `implements` today; otherwise document the exclusion).
- Generic `class` instances (`Box<Int32>`) are not yet implicitly assignable to
  a generic contract reference (`Container<Int32>`): `is_subclass_of` would
  need to substitute the class's own `implements` contract arguments with the
  instantiation's concrete types before comparing `Base::Instance` to
  `Base::ContractInstance`.
- Trait default bodies for generic contracts are not specialized per
  instantiation; a class that relies on a trait default for a `T`-bearing
  contract method still hits an `unreachable!` in lowering.
