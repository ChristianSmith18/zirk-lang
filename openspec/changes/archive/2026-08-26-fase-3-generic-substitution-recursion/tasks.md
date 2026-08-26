## 1. Checker (zirk-sema)

- [x] 1.1 Fixed `Checker::substitute` (`crates/zirk-sema/src/checker.rs`, ~line 9927) to recurse into a nested `Base::EnumInstance`/`Base::ContractInstance`/`Base::Instance`'s own args at any depth, mirroring `substitute_type`. **Design deviation**: kept `substitute` and `substitute_type` as two functions rather than unifying them (design D1's preferred path) — `substitute`'s three call sites all pass a `&HashMap<u32, Type>` built from inference, and converting them all to `substitute_type`'s `&[(u32, Type)]`+`&mut self` shape was judged a larger, riskier touch than porting the recursive-args logic (D1's own documented fallback, taken here). Top-level `Base::Param` nullable handling untouched.
- [x] 1.2 Added `Checker::unify_declared_with_actual` (new), a recursive-descent unifier walking a declared parameter type against an argument's actual type in parallel, discovering a type parameter's solution at any nesting depth inside a matching `EnumInstance`/`ContractInstance`/`Instance` shape. `infer_type_params`'s own loop now calls this instead of only handling a direct `Base::Param`.
- [x] 1.3 Corpus fixtures added under `crates/zirk-cli/tests/corpus/valid/`: `generic_infer_nested_param_no_seed.zrk` (single-level inference through a nested parameter type), `generic_infer_doubly_nested_param.zrk` (`Box<Box<T>>`, design's own flagged depth risk), `generic_enum_nested_payload.zrk` (the originally-reported `Bar<Baz<T>>` shape, as `Wrapper<T>{Inner(value: Box<T>)}`).
- [x] 1.4 **Necessary fix beyond this change's stated Impact, found during verification**: `zirk-ir/src/lower.rs`'s own `specialize_enum` has an independent local `substitute` closure with the *identical* shallow bug, and no `NOT_LOWERED` gate exists for a generic enum's nested-payload shape the way there is for classes. Leaving it unfixed would have turned the checker fix above into an internal-compiler-error / miscompile for exactly the scenario this change targets (`Wrapper.Inner(Box.Full(9))` against `Wrapper<Int32>`), rather than the correctness improvement intended — a regression the proposal's "no breaking changes" claim didn't anticipate. Fixed via a new `substitute_generic_type` (lookup-based, since `checked` is immutable by lowering time, but any needed instance is guaranteed already interned by the checker's own fixed `substitute`), wired into `specialize_enum`. `specialize_class` deliberately left untouched — independently gated `NOT_LOWERED` for this shape already, with its own doc comment explaining why recursion isn't needed there.

## 2. Regression verification (this touches widely-shared inference machinery — do not skip)

- [x] 2.1 Diffed live output against `.out` for every pre-existing generic corpus fixture (`generic_enum_box`, `generic_enum_either`, `generic_class_specialization`, `result_construction_and_match`, `user_iterable`) — byte-identical, no diffs. Also confirmed via `git stash` that `generic_infer_nested_param_no_seed.zrk` genuinely fails with `E0403` without the fix and passes with it (not a fixture that happened to pass anyway).
- [x] 2.2 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` (984 passed); `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 3. Cross-cutting correctness tests

- [x] 3.1 `generic_infer_nested_param_no_seed.zrk` — a generic function taking a parameter of a nested generic instantiation type, called and used correctly.
- [x] 3.2 `generic_enum_nested_payload.zrk` — `Wrapper<T> { Inner(value: Box<T>) }`, constructed and pattern-matched, output `full:9`/`empty` confirmed against a real compiled-and-run binary.

**Two pre-existing, unrelated limitations found and deliberately routed around, not fixed (out of this change's scope)**: (1) top-level/method generic functions (`fn f<T>(...)`) are gated `E0423` — codegen for function-level generics doesn't exist yet, a known roadmap gap; (2) a nested `match`-expression used as another match arm's return value loses its inner pattern binding during IR lowering (reproduced with plain non-generic enums too, so unrelated to substitution/generics) — fixtures route around it via small helper functions instead of a nakedly-nested match.

## 4. Documentation and status sync

- [ ] 4.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — remove the "generic-instantiation-in-a-payload... blocked" note added by `fase-3-generic-enums`.
- [ ] 4.2 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 4.3 Report both the zirk-lang and zirk-lang-site revisions used.

## 5. OpenSpec close-out

- [ ] 5.1 Run `openspec validate fase-3-generic-substitution-recursion` before archiving.
- [ ] 5.2 Archive the change once implementation, tests, and documentation sync are complete.
