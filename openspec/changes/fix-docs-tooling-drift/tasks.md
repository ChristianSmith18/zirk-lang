## 0. Investigate before writing (blocks tasks 2 and 3)

- [x] 0.1 Review `openspec/changes/archive/2026-08-25-phase-4d-callables` (proposal, tasks, specs) and confirm whether its tasks are 100% complete and archived with nothing pending.
- [x] 0.2 Run the test subset related to "callable and binding completion" (`cargo test -p zirk-sema -p zirk-parser -- callable`, and review `crates/zirk-cli/tests/corpus/valid` and `invalid` for closure/binding cases from Phase 4d) to verify behavior with evidence, not just wording.
- [x] 0.3 Decide the real status of Phase 4d ("complete" or "in progress, scope X pending") from the evidence in 0.1 and 0.2, and write that decision in a short note inside this change folder (e.g. `4d-status-decision.md`) citing the files and lines reviewed.
- [x] 0.4 Review `ADR-004-portabilidad.md` in full (not just the summary) to confirm whether CI portability verification is pending or already covered, and note the conclusion in the same note from 0.3.

## 1. Single feature-status catalog

- [x] 1.1 Create `docs/init/ZIRK_FEATURE_STATUS.md` with a table per feature: name, owning phase (per `docs/init/ZIRK_ROADMAP.md`), recognized by the lexer (yes/no), parsed (yes/no), semantically checked (yes/no), lowered to IR (yes/no), supported by backend/runtime (yes/no), available through CLI/tooling (yes/no).
- [x] 1.2 Populate the table from real evidence: `Keyword::phase()`/`TokenKind::phase()` in `crates/zirk-lexer/src/token.rs`, `NOT_IMPLEMENTED`/`PENDING_FEATURE`/`NOT_LOWERED` diagnostics in `zirk-parser`/`zirk-sema`, and the tests in `zirk-ir/tests/lowering.rs`, `zirk-codegen-llvm/tests/emission.rs`, `zirk-cli/tests/end_to_end.rs`.
- [x] 1.3 Explicitly include in the table the Phase 4d status settled in task 0.3.
- [x] 1.4 Add a closing note about the recommended future direction (generating this table from the code instead of maintaining it by hand), without implementing it.

## 2. Synchronize the project's status documents

- [x] 2.1 Edit `README.md`: replace "Estado: Fase 1 completa" with the real current phase; replace the "does not yet exist" list with what is actually still missing today; add a link to `docs/init/ZIRK_FEATURE_STATUS.md` as the detailed source.
- [x] 2.2 Edit `docs/decisions/README.md`: mark ADR-003 as closed/accepted citing the closing date from `ADR-003-memoria.md`; add the missing ADR-015 entry; fix the portability-verification wording to match the conclusion from task 0.4.
- [x] 2.3 Edit `docs/init/ZIRK_ROADMAP.md` and `docs/init/ZIRK_AGENT_PROMPT.md` so both declare the same Phase 4d status settled in task 0.3, and so both reference `docs/init/ZIRK_FEATURE_STATUS.md` instead of restating per-feature status detail.
- [x] 2.4 Review `docs/handbook/00-getting-started/03-language-status.md` (flagged in the survey as misaligned) and fix it if it repeats the same kind of contradiction.
- [x] 2.5 Search (`grep`) for any other mention of "Fase 1 completa" or of a specific phase status outside `docs/init/ZIRK_FEATURE_STATUS.md`, and fix it or turn it into a link to the catalog.

## 3. Fix local verification

- [ ] 3.1 Edit `scripts/check-local.sh` to insert `cargo build --workspace` (with a `== build ==` section message, following the existing `blue`/`green` style) immediately before the `== tests ==` section.
- [ ] 3.2 Verify, in a clean working tree (disposable copy with `target/` removed), that `./scripts/check-local.sh` builds before testing and that the tests in `crates/zirk-cli/tests/end_to_end.rs` find the `zirk` executable and the static runtime without an "executable was not found" error.
- [ ] 3.3 Confirm that the order and flags of `scripts/check-local.sh` remain equivalent to the "Formato", "Clippy", "Build", "Tests" steps of `.github/workflows/ci.yml`.

## 4. Fix the checker's pending-type table

- [ ] 4.1 Locate the `Iterable`, `Iterator`, and `Resource` entries in `pending_type` inside `crates/zirk-sema/src/types.rs` and confirm that `crates/zirk-sema/src/checker.rs` already registers them as resolved native contracts (lines cited in the survey: `checker.rs:958`, `checker.rs:1499`).
- [ ] 4.2 Remove those three entries from `pending_type`.
- [ ] 4.3 Add a regression test in `crates/zirk-sema/tests/typing.rs` verifying that annotating a type with `Iterable<T>`, `Iterator<T>`, or `Resource<E>` does not produce a pending-phase diagnostic.
- [ ] 4.4 Run `cargo test -p zirk-sema` and confirm the existing 412 tests in `typing.rs` stay green alongside the new one.

## 5. Full verification

- [ ] 5.1 Run `export LLVM_SYS_201_PREFIX=<LLVM 20.1 prefix> && cargo fmt --all --check`.
- [ ] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 5.3 Run `cargo build --workspace`.
- [ ] 5.4 Run `cargo test --workspace -- --test-threads=1` and confirm 995+1 (the new test from 4.3) tests green, 0 failing.
- [ ] 5.5 Re-read `README.md`, `docs/decisions/README.md`, `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_AGENT_PROMPT.md`, and `docs/init/ZIRK_FEATURE_STATUS.md` together and confirm none contradicts another.

## 6. Sync with the public website

- [ ] 6.1 Commit this change's edits in `zirk-lang`.
- [ ] 6.2 Run `./scripts/sync-website-content.sh` from `zirk-lang` pointing at `../zirk-lang-site`.
- [ ] 6.3 Review the generated diff in `../zirk-lang-site` (site's own status catalog, hashes, handbook-imported content).
- [ ] 6.4 If public status evidence changed (current phase, limitations, publicly cited ADRs), approve the review with an explicit `--audit-date YYYY-MM-DD` and record who reviewed it.
- [ ] 6.5 Commit the result in `zirk-lang-site` separately, and note the corresponding `zirk-lang-site` revision/commit in the `zirk-lang` PR description.

## 7. Record the engineering debt not resolved here

- [ ] 7.1 Create a follow-up note (issue, or an entry in `docs/decisions/proximos-pasos-fase-4.md` if applicable, or a new backlog document) for: `String`/`Char` lifetime strategy outside the mark-sweep collector, with a suggested change name (e.g., `fase-4e-memoria-de-strings`).
- [ ] 7.2 Record the same way: audit of `static mut`/`thread_local!` in `zirk-runtime` ahead of starting Phase 5 (e.g., `fase-4e-runtime-thread-safety`).
- [ ] 7.3 Record the same way: modularizing `crates/zirk-ir/src/lower.rs` and `crates/zirk-codegen-llvm/src/emit.rs` (e.g., `refactor-lower-y-emit`).
- [ ] 7.4 Record the same way: the `value class` versus `record` scope decision, and semantic verification of `in`/`out` variance (e.g., `fase-3-value-class-alcance`, `fase-3-verificacion-varianza`).
- [ ] 7.5 Link the four follow-up notes from `docs/init/ZIRK_FEATURE_STATUS.md` or from `COMPILER_IMPROVEMENT_SUGGESTIONS.md`, so they are not lost as loose comments in this change.
