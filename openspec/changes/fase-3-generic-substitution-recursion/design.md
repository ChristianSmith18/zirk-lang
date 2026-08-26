## Context

`Checker::substitute_type` (`checker.rs:4389`) and `Checker::substitute`/`Checker::infer_type_params` (`checker.rs:9927`/`9837`) are two independently-maintained substitution implementations serving different call sites (contract/class signature substitution vs. generic function/method call inference) that have silently diverged: only the first recurses into a nested `Base::ContractInstance`/`Base::EnumInstance`'s own type arguments.

## Goals / Non-Goals

**Goals:**
- `substitute` correctly replaces a type parameter appearing inside a nested generic instantiation (`Option2<T>` substituted with `{T: Int32}` becomes `Option2<Int32>`, not left as `Option2<T>`).
- `infer_type_params` correctly infers a type parameter from an argument's type when the parameter's own declared type is a nested generic instantiation naming that parameter, not just when the parameter's declared type is directly `Base::Param`.
- No regression to any existing generic call site — this function backs every generic function/method call's own inference.

**Non-Goals:** unifying every substitution call site in the codebase into one implementation if that turns out to be a larger refactor than this fix needs (see D1's own scoping note).

## Decisions

### D1: Prefer having `substitute` delegate to `substitute_type`'s logic directly; fall back to porting the recursive-args logic only if delegation proves impractical

The two functions differ in receiver mutability (`&mut self` vs `&self`) and substitution-map shape (`&[(u32, Type)]` vs `&HashMap<u32, Type>`) — likely for real reasons tied to their respective call sites (confirm during implementation, don't assume they're accidental). If `substitute`'s call sites can tolerate `&mut self` and a slice-based substitution list (or a cheap conversion from the `HashMap` it already builds), delegating directly to `substitute_type` removes the duplication outright and guarantees the two paths can never diverge again. If that turns out to require touching every `substitute` call site in a way that risks the "verify, don't build" discipline this class of change has followed all session, port `substitute_type`'s recursive-args handling into `substitute` as a smaller, scoped fix instead, and note the remaining duplication as a known, accepted trade-off rather than silently declaring the two unified when they are not.

### D2: `infer_type_params` needs its own, separate recursive-matching logic, not just a call to the (now-fixed) `substitute`

`substitute` replaces a parameter once its solution is already known; `infer_type_params`'s own loop (`checker.rs:9850-9886`) is what *discovers* that solution from an argument's actual type, matched positionally against a parameter's *declared* type. Discovering `T = Int32` from an argument typed `Option2<Int32>` against a declared parameter type `Option2<T>` requires walking both types in parallel (declared vs. actual) and unifying at the leaf `Base::Param` — a different operation from substitution itself, needing its own recursive-descent implementation, structurally mirroring `substitute_type`'s own recursive shape but unifying two types against each other rather than substituting one type through a map.

## Risks / Trade-offs

- **[Risk] `infer_type_params`'s new recursive unification could itself have gaps for some type shape not yet exercised** (e.g., a doubly-nested instantiation, `Outer<Inner<T>>`) — the fix should be written generally (walk any nested `EnumInstance`/`ContractInstance`'s own args recursively, not just one level deep) rather than special-cased for exactly the one reported shape, but this must be verified with an explicit doubly-nested test, not assumed correct from the shape of the code alone.
- **[Risk] This function backs every existing generic call site in the compiler** — a subtle regression here would be a correctness regression across a wide surface, not a narrow one. → Mitigation: full `cargo test --workspace` (not just new-shape tests) is non-negotiable before this change is considered done, and existing generic-call corpus fixtures must be spot-checked to still produce identical output, not just "still compile."
