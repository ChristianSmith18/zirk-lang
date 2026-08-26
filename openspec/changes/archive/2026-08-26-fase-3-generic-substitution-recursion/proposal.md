## Why

`fase-3-generic-enums` (merged) found and reported, rather than fixed, a real gap in the checker's shared generic-argument inference/substitution machinery: `Checker::infer_type_params` (`crates/zirk-sema/src/checker.rs:9837`) and `Checker::substitute` (`checker.rs:9927`) only handle a parameter type that is *directly* `Base::Param` — neither recurses into a nested `Base::EnumInstance`'s or `Base::ContractInstance`'s own type arguments. A third, sibling function, `Checker::substitute_type` (`checker.rs:4389`), already does this correctly (it recurses into `Base::ContractInstance`/`Base::EnumInstance` args, `checker.rs:4400-4418`). The result: constructing `Wrapper.Inner(Option2.Some2(9))` against an expected `Wrapper<Int32>` fails with `E0403: there is no implicit conversion from Option2<Int32> to Option2<T>`, because `substitute(Option2<T>, {T: Int32})` returns `Option2<T>` unchanged — the top-level type is `EnumInstance`, not `Param`, so the substitution never looks inside it.

This is not enum-specific: `infer_type_params`/`substitute` back every generic function/method call's own inference, so the same gap blocks a generic function parameter typed `Option2<T>` (or `SomeContract<T>`) for any generic function, independent of `fase-3-generic-enums`'s own scope.

## What Changes

- Generalize `Checker::substitute` (`checker.rs:9927`) to recurse into a `Base::EnumInstance`/`Base::ContractInstance`'s own type arguments the same way `substitute_type` already does — ideally by having `substitute` delegate to (or share logic with) `substitute_type` directly, rather than maintaining two independently-evolving substitution implementations. Confirm during implementation whether unifying them outright is safe (their signatures differ slightly: `substitute_type` takes `&mut self` and a `&[(u32, Type)]` slice, `substitute` takes `&self` and a `&HashMap<u32, Type>`) or whether the fix is duplicating `substitute_type`'s recursive-args logic into `substitute` while noting the duplication for a future unification.
- Generalize `Checker::infer_type_params` (`checker.rs:9837`) to also infer a type parameter from inside a nested generic instantiation appearing as a parameter's declared type (e.g. inferring `T` from an argument of type `Option2<Int32>` against a declared parameter type `Option2<T>`), not just when the parameter's own declared type is directly `Base::Param`.
- No grammar or new diagnostic: this closes a checker-level inference gap for already-legal generic call syntax, it does not add new syntax.

### Explicitly out of scope

- **Full higher-kinded or partial-application generic inference** — this closes the specific "nested instantiation as a parameter type" gap, not a general overhaul of the inference algorithm's expressiveness.
- **Anything about enum declaration order or self-reference** — that is `fase-3-recursive-enums`'s own, separate concern; this change is purely about substitution correctness once a type is already resolvable.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-type-system`: the existing "Generic projection needs Clone"-adjacent generic-inference behavior has no prior requirement stating inference recurses into a nested generic instantiation; a new scenario is added confirming this now works for both a generic function parameter and (once `fase-3-recursive-enums` or a similar case exercises it) a generic enum variant payload.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (`substitute`, `infer_type_params` — both are core, widely-called checker functions, not enum- or contract-specific; changing them touches every generic function/method call site's own inference, so this needs full-suite regression testing, not just new-shape tests). **Found necessary during implementation, beyond this list at proposal time**: `crates/zirk-ir/src/lower.rs`'s `specialize_enum` has an independent local `substitute` closure with the identical shallow bug — left unfixed, the checker fix alone would have turned a clean type error into an internal-compiler-error/miscompile for exactly this change's own target scenario. Fixed alongside the checker change (new `substitute_generic_type`); `specialize_class` was not touched (independently gated for this shape already).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s note (added by `fase-3-generic-enums`) about "generic-instantiation-in-a-payload... blocked in shared generic-inference substitution" is removed once delivered.
- No breaking changes: this only makes previously-rejected, already-legal-looking generic calls succeed; nothing that compiles today changes behavior, since the old code path only ever left a type parameter unsolved in exactly the case this fixes.
