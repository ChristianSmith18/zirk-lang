## 1. Checker (zirk-sema)

- [ ] 1.1 Remove the `is_native` gate in `resolve_enum_reference` (`crates/zirk-sema/src/checker.rs`, ~line 4290-4305, design D1) — a generic enum instantiation is accepted for any enum, not just `Iteration<T>`/`Result<T,E>`. Confirm the surrounding constraint-checking logic (the loop just above the gate) already applies correctly regardless of which enum is being instantiated — it should, since it iterates the enum's own declared type parameters, but verify rather than assume.
- [ ] 1.2 Checker unit tests: a simple single-type-parameter user generic enum instantiates and type-checks; a multi-type-parameter enum (`Either<L, R>`) instantiates correctly; constraint violations on a user generic enum's type argument are still rejected with the existing diagnostic (unchanged behavior, just no longer gated afterward).

## 2. IR (zirk-ir) — verification, not new mechanism (design says this should mostly already work)

- [ ] 2.1 Verify `specialize_enum` (`crates/zirk-ir/src/lower.rs`, ~line 752) correctly specializes a multi-type-parameter user enum (design D2's first case).
- [ ] 2.2 Verify `specialize_enum` correctly handles a nested generic instantiation in a variant payload (design D2's second case, e.g. a variant payload naming another generic instantiation, `Bar<Baz<T>>`).
- [ ] 2.3 **Critical**: verify `specialize_enum` memoizes by instantiation identity (enum id + concrete type arguments) and correctly terminates on a recursive generic enum (design D2's third case, D1's own flagged risk: `enum Tree<T> { Leaf, Node(T, Tree<T>, Tree<T>) }`). If it does NOT already memoize correctly, this is a blocking finding — stop and report the specific gap rather than attempting a fix outside this task's own scope assessment, since the fix's shape depends on what's actually found.
- [ ] 2.4 IR-level tests for all three shapes in 2.1-2.3, each with an explicit assertion that only one `EnumLayout` is produced per distinct instantiation (not one per syntactic occurrence in source).

## 3. Cross-cutting correctness tests (do not skip)

- [ ] 3.1 End-to-end `.zrk` fixture: construct and pattern-match a simple single-type-parameter user generic enum (`enum Box<T> { Full(T), Empty }`) instantiated at two different type arguments in the same program — confirm both produce correct, independent results.
- [ ] 3.2 End-to-end `.zrk` fixture: a multi-type-parameter user generic enum (`Either<L, R>`), constructed and matched on both variants.
- [ ] 3.3 End-to-end `.zrk` fixture: a recursive generic enum (`Tree<T>`) — construct a small tree (at least 2 levels deep) and pattern-match it recursively, confirming correct values at every level. This is the single most important test in this change per design's own risk flag.
- [ ] 3.4 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 4. Documentation and status sync

- [ ] 4.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — drop "user generic... enums" from the "Several Phase 3 constructs..." bullet/row once delivered.
- [ ] 4.2 Check `docs/handbook` for any existing generic-enum teaching material with a stale "not yet implemented" caveat — reconcile.
- [ ] 4.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 4.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 5. OpenSpec close-out

- [ ] 5.1 Run `openspec validate fase-3-generic-enums` before archiving.
- [ ] 5.2 Archive the change once implementation, tests, and documentation sync are complete.
