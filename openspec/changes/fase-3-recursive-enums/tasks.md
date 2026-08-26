## 1. Checker (zirk-sema)

- [ ] 1.1 Split `Checker::declare_enum` (`crates/zirk-sema/src/checker.rs`, ~line 3261) into `register_enum` (duplicate-name/native-name checks, mint type-parameter ids the same way the current code already does, push a placeholder `EnumType` with empty `variants` into `self.enums` immediately) and `declare_enum_variants` (resolve each variant's associated field types and populate the placeholder). Confirm the actual current top-level declaration-pass driver (whatever calls `declare_enum` today) and wire the two new phases into the same two-pass structure `register_class`/`declare_class_members` already use for classes — verify the exact ordering, don't assume it mirrors classes' own without checking.
- [ ] 1.2 Audit every other reader of `self.enums` for an assumption that `variants` is always populated the moment an `EnumType` exists (design's own flagged central risk) — confirm nothing runs between `register_enum` and `declare_enum_variants` that would observe a still-empty `variants` list incorrectly.
- [ ] 1.3 Checker unit tests: a non-generic self-referencing enum (`IntList`) declares successfully; a generic self-referencing enum (`Tree<T>`) declares successfully; a class and an enum that reference each other (in either declaration order) both resolve; an enum declared before vs. after a class it references both work identically.

## 2. IR/lowering verification (design predicts no change needed for the direct self-reference case — confirm, don't assume)

- [ ] 2.1 Verify `specialize_enum` correctly specializes and terminates for `Tree<Int32>` — confirmed via an explicit test asserting exactly one `EnumLayout` is produced for `Tree<Int32>` regardless of how many times it's referenced recursively within itself (design D2).
- [ ] 2.2 IR-level test: a non-generic recursive enum (`IntList`) lowers correctly (construction and pattern match over at least 3 levels of `Cons`).

## 3. Cross-cutting correctness tests (do not skip — this is the test `fase-3-generic-enums` originally planned and never reached)

- [ ] 3.1 End-to-end `.zrk` fixture: construct a small `IntList` (at least 3 elements) and sum it via recursive pattern matching — confirm the correct sum.
- [ ] 3.2 End-to-end `.zrk` fixture: construct a small `Tree<Int32>` (at least 2 levels deep) and compute something over it recursively (e.g. a sum of all node values) via pattern matching — confirm the correct result. This is the single most important test in this change.
- [ ] 3.3 End-to-end `.zrk` fixture: a class and an enum referencing each other, both constructed and used correctly.
- [ ] 3.4 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 4. Documentation and status sync

- [ ] 4.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — remove the "self-referencing enum declaration... blocked" note added by `fase-3-generic-enums`.
- [ ] 4.2 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 4.3 Report both the zirk-lang and zirk-lang-site revisions used.

## 5. OpenSpec close-out

- [ ] 5.1 Run `openspec validate fase-3-recursive-enums` before archiving.
- [ ] 5.2 Archive the change once implementation, tests, and documentation sync are complete.
