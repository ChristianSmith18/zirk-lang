# Language Documentation Information Architecture — Data Types Deep Dive

## ADDED Requirements

### Requirement: Ordered type deep-dive track in the handbook

The handbook information architecture SHALL provide an ordered track that teaches the type spectrum before presenting per-type API chapters, and SHALL include a hub, rewritten category chapter, a "choosing a type" decision page, and new/expanded per-type chapters for `List`, `Array`, `Regex`, `Pointer`, `NativeSlice`, `Weak`, `Dependent`, and `Fn` / `Function`.

#### Scenario: Reader follows the canonical learning path

- **WHEN** a reader opens `03-everyday-types` from `SUMMARY.md`
- **THEN** the first chapter is the new hub, followed by the category chapter, then per-type chapters in an order that respects the spectrum

#### Scenario: Reader finds a missing type

- **WHEN** a reader looks for `Regex`, `List`, `Pointer`, `NativeSlice`, `Weak`, `Dependent`, or `Fn` in the handbook
- **THEN** `SUMMARY.md` lists a dedicated canonical chapter or a clear cross-reference from the hub

### Requirement: Cross-linked built-in catalog

The reference page `11-reference/03-built-in-types.md` SHALL contain at least one runnable example for every major built-in category and SHALL link every type to its detailed handbook chapter or to an explicit implementation-status notice.

#### Scenario: Developer uses the built-in catalog as a lookup

- **WHEN** a reader opens `03-built-in-types.md` to choose a type
- **THEN** the page shows an example for each spectrum position and links to the deeper chapter
