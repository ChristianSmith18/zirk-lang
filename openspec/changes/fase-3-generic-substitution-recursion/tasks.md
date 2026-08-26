## 1. Checker (zirk-sema)

- [ ] 1.1 Fix `Checker::substitute` (`crates/zirk-sema/src/checker.rs`, ~line 9927) to recurse into a nested `Base::EnumInstance`/`Base::ContractInstance`'s own type arguments (design D1) — prefer delegating to `substitute_type`'s existing logic; if that proves impractical given the two functions' differing signatures, port the recursive-args handling into `substitute` instead and note the residual duplication explicitly in this task's own completion note.
- [ ] 1.2 Fix `Checker::infer_type_params` (~line 9837) to infer a type parameter from an argument's actual type when the parameter's declared type nests it inside a generic instantiation, not only when the declared type is directly `Base::Param` (design D2) — a recursive-descent unification between the declared and actual type, written generally (any nesting depth), not special-cased for one level.
- [ ] 1.3 Checker unit tests: a generic function with a parameter declared `Option2<T>` called with an argument typed `Option2<Int32>` infers `T = Int32`; a doubly-nested case (`Outer<Inner<T>>`) infers correctly (design's own flagged risk — do not skip this depth); a generic enum's variant payload naming another generic instantiation (`Bar<Baz<T>>`) now constructs correctly.

## 2. Regression verification (this touches widely-shared inference machinery — do not skip)

- [ ] 2.1 Run the full existing generic-call/generic-enum/generic-class corpus (every existing `.zrk` fixture exercising a generic function, method, class, or enum) and confirm identical output — not just "still compiles." List which fixtures were checked in this task's own completion note.
- [ ] 2.2 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 3. Cross-cutting correctness tests (do not skip)

- [ ] 3.1 End-to-end `.zrk` fixture: a generic function taking a parameter of a nested generic instantiation type, called and used correctly.
- [ ] 3.2 End-to-end `.zrk` fixture: `enum Wrapper<T> { Inner(value: Option2<T>) }` (or an equivalent nested-generic-enum-payload shape) constructed and pattern-matched.

## 4. Documentation and status sync

- [ ] 4.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — remove the "generic-instantiation-in-a-payload... blocked" note added by `fase-3-generic-enums`.
- [ ] 4.2 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 4.3 Report both the zirk-lang and zirk-lang-site revisions used.

## 5. OpenSpec close-out

- [ ] 5.1 Run `openspec validate fase-3-generic-substitution-recursion` before archiving.
- [ ] 5.2 Archive the change once implementation, tests, and documentation sync are complete.
