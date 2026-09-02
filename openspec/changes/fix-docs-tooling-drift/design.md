## Context

A recent survey (docs, specs, frontend, backend, CLI/CI — run with parallel subagents and verified with `cargo fmt`/`clippy`/`build`/`test` in this environment) confirmed the compiler is healthy: 995 tests pass, `fmt` and `clippy` are clean, and the workspace builds with `LLVM_SYS_201_PREFIX` pointing at LLVM 20.1. The problem is not the compiler — it is that the documents and the local verification script describing its state drifted from it:

- `README.md` declares "Phase 1 complete" and lists as nonexistent generics, classes, `Result`, `match`, nullability, loops — all of which the roadmap and the tests have covered for several phases already.
- `docs/decisions/README.md` lists ADR-003 as open when `ADR-003-memoria.md` itself closed on August 24; it is missing ADR-015; and it states a portability-verification status different from what ADR-004 reports.
- `docs/init/ZIRK_ROADMAP.md` says Phase 4d is complete ("both slices shipped"); `docs/init/ZIRK_AGENT_PROMPT.md` says Phase 4d is the next phase to build.
- `scripts/check-local.sh` runs `cargo test --workspace` without a prior `cargo build --workspace`, while `.github/workflows/ci.yml` does build before testing. Tests in `crates/zirk-cli/tests/end_to_end.rs` locate the `zirk` executable and the static runtime under `cargo build`'s final output paths (`target/<profile>/zirk`, not `target/<profile>/deps/...-<hash>`), so a tree without a prior build can fail the local script for a reason unrelated to the actual state of the code.
- The checker (`crates/zirk-sema/src/types.rs`) keeps `Iterable`, `Iterator`, and `Resource` in the `pending_type` table (announcing a future phase) even though `checker.rs` already registers them as resolved native contracts — a narrow, bounded violation of the already-existing `zirk-feature-phasing` requirement that the pending-type table reflect the language as it stands.

The repository already enforces this discipline strongly in the language domain (`zirk-feature-phasing`: every documented feature has an owning phase, everything not implemented is diagnosed by naming its phase, the pending-type table does not announce false phases). What is missing is the same discipline applied to the documents describing the *project's* state, not the *language's*.

## Goals / Non-Goals

**Goals:**
- Make `README.md`, `docs/decisions/README.md`, `docs/init/ZIRK_ROADMAP.md`, and `docs/init/ZIRK_AGENT_PROMPT.md` all describe the same current implementation status at once.
- Have a single source of truth for feature status per pipeline stage, so updating status means editing one document, not four.
- Make `./scripts/check-local.sh` passing imply the same thing as CI passing, on the same commit.
- Fix the one code inconsistency found that is a narrow, low-risk bugfix (dead entries in `pending_type`), without touching the behavior of any already-supported feature.
- Leave every deeper engineering risk recorded with a name, description, and suggested follow-up change, instead of improvising a fix here.

**Non-Goals:**
- Deciding or implementing the `String`/`Char` lifetime strategy outside the collector (needs its own ADR).
- Auditing or refactoring `static mut`/`thread_local!` usage in `zirk-runtime` (belongs to Phase 5 readiness, not this process fix).
- Modularizing `lower.rs` or `emit.rs`.
- Deciding the final scope of `value class` versus `record`, or implementing `in`/`out` variance verification. Both are language design decisions, not process ones.
- Rewriting the CI matrix: `.github/workflows/ci.yml` already does the right thing and is the reference this change corrects the local script against.

## Decisions

### 1. Status catalog as single source, existing documents as consumers

**Decision:** create `docs/init/ZIRK_FEATURE_STATUS.md` as the single per-feature, per-pipeline-stage status table. `README.md`, the roadmap, and the agent prompt link to that table instead of restating status in their own wording.

**Alternatives considered:**
- *Generate the catalog automatically from the code* (e.g., extracting `Keyword::phase()` and `pending_type` from `zirk-lexer`/`zirk-sema`). This is the ideal long-term solution and is explicitly recommended in `COMPILER_IMPROVEMENT_SUGGESTIONS.md` (P1: "Centralize feature-stage status"), but it requires designing an annotation format and a build/lint step that verifies it — compiler work, not this documentation fix. It is left as the recommended direction for when that change is proposed; for now the catalog is manually maintained but single.
- *Not creating a new catalog and only fixing each document separately.* Rejected: it fixes today's symptom but does not prevent the same drift the next time a phase advances, which is exactly what already happened twice (Phase 1→4e in the README, Phase 4d in the agent prompt).

### 2. Resolve the Phase 4d contradiction by evidence, not preference

**Decision:** before editing any document, determine the real status of Phase 4d by running its test corpus (`crates/zirk-cli/tests/corpus/`, `zirk-sema/tests/typing.rs`, `zirk-parser/tests/grammar.rs`, filtered for the "callable and binding completion" constructs described in `docs/init/ZIRK_AGENT_PROMPT.md`) and reviewing the already-archived change `openspec/changes/archive/2026-08-25-phase-4d-callables`. If that change is archived with its tasks complete, Phase 4d is closed and `ZIRK_AGENT_PROMPT.md` is the document to fix; if tasks remain open or there is an active, unarchived change, `ZIRK_ROADMAP.md` is the document to fix.

**Alternatives considered:**
- *Declaring 4d "complete for its current scope" without verifying it*, repeating the ambiguous wording that caused the problem. Explicitly rejected: it is the same kind of unverified claim this proposal is trying to eliminate.

### 3. The local script is fixed by diffing against CI, not by redesign

**Decision:** fix `scripts/check-local.sh` by adding `cargo build --workspace` immediately before the `== tests ==` section, in the same place `.github/workflows/ci.yml` has it. No other step is changed and no new detection logic is introduced.

**Alternatives considered:**
- *Having `end_to_end.rs` build its own artifacts on demand* (e.g., invoking `cargo build` from the test if the binary is missing). Rejected: it changes the behavior and runtime of the test suite for every consumer (including CI), and it is unnecessary — the problem is that the local script skips a step CI already has, not that the test is poorly designed.

### 4. The `pending_type` bugfix is treated as compliance with an existing requirement, not a new feature

**Decision:** removing `Iterable`, `Iterator`, and `Resource` from the `pending_type` table in `crates/zirk-sema/src/types.rs` is specified under the already-existing `zirk-feature-phasing` spec (no new spec is written for it), because its requirement — "the pending-type table reflects the language as it stands" — already covers exactly this case. This change only adds the narrow fix and its regression test.

**Alternatives considered:**
- *Leaving it out of this change* because it touches `zirk-sema` code rather than only documentation. Rejected: it is a one-line-behavior bugfix (an incorrect future-phase diagnostic) whose verification cost is a unit test in the same style as `typing.rs`, and fixing it alongside the documentation closes the same kind of gap (claiming a future phase for something that already works) in the same logical commit.

## Risks / Trade-offs

- **[Risk] Fixing `README.md` could drift again the next time a phase advances.** → Mitigation: the new status catalog is the only table that changes per phase; `README.md` only links to it. The `project-status-integrity` requirement ("no other document restates that enumeration in its own words") makes updating the catalog sufficient.
- **[Risk] Resolving Phase 4d might require more investigation than anticipated if the archived change does not cover everything the agent prompt describes.** → Mitigation: if the evidence is ambiguous, this change explicitly leaves Phase 4d as "in progress, exact scope to confirm" in both documents instead of forcing an unverified claim, and opens a follow-up task instead of blocking the rest of the change.
- **[Risk] Touching `crates/zirk-sema/src/types.rs` in a change nominally about documentation can cause scope confusion.** → Mitigation: `proposal.md` explicitly lists that file under "Impact", and the change is narrow (removing three table entries plus one test), with no changes to type-resolution logic.
- **[Risk] `../zirk-lang-site` imports status evidence pinned to an exact commit; mixing this change with unrelated work could drag unreviewed content into the site sync.** → Mitigation: `tasks.md` includes the site sync as an explicit step after merge, with a manual `--audit-date`, as the project context requires.
- **[Trade-off] Not resolving the engineering debt (`String` memory, `static mut`, `lower.rs` size) in this change leaves those risks open longer.** → Deliberately accepted: bundling them here would violate `CONTRIBUTING.md`'s rule against mixing unrelated feature work, and would risk the current build health (995 green tests) for a much larger surface of change. They are recorded with a suggested change name in `tasks.md`.

## Migration Plan

1. Investigate and settle the real status of Phase 4d (step 0, blocks the README/roadmap/agent-prompt wording).
2. Create `docs/init/ZIRK_FEATURE_STATUS.md`.
3. Edit `README.md`, `docs/decisions/README.md`, `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_AGENT_PROMPT.md` to link to the catalog and remain mutually consistent.
4. Fix `scripts/check-local.sh` and verify, in a clean disposable working tree (`git clean -fdx`), that it reproduces CI's result.
5. Fix `pending_type` in `zirk-sema` and add the regression test.
6. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --workspace -- --test-threads=1`.
7. Sync `../zirk-lang-site` with `./scripts/sync-website-content.sh` and an `--audit-date` if public status evidence changed.
8. Open (without implementing) the follow-up changes for the recorded engineering debt.

No special rollback is needed: these are documentation changes plus one narrow, revertible bugfix, reversible with `git revert`.

## Open Questions

- Is Phase 4d's real status resolved as "complete" or "in progress"? Decided in task 0 of `tasks.md` with evidence, not here.
- Should the status catalog (`docs/init/ZIRK_FEATURE_STATUS.md`) eventually be generated from the code (`Keyword::phase()`, `pending_type`, `NOT_LOWERED`/`PENDING_FEATURE` diagnostics) in a later change? Noted as the recommended direction in Decision 1, without committing to a concrete design here.
