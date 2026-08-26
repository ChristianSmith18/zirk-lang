## 1. Checker (zirk-sema)

- [ ] 1.1 Remove the `not_lowered` gate for `record`/`value class` in `reject_unstructured_comparison` (`crates/zirk-sema/src/checker.rs`, ~line 6528-6548).
- [ ] 1.2 Confirm the exact current rule for a bare `class`-typed field's own `==` (identity via `is`, or the referenced class's own `_equals` if declared) by reading the actual current checker behavior (design D2 — do not restate an assumed rule, verify it).
- [ ] 1.3 Checker unit tests: `==` on two same-shape `record`/`value class` values type-checks and no longer emits `NOT_LOWERED`; a nested `record`/`value class` field is accepted; a `class`-typed field inside a `record`/`value class` uses whatever the confirmed rule (1.2) says.

## 2. IR (zirk-ir)

- [ ] 2.1 Lower `Eq`/`NotEq` on a `record`/`value class` operand (with no user `_equals`) to a conjunction of per-field comparisons (design D1) — confirm the actual current IR shape for logical `&&`/short-circuiting in this codebase and reuse it exactly, rather than inventing a new conjunction shape.
- [ ] 2.2 Recurse correctly into a nested `record`/`value class` field (its own comparison is generated the same way, not flattened into the parent's field list).
- [ ] 2.3 `!=` reuses the same generated comparison, negated — confirm against the actual current `Eq`/`NotEq` lowering split (`lower.rs:8034` and its surrounding code) rather than assuming a specific shape.
- [ ] 2.4 IR-level tests: a flat `record`/`value class` with only scalar fields lowers to the expected chain of per-field comparisons ANDed together; a nested case recurses correctly; the generated comparison short-circuits (confirm via an explicit test using a field whose comparison would be observably skipped, e.g. a side-effect-free but distinguishably-ordered check, not just asserting the IR shape).

## 3. Codegen (zirk-codegen-llvm) — only if lowering to existing IR shapes isn't already sufficient

- [ ] 3.1 If task 2.1-2.3 lowers entirely to already-existing IR instruction kinds (`Binary { op: Eq }`, existing logical-AND shape), confirm codegen already handles the resulting IR correctly with no new codegen work needed — verify with a real compiled-and-run test, not by inspection alone. If a genuinely new IR shape turns out to be necessary, document why here before implementing it.

## 4. Cross-cutting correctness tests (do not skip)

- [ ] 4.1 End-to-end `.zrk` fixture: two `record` values with identical field values compare equal; two `record` values differing in exactly one field compare unequal.
- [ ] 4.2 End-to-end `.zrk` fixture: same for `value class`.
- [ ] 4.3 End-to-end `.zrk` fixture: a `record`/`value class` with a nested `record`/`value class` field — confirm the nested field's own equality is what's being compared (a nested field differing alone makes the whole comparison `false`).
- [ ] 4.4 End-to-end `.zrk` fixture: `!=` produces the correct negated result for both an equal and an unequal pair.
- [ ] 4.5 Confirm short-circuiting for real: a `record` with several fields differing only in the first one — this must not require evaluating every field for the overall result to be correct (a targeted test if the language surface allows observing evaluation order; otherwise document why this could only be confirmed at the IR level in task 2.4, not end-to-end).
- [ ] 4.6 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 5. Documentation and status sync

- [ ] 5.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — drop "derived structural equality" from the "Several Phase 3 constructs..." bullet/row once delivered.
- [ ] 5.2 Check `docs/handbook/02-handbook/05-operators-and-expressions/03-structural-equality.md` for any stale "not yet implemented" caveat — reconcile.
- [ ] 5.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 5.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 6. OpenSpec close-out

- [ ] 6.1 Run `openspec validate fase-3-structural-equality` before archiving.
- [ ] 6.2 Archive the change once implementation, tests, and documentation sync are complete.
