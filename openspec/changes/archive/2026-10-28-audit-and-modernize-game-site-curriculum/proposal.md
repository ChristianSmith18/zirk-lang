## Why

The playable curriculum in `../zirk-lang-game-site` teaches a language model that has since changed: it presents removed `record` declarations as current syntax, describes classes and interfaces as absent, and uses the removed `Map.get_or_null` API. Its lesson order also introduces maps and sets before learners have met tuple/value semantics, reference sharing, cloning, and equality, which makes the collection contracts harder to understand. Because the game runs a separately packaged browser interpreter and duplicates Spanish curriculum content into generated JSON and English lesson copy, the current course must be audited against both current Zirk and the exact runtime learners receive.

## What Changes

- Audit every learner-facing and curriculum-owning surface in `zirk-lang-game-site`: the canonical curriculum and export, lesson and journey copy, story-block ordering, course/sector summaries, level screens, diagnostics/help, reference/history pages, curriculum tooling, route/progress behavior, and tests.
- Replace current-tense teaching of removed `record`/`interface` declarations with the current `Object` and `contract` model only where the exact learner browser runtime supports those examples; retain classes, inheritance, traits, and exceptions as explicitly out-of-scope topics for this foundations course if that remains the teaching goal.
- Correct stale API and semantic claims, including `Map.get_or_null`, equality/value/reference explanations, tuple capabilities, type and diagnostic names, and claims about the language versus the narrower course scope.
- Reorder Sector 5 so prerequisites come before dependent ideas: scalar value-copy and tuples; fixed/resizable collections; reference sharing, clone, and equality; then Map/Set; then parameter behavior and the boss. Update the canonical curriculum order and the separate story-block sequence together. Keep level IDs and routes; preserve or migrate saved story-step progress as well as coding-level progress.
- Replace the obsolete “record” lessons with a staged explanation of open `Object` values and structural `contract` views, then continue with traditional enums, payload enums, and `Result`. Keep the science-fiction story and “Archive” setting; distinguish that narrative from claims about Zirk's historical syntax.
- Refresh English UI copy and journey briefings from the canonical Spanish course source, export `levels.json` using the supported exporter, and keep all 53 lesson IDs playable.
- Verify every revised starter, solution, prediction, diagnostic, mistake, and hint against the exact browser package shipped by the game. The checked-in generated provenance pins the package to compiler revision `c9f0ba0de5659eb7fa307d667761db906d177a27` (acquired 2026-10-10), while the current compiler checkout is at `59275f292ec3059a6717ffa03ae8d7a323762a92`; compare the intervening behavior and refresh through the documented compiler handoff if required. Native compiler checks alone are not browser-runtime evidence.
- Preserve browser coverage reports as dated evidence; update current-facing support documentation to the current browser interpreter contract and mark historical matrices as historical.
- Validate curriculum export/parity, lesson and progression behavior, persisted progress, direct routes, accessibility, the site test suite, and production build; commit game-site changes separately from `zirk-lang` changes.

### Confirmed audit findings to resolve

- `docs/CURRICULUM.md` describes `record` as current syntax and says classes/interfaces do not exist. The current compiler model removed `record`/`interface`; current language docs distinguish `Object`, structural `contract`, and nominal classes.
- The Map lesson calls `get_or_null`; current `Map.get` is the API and a focused current `zirk check` accepts a nullable-context `m.get(key) ?? fallback`.
- The curriculum says tuples cannot be indexed, while the current handbook supports compile-time literal tuple indexing. A focused `zirk check` rejects a tuple return type, so keep that limitation separate from supported literal indexing and explicitly reject the old `.0` notation.
- The game's current-facing browser interpreter matrix is pinned in prose to IR revision `7183e15`; the compiler repository now has a matrix reviewed on 2026-10-28 that includes the current `Object`/contract/tuple model. The game's copy of `browser-oop-coverage.md` also lacks the historical-snapshot banner present in the compiler source dated 2026-10-23. Sync these current documentation sources through the repository's documented website-content workflow rather than maintaining divergent copies.
- The site does have generated provenance in `src/app/game/runtime/browser-compiler-provenance.ts`, including artifact hashes and compiler revision `c9f0ba0…`; that package predates the current compiler checkout by 184 commits. Compare actual supported behavior before deciding whether to refresh it.
- The current Zirk List chapter says `List is` distinguishes two instances, but `target/debug/zirk check` rejects `a is b` for `List<Int32>` with `E0403`; the game's finding that `List` has no observable `is` identity matches that binary. This conflicts with current Zirk documentation and must be resolved against normative semantics and focused tests before teaching `is` or changing the game's example.
- Reordering also affects `src/app/game/levels/journeys/course-story.ts`: it groups coding levels into 29 ordered story blocks, generates stable-looking block IDs from position, and maps legacy journey routes to specific block IDs. Learning-step completion keys include the block ID, so preserve those IDs or add an explicit migration when regrouping the story.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a teaching-content, curriculum-order, runtime-evidence, and companion-site maintenance change; it does not change Zirk language semantics or compiler APIs. `.openspec.yaml` sets `skip_specs: true`.

## Impact

- Companion repository `../zirk-lang-game-site`: `docs/CURRICULUM.md`, generated `assets/curriculum/levels.json`, `src/app/game/levels/` Spanish/English lesson mappings and journey copy, `docs/reference/` browser support records, curriculum exporter/verifier, curriculum tests, route/progress logic if presentation order exposes assumptions, and E2E/accessibility tests.
- Compiler repository `zirk-lang`: current language specifications/status and the browser package handoff used as evidence; rebuild or package refresh only if needed to make the shipped learner runtime match current supported lessons.
- Existing level IDs are externally observable through URLs and saved local/cloud progress. Reordering must preserve those IDs or provide and test an explicit migration; the default is to preserve them.
