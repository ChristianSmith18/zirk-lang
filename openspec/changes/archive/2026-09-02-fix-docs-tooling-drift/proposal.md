## Why

A recent project-state survey (docs, specs, frontend, backend, CLI/CI) found that the compiler is green — it builds, passes 995 tests, `fmt`/`clippy` are clean — but the entry-point documentation and the local verification process have drifted from that real state. `README.md` still announces "Phase 1 complete" while the roadmap and the code itself already cover phases 2 through 4e; the ADR index contradicts the state of its own ADRs (ADR-003 closed but listed as open, ADR-015 missing); `docs/init/ZIRK_AGENT_PROMPT.md` and `ZIRK_ROADMAP.md` disagree on whether Phase 4d is complete; and `scripts/check-local.sh` does not reproduce CI's step order, so it can report success — or failure — differently from what CI decides.

This is not cosmetic: a contributor or an agent who trusts `README.md`, the ADR index, or `check-local.sh` as a source of truth makes decisions about a version of the project that does not exist — they can redo work that is already done, reopen a decision that is already closed, or treat as "verified locally" a state CI would reject. `zirk-feature-phasing` already requires that no documented feature go without an owning phase and that the pending-type table not lie about the language as it stands today; this proposal extends that same honesty discipline to the documents that describe the project's own state, and fixes the single process gap (local verification vs. CI) found during the survey.

The deeper engineering gaps found in the same survey (`String`/`Char` lifetime outside the collector, `static mut`/`thread_local!` in the runtime ahead of Phase 5, the size of `lower.rs`/`emit.rs`, the scope of `value class`, `in`/`out` variance checking) are not resolved here: each needs its own design decision (possibly an ADR) and its own change, following `CONTRIBUTING.md`'s rule against mixing unrelated feature work. This proposal records them as owned debt with a follow-up ticket instead of implementing them.

## What Changes

- Fix `README.md` to state the real status (Phase 4e in progress) and explicitly list which language subset already works today, linking to the detailed source of truth.
- Fix `docs/decisions/README.md`: mark ADR-003 as closed, add the missing ADR-015 entry, and align the portability-verification wording with what ADR-004 already reports.
- Resolve the Phase 4d contradiction between `docs/init/ZIRK_ROADMAP.md` and `docs/init/ZIRK_AGENT_PROMPT.md`: determine the correct status (investigated against code/tests before deciding) and bring both documents into agreement.
- Create a central, versioned feature-status catalog (`docs/init/ZIRK_FEATURE_STATUS.md` or equivalent) recording, per language feature: owning phase, recognized by the lexer, parsed, semantically checked, lowered to IR, supported by backend/runtime, available through the CLI. `README.md`, the roadmap, and the agent prompt reference this catalog instead of restating status in their own words.
- Fix `scripts/check-local.sh` to run `cargo build --workspace` before `cargo test --workspace`, in the same order as `.github/workflows/ci.yml`, so a green local result implies the same result CI would produce.
- Audit and remove from the checker (`crates/zirk-sema/src/types.rs`) the `pending_type` entries for types already registered as native contracts (`Iterable`, `Iterator`, `Resource`), which today announce a future phase for something that already works — a conformance bugfix against the existing `zirk-feature-phasing` requirement ("the pending-type table reflects the language as it stands").
- Record as explicit technical debt, each with a proposed follow-up change (not implemented in this change): `String`/`Char` lifetime strategy outside the collector, `static mut`/`thread_local!` in `zirk-runtime` ahead of Phase 5, size/modularization of `lower.rs` and `emit.rs`, `value class` scope versus `record`, and semantic verification of `in`/`out` variance.
- If the result of this change affects public content, implementation status, limitations, or roadmap wording, sync `../zirk-lang-site` with `./scripts/sync-website-content.sh` and, if status evidence changed, review the site's own status catalog with an explicit `--audit-date`.

## Capabilities

### New Capabilities
- `project-status-integrity`: the discipline and catalog that keep `README.md`, `docs/decisions/README.md`, `docs/init/ZIRK_ROADMAP.md`, and `docs/init/ZIRK_AGENT_PROMPT.md` all describing the same current implementation status at once, with a central feature-status catalog as the single source.

### Modified Capabilities
- `toolchain-bootstrap`: adds the requirement that local verification (`scripts/check-local.sh`) reproduce the same step order as CI (`fmt` → `clippy` → `build` → `test`), so a local result cannot declare success where CI would fail.

## Impact

- **Documents affected**: `README.md`, `docs/decisions/README.md`, `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_AGENT_PROMPT.md`, new `docs/init/ZIRK_FEATURE_STATUS.md`.
- **Scripts affected**: `scripts/check-local.sh`.
- **Code affected**: `crates/zirk-sema/src/types.rs` (`pending_type` table), with no observable behavior change other than removing incorrect future-phase diagnostics for `Iterable`/`Iterator`/`Resource`.
- **No impact on**: grammar, IR, codegen, runtime (beyond the `pending_type` bugfix), CI (`.github/workflows/ci.yml` already does the right thing and is not modified).
- **Sibling repository**: `../zirk-lang-site` needs syncing if public status (current phase, limitations, roadmap) changes as a result of this change.
- **Debt recorded, not resolved here**: `String`/`Char` memory management, runtime concurrency-safety (`static mut`/`thread_local!`), size of `lower.rs`/`emit.rs`, `value class` scope, variance verification.
