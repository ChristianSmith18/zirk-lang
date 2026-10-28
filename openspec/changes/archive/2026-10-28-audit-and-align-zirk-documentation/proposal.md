## Why

Zirk's maintained documentation contains implementation claims that disagree across the collection handbook, standard-library index, feature-status catalog, current limitations, normative specifications, and compiler evidence. Readers cannot reliably distinguish delivered behavior, partial support, future contracts, and explicit exclusions; a repository-wide evidence-led audit is required before these documents can be trusted as a delivery inventory.

## What Changes

- Audit all maintained Markdown documentation in `docs/`, top-level project guidance, and current `openspec/specs/` against the documented authority order and verified compiler, runtime, package-source, and target evidence.
- Commit an audit ledger that inventories every in-scope document and source-status file exactly once, records each discrepancy with its canonical owner/evidence/disposition, and marks files without a discrepancy explicitly.
- Reconcile status and terminology across normative specifications, handbook chapters, feature-status and limitation catalogs, compiler and standard-library specifications, roadmaps, indexes, examples, and the companion website.
- Correct known contradictions, including collection-family delivery, JSON and networking module status, `Mutex` availability, `Clone` support boundaries, removed `record` declarations, collection-key eligibility, and native versus object-emission evidence.
- Preserve the distinction between normative targets, delivered subsets, active work, future contracts, historical evidence, and explicit Zirk 1.x exclusions. Do not rewrite archived artifacts to make historical claims appear current.
- Require every implementation-status claim to name its evidence scope, target, source revision, and verification mode where those details affect the claim.
- Update the companion `zirk-lang-site` from the reviewed source revision, review site-owned status content, validate the imported content and build, and commit site changes separately.
- Audit the companion `zirk-lang-game-site` as a separate maintained teaching surface, including its canonical curriculum source, generated level data, lesson copy, historical framing, and beginner-facing examples; correct the proposal scope to cover its reconciliation with current Zirk syntax and delivery status.

### Follow-up discovered after the initial audit

The initial inventory did not include `../zirk-lang-game-site`. A follow-up scan
found that its curriculum still teaches removed `record` declarations and
describes the language as having no classes, interfaces, or inheritance. It
also frames those claims as the current course introduction, not as dated
history. The game site must therefore be audited and updated before the broader
documentation audit can be considered complete. Review `docs/CURRICULUM.md` and
`assets/curriculum/levels.json` together with the exporter, level validation,
exercise solutions, hints, progression, and current compiler/browser support.
Keep generated curriculum JSON synchronized from its canonical source and
verify every revised exercise against the browser package learners actually
run. Preserve intentional narrative history, but clearly distinguish it from
current language instruction. No game-site content was changed by the initial
audit commits.

### Follow-up resolution

The separate Game Site curriculum audit was completed in
`audit-and-modernize-game-site-curriculum`. Its 53-level curriculum, story and
lesson copy, generated data, package provenance, and runnable exercises were
reconciled with current Zirk syntax and browser support. The exact shipped
browser package passed the curriculum verifier (53 levels, 8 findings, 14
diagnostics, 226 runs); the site suite passed 240 tests, and the production
build prerendered 93 routes. The site was deployed to Firebase Hosting on
2026-10-28 and its live package hashes were verified. The separate change is
archived; its Game Site commits are `e3c1f6a`, `d861a63`, `34e9055`, and
`edc22f2`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change reconciles documentation with existing behavior and evidence without changing normative language behavior; `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `docs/**/*.md`, maintained top-level documentation, and current specifications under `openspec/specs/`.
- Documentation inventories, handbook navigation, implementation-status and limitation statements, examples, and links to verification evidence.
- Companion repository `../zirk-lang-site`, including synchronized handbook content, generated imports, and its status catalog.
- No compiler, runtime, standard-library API, or language semantics are changed by this documentation-only proposal.
