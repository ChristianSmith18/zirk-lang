## 1. Corpus inventory and evidence audit

- [x] 1.1 Inventory every maintained Markdown document under `docs/`, maintained top-level project guidance, current `openspec/specs/`, standard-library source packages, and public site-owned status content; classify archived OpenSpec changes, vendor docs, generated content, and historical run notes separately, and verify the inventory covers every in-scope path exactly once.
- [x] 1.2 Read every in-scope maintained document and record each stale, contradictory, ambiguous, or unsupported claim in a reviewable audit ledger with file/section, claim, canonical owner, current evidence, disposition, and required edit; verify every inventoried file has an audit outcome, including an explicit “no discrepancy found” result.
- [x] 1.3 For every implementation-status claim found, trace the API/type to its current source implementation and focused executable test or target-specific CI evidence; distinguish native execution, emulation, and object emission, and verify unsupported claims are narrowed or marked unverified rather than inferred from source presence or completed task checkboxes.

## 2. Reconcile authoritative Zirk documentation

- [x] 2.1 Reconcile type and collection claims across normative owners, feature status, current limitations, compiler specification, roadmap, and handbook, including admitted union/intersection families, generic and flow-narrowing boundaries, generic dispatch/substitution gaps, recursive enum layout, enum `Clone`, Map/Set key and element eligibility, nullable Decimal, collection package versus compiler-surface status, and the 4096-byte payload boundary; verify each remaining limitation has one consistent statement and evidence scope.
- [x] 2.2 Reconcile standard-library module and API claims against current `stdlib/` sources, compiler/runtime behavior, and tests, including collections, civil and zone-aware time, JSON, environment/secret sinks, networking/HTTP browser boundaries, Mutex and remaining synchronization, and specified-but-undelivered modules; verify the standard-library index and owning module chapters agree on delivered, partial, specified-only, and excluded status.
- [x] 2.3 Correct stale or conflicting statements about Deque/Queue/Stack/PriorityQueue, JSON, networking, `Mutex`, `Clone`, removed `record` declarations, package availability, and target evidence, plus every additional discrepancy discovered by the corpus audit; verify each correction against its canonical owner and the audit ledger.
- [x] 2.4 Review normative specifications, examples, indexes, cross-links, diagnostics, tutorials, onboarding, and contributor guidance for claims affected by the audit; run the current Zirk binary on every example newly asserted to be executable and verify old examples are either corrected, labeled normative-only, or explicitly unsupported.

## 3. Synchronize the companion website

- [x] 3.1 Commit the reviewed source documentation and audit ledger in `zirk-lang`, then run `./scripts/sync-website-content.sh` from the exact committed source revision; verify both source descriptors pin that revision and generated imports/checksums match.
- [x] 3.2 Review `zirk-lang-site` generated content and its owned status catalog against the audited source claims; if status evidence changed, use a reviewed date with `--audit-date YYYY-MM-DD`, then run content validation, content verification, site tests, and the production build.
- [x] 3.3 Commit the reviewed site synchronization separately and verify the source and site repositories each record the intended revision; ensure the site tree is clean and preserve any unrelated user changes in the source repository.

## 4. Final consistency verification

- [x] 4.1 Run strict OpenSpec validation, documentation example formatting/validation, link and route checks, site content checks, and site build; verify all pass or record specific pre-existing infrastructure failures without claiming a clean result.
- [x] 4.2 Re-scan the maintained documentation corpus for all audited stale-status patterns and known contradictions; verify every hit is resolved, intentionally historical, or linked to a documented open decision, and confirm the audit ledger has no unreviewed entries.
