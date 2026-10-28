## Context

See `proposal.md` for the discovered content drift and scope. The playable course has two content channels: `docs/CURRICULUM.md` is the authoring source and `assets/curriculum/levels.json` is exported; `src/app/game/levels/` supplies English lesson/journey copy and UI model data. Routes and local/cloud progress use the existing numeric level IDs. The course verifier and the browser package used by learners are separate execution paths, so success in one is not proof of success in the other.

The current 53-level course moves from scalar basics through control flow and functions, then presents Array/List/Map/Set before tuples and before its explicit value/reference/equality lessons. Its final “Archive” sector teaches removed `record` declarations and a record value/equality model. The current map exercise names `get_or_null`, while current Zirk uses `Map.get` with `Result` or nullable projection. Current language documentation and compiler browser support evidence are the semantic authorities; the shipped browser package is the execution authority for playable examples.

The audit has also established two claims that need careful reconciliation instead of a direct copy edit. Current Zirk tuple documentation supports compile-time literal indexing, contradicting the course's broad claim that tuples cannot be indexed; a focused CLI check rejects tuple return syntax. Separately, the current List handbook presents `a is b` as an identity comparison, but the current CLI rejects that expression for `List<Int32>` with `E0403`. The game's finding that `List` has no `is` identity matches the compiler result. Resolve the authoritative language behavior before updating either teaching source. Browser provenance is available in generated `browser-compiler-provenance.ts`: the checked-in package comes from compiler revision `c9f0ba0…`, while the current compiler checkout is 184 commits later. The game's matrix still cites older revision `7183e15`, whereas the compiler repository has a matrix reviewed on 2026-10-28; its copy of the 2026-10-23 historical OOP report also omits the historical banner present in the source. Sync these docs through the canonical website-content workflow.

## Goals / Non-Goals

**Goals:**

- Teach a coherent progression in which each lesson's prerequisites have already appeared, while retaining the existing 53 playable level identities.
- Align Spanish source, exported JSON, English UI copy, narrative briefings, diagnostics, and current-facing support documentation.
- Test learner-facing programs against the exact browser package the game loads and record its compiler revision.
- Protect stable URLs, local/cloud completions, sector rewards, direct links, and final-level calculations while changing presentation order.
- Keep the story's archive setting and distinguish fictional narrative from historical language claims.

**Non-Goals:**

- Expand this foundations course into a complete reference for advanced Zirk features such as all generic dispatch cases, unions/intersections, zoned time, or advanced JSON.
- Teach classes, inheritance, traits, or exception syntax in this course unless a separate curriculum decision expands its learning outcomes. The text may say those topics are outside this course; it must not claim the language lacks them.
- Change Zirk semantics or silently claim browser support from native compiler behavior.
- Renumber level IDs or discard learner progress.

## Decisions

### Keep the Markdown curriculum canonical

Edit `docs/CURRICULUM.md`, then run `curriculum:export` to regenerate `assets/curriculum/levels.json`. Never patch the generated JSON manually. Treat `src/app/game/levels/` as a second presentation surface that must be reconciled and tested against the canonical lesson IDs and concepts. Update hand-written English text where the mapping is intentional; where copy is merely a duplicate, prefer deriving it from the canonical model rather than creating another source of drift.

Alternative considered: edit the JSON because it is consumed at runtime. Rejected because it is generated and verification will overwrite the edit.

### Reorder ideas without changing learner identity

Keep every existing level `id` as the stable route and local/cloud progress key. Reorder source sections and the exported levels array to define the new coding progression; update `course-story.ts` groups in the same order because story briefings interleave with levels and progression checks depend on both sequences. Retain `5.13` as the sector boss/final item. If the UI needs a human-facing ordinal, model it separately from `id`; do not derive or display a misleading ordinal from the legacy route key. Story block IDs and `${blockId}:${stepId}` completion keys are also persisted. Keep the existing group count/positions and explicit mappings where practical; if regrouping changes a block or learning-step ID, add and test a migration and preserve legacy journey routes. Add regression coverage for direct routes and existing coding/story completions before and after the reorder.

Recommended Sector 5 sequence, with stable IDs shown here only as mapping aids:

1. `5.6` — scalar values copy on assignment.
2. `5.5` — tuples group and destructure values.
3. `5.1` — fixed-size `Array`.
4. `5.2` — resizable `List`.
5. `5.7` — list assignment shares a reference.
6. `5.8` — `clone()` makes an independent list copy.
7. `5.9` — String sharing and cloning.
8. `5.10` — content equality versus reference identity.
9. `5.3` — `Map`, using only verified key/value types and current `get` semantics.
10. `5.4` — `Set`, using only verified element types.
11. `5.11` — value/reference behavior at function-call boundaries.
12. `5.12` — read-only parameter bindings.
13. `5.13` — integrated boss.

This order puts equality and reference behavior before hash collections while leaving the sector's prerequisite ideas intact. Update the four Sector 5 story blocks to group the new order: (a) scalar copy, tuples, Array, List; (b) aliasing, clone, String sharing, equality; (c) Map, Set, function calls, read-only parameters; (d) boss. Preserve block IDs and legacy journey aliases, or explicitly migrate their progress if the story grouping changes. If package/runtime tests show a concept is not available in the learner browser, retain its place as a marked non-playable reading only after an explicit course decision; do not silently substitute a weaker example.

### Replace record lessons with today's data model, not a superficial rename

Keep level IDs `6.1` and `6.2`, but redesign their learning outcomes: first teach open, identity-bearing `Object` data and field access; then teach a named structural `contract` view over a compatible object. Do not call either a record or imply value equality/immutability. Present the Archive progression as open named objects and structural views, then closed enum states and payloads, then `Result` for success/failure; this gives the sector a clear open-data → closed-alternatives → explicit-outcome sequence. Keep the Archive narrative and explain that its name is fictional; where the story refers to historical language designs, label those claims by version/date and link current syntax separately.

Alternative considered: rename `record` to `Object` while retaining the immutable-value and content-equality exercises. Rejected because it would teach false semantics and create invalid equality expectations.

### Verify semantics and browser execution independently

First verify the generated browser provenance record against every checked-in artifact hash, then compare its pinned compiler revision (`c9f0ba0…`) with the current source and support catalog. If it does not contain the intended constructs or lesson behavior, use the documented `zirk-lang` browser-package handoff from a clean, committed compiler revision, verify package smoke behavior, and update the companion asset/provenance in its own repository. For each revised runnable program, require both (a) the curriculum verifier/current CLI evidence needed for diagnostics and lesson cases and (b) execution through the exact browser API/package used by the game. Record revision, target, and verification mode. A native `zirk run`, a local interpreter example, generated JSON, or stale checked-in WASM alone is not sufficient evidence for the player experience.

Do not collapse language semantics, compiler acceptance, and browser-package support into one claim. In particular, resolve the current `List is` documentation/compiler conflict against the normative spec and focused tests before deciding whether to correct the handbook, compiler behavior, or course wording. Keep tuple literal indexing separate from tuple return-type support and verify each claim independently. If the language intentionally makes List identity unobservable through `is`, teach aliasing through mutation effects and avoid presenting `a is b` as a playable example.

### Update historical support material without rewriting its evidence

Use the compiler repository's current browser matrix and historical OOP report as canonical website content; sync them through `./scripts/sync-website-content.sh` after satisfying its committed-source and clean-site preconditions. Keep the dated 2026-10-23 OOP audit's results intact, including its historical banner, and link it to the current support contract. The browser matrix must distinguish language support from interpreter inclusion, exact package support, and site policy.

### Keep the course scope honest

The opening should describe this as a Zirk foundations course, not as a language subset that lacks delivered features. It may explicitly defer classes, traits, inheritance, exceptions, or advanced standard-library work to later material. The error glossary and “limits” section must separate (1) current language/compiler behavior, (2) browser-interpreter exclusions, (3) course omissions, and (4) historical findings. Do not teach unstable APIs or incomplete collection type combinations as universal guarantees.

## Risks / Trade-offs

- [Changing order may change unlock and final-level behavior] → Preserve IDs, keep `5.13` last, update ordering-dependent tests, and verify old local/cloud completions and direct routes.
- [New Object/contract lessons may exceed the checked-in browser package] → Establish package provenance first and make package refresh a prerequisite; do not make a lesson playable until the exact package passes it.
- [Spanish authoring and English UI copy may drift again] → Add parity checks for title/concept/goal/hints and require explicit review for story adaptations.
- [Historical evidence may be mistaken for a current claim] → Date and label reports; keep historical rows unchanged and point readers to the current matrix.
- [Modernization could unintentionally broaden the course] → Retain the foundations-course boundary and defer advanced features instead of presenting every newly delivered API.

## Migration Plan

1. Capture current level IDs, order, routes, sector rewards, saved completion IDs, and the browser package's provenance before editing.
2. Build a lesson dependency/outcome table and approve the new pedagogical order, stable-ID mapping, and scope text.
3. Update canonical Spanish curriculum and current-facing docs; export the JSON; update English lesson and narrative copy.
4. Refresh/verify the browser package if required, then run every lesson through curriculum and browser-level validation.
5. Run progression, local/cloud progress, routing, accessibility, full site tests, and production build. If behavior regresses, revert the order/content export and package handoff together; preserve the stable-ID scheme.
6. Commit any required compiler package provenance/handoff first in `zirk-lang`, then commit game-site curriculum and generated artifacts separately. No public deployment is included.
