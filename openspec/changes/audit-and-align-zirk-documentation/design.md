## Context

See `proposal.md` for the motivation. The repository has 440 Markdown files under `docs/`, plus maintained project guidance, current OpenSpec capabilities, and a companion site. Delivery evidence is distributed across compiler/runtime code, tests, GitHub Actions, the status catalog, handbook chapters, and source packages. Some status pages are demonstrably stale: the collection handbook says sequence families are absent while the current feature catalog records them as delivered; the standard-library index still calls JSON unimplemented although `stdlib/std/json` and passing CLI fixtures exist; and package availability differs from compiler-known language surfaces. Archived change artifacts also contain deliberate historical snapshots and must not be rewritten as current documentation.

## Goals / Non-Goals

**Goals:**

- Establish an auditable claim inventory for maintained documentation, including the claim, canonical owner, implementation evidence, target scope, and disposition.
- Resolve contradictions by the repository's authority order, then refresh all affected handbook, index, status, limitation, roadmap, example, and website content.
- Keep explicit boundaries visible: normative contract versus delivered implementation, native execution versus object emission, and available Zirk APIs versus importable packages.

**Non-Goals:**

- Change language semantics, compiler/runtime implementation, standard-library APIs, or roadmap decisions.
- Rewrite archived OpenSpec changes, old decisions, historical run records, or vendor documentation to imply they described today's implementation.
- Promote a feature to delivered based only on a specification, a source file, a checked-in fixture, or completed OpenSpec tasks.

## Decisions

### Use the normative authority chain for meaning and executable evidence for delivery

Resolve semantic wording in this order: `docs/ZIRK_SPEC_FINAL.md`; the consolidated semantic owners (`CORE_LANGUAGE_SEMANTICS.md`, error/resource and memory semantics, and decorator semantics); unsuperseded specialized specifications; then handbook teaching material. For implementation status, verify the status-catalog claim against the relevant current code path and an executable focused test or CI result. Record test host and whether evidence is native execution, emulated execution, or object emission. A conflict not resolvable from those sources remains an explicitly listed decision rather than being silently chosen.

Alternative considered: treating the feature-status catalog as the semantic source of truth. Rejected because it reports implementation and cannot override normative behavior contracts.

### Audit maintained content comprehensively and classify historical content separately

Inventory every maintained Markdown file in `docs/`, top-level contributor/project guidance, current `openspec/specs/`, and the current standard-library source tree. Use repository-wide searches for dated status language, “not implemented” claims, removed names, target/host qualifiers, references to completed or active changes, and duplicated API signatures; manually inspect each match in context and cross-read the canonical owner. Inspect archived OpenSpec artifacts for links or labels that falsely present them as current, but preserve their historical content.

Alternative considered: limit the review to the handbook pages already known to conflict. Rejected because duplicated status claims outside those pages would continue to mislead users.

### Make evidence boundaries explicit in every status surface

Status entries must distinguish: target contract, delivered executable behavior, partial behavior, future/active work, and explicit exclusion. A native-host test proves that host only; object generation for a target proves emission only; emulation is labeled as emulation. A public example presented as runnable must pass the current Zirk binary. Where status depends on a focused CI run, cite its run and revision; where no evidence can be recovered, narrow the claim or mark it unverified.

Alternative considered: retain broad “supported cross-platform” statements and link to general CI. Rejected because a green focused job or object-emission matrix does not prove native runtime behavior on every target.

### Update source documentation before synchronizing the site

Correct and validate the authoritative source documents first, commit the reviewed source revision, then run `./scripts/sync-website-content.sh`. Review the generated site diff and its owned status catalog against the same evidence; supply `--audit-date YYYY-MM-DD` when status evidence changes. Validate and commit `zirk-lang-site` separately, recording both revisions.

Alternative considered: hand-edit generated site content. Rejected because it bypasses the source digest, revision pin, and generated-import validation.

## Risks / Trade-offs

- [The full documentation corpus contains intentional repeated explanations and old snapshots] → Inventory all maintained claims, classify archival/history once, and update canonical owners plus public copies without rewriting history.
- [Available source or tests may not establish a platform or API claim] → Narrow the claim, label it unverified, or create a follow-up implementation gap; do not infer support.
- [Resolving one stale statement may expose a genuine unresolved language decision] → Record it as an explicit open decision and keep the affected documentation conditional until the owner decides.
- [Website status can drift after source edits] → Sync only from the committed source revision, review the generated diff, and validate/commit the companion repository separately.
