## Why

The Zirk handbook already explains *what* each built-in and user-defined type is called, but it rarely shows *how* each one lives in a program. The existing `03-everyday-types` and `10-data-types` pages are conceptual or per-type reference cards with uneven depth (`String` and `Duration` are rich, `List`, `Array`, `Pointer`, `Regex`, `Weak`, `Fn` are thin or absent). This makes it hard for readers to choose the right type and to predict the memory, identity, and sharing behavior of their values. We need a single, coherent deep-dive that treats every important type, with a unifying narrative instead of an alphabetical catalog.

## What Changes

- Add a new **"How Zirk values live and share"** editorial hub that walks readers through the memory/ownership spectrum from immediate values to unsafe pointers.
- Rewrite/expand `01a-type-categories.md` to include runnable examples for every category and to state the shared-copy rules in one place.
- Convert `03-built-in-types.md` from a table-only catalog into a quick-example-per-type reference.
- Add or substantially expand per-type chapters for every currently delivered or specified surface type:
  - `List<T>`
  - `Array<T>` / fixed arrays
  - `Regex`
  - `Pointer<T>` and `NativeSlice<T>`
  - `Weak<T>`
  - `Fn(...)` / `Function(...)`
  - `Duration` integration with the new narrative
- Add a new **"Choosing a type"** decision page that maps common data-shape problems to the right category.
- Update `docs/handbook/SUMMARY.md` with the new and reordered pages.
- Update `docs/init/ZIRK_FEATURE_STATUS.md`, `12-feature-status.md`, and `07-current-limitations.md` if any example or status claim changes.
- Re-sync the public website (`../zirk-lang-site`) once the handbook changes are committed.

## Capabilities

### Modified Capabilities

- `handbook-editorial-system`: Add a coherent data-type learning track with a memory-behavior narrative and example-driven per-type chapters.
- `language-documentation-information-architecture`: Reorganize the handbook's type coverage around a shared-copy / lifetime spectrum and add the missing `Regex`, `List`, `Pointer`, `Weak`, and callable type chapters.
- `zirk-data-types`: Strengthen the normative examples that illustrate primitive, native-value, native-reference, borrowed, and unsafe pointer semantics without changing the language contract.
- `project-status-integrity`: Update `feature-status.md` / `current-limitations.md` only if any page changes a public implementation-status claim; no compiler-delivery promotion without evidence.

## Impact

- `docs/handbook/02-handbook/03-everyday-types/` (expanded conceptual pages)
- `docs/handbook/02-handbook/10-data-types/` (new/expanded per-type pages)
- `docs/handbook/02-handbook/12-collections/` (deeper `List` and `Array` chapters)
- `docs/handbook/02-handbook/17-memory-and-safety/` (deeper `Pointer`, `NativeSlice`, `Weak` chapters)
- `docs/handbook/11-reference/03-built-in-types.md` (example-enriched catalog)
- `docs/handbook/SUMMARY.md`
- `docs/init/ZIRK_FEATURE_STATUS.md` (status callouts, if needed)
- `../zirk-lang-site` (re-sync after commit)
