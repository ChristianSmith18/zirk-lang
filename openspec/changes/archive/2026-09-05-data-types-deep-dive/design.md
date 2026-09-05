## Context

The Zirk handbook already divides types into conceptual sections (`03-everyday-types`, `10-data-types`, `12-collections`, `03a-temporal`, `17-memory-and-safety`). However, the reader must assemble the mental model alone: the same type is described once as a "primitive", again as a "reference", and again in a memory section, without a single narrative that explains *why* each one exists in relation to the others. This design introduces a single, pedagogical through-line.

## Goals / Non-Goals

**Goals:**
- Give the handbook a unifying memory-behavior narrative: `immediate value → native value → managed reference → borrowed/dependent view → unsafe pointer`.
- Ensure every important type has a dedicated, example-rich page that follows the narrative.
- Add a decision-oriented "Choosing a type" page.
- Keep all examples specification-compatible and mark implementation status where needed.
- Update `SUMMARY.md` and cross-links so the new material is part of the canonical path.

**Non-Goals:**
- Change the Zirk language or runtime.
- Remove existing short reference pages (we expand, not replace, where possible).
- Promote any feature as implemented without evidence.

## Decisions

- **Narrative spine**: organize around "where the bytes live and who owns them" rather than by category name. This lets `String`, `Array`, `List`, `Pointer`, `NativeSlice`, `Weak`, `Dependent`, and `Fn` share a single story.
- **Hub page**: create a new `03-everyday-types/00-how-values-live-and-share.md` that is the entry point. Existing `01a-type-categories.md` becomes the second conceptual chapter, rewritten with examples.
- **Per-type depth contract**: every new/expanded page must include (1) what problem it solves, (2) a construction/literal example, (3) one valid mutation/sharing example, (4) one invalid example with the expected controlled error, (5) a "when to choose" callout.
- **Split the `String` chapter**: keep the existing rich `09-string.md` but add a concise cross-reference from the new hub to establish the managed-reference slot in the spectrum.
- **Reuse temporal unit**: `Duration` and other temporal types already live in `03a-temporal`; the hub links there instead of duplicating.
- **Status callouts**: any page that depends on `array-list-tuple-duration-regex` or Phase 4e features must carry an implementation-status block.

## Risks / Trade-offs

- **Risk: Handbook becomes too long.** → Mitigation: the hub is concise; per-type pages follow semantic complexity, and reference pages remain reference-grade.
- **Risk: Examples diverge from the actual compiler.** → Mitigation: every example is reviewed against the current fixtures and feature-status before publication.
- **Risk: `SUMMARY.md` ordering breaks the website pipeline.** → Mitigation: update `SUMMARY.md` first, then run the website content verify against it.

## Open Questions

- Should the hub live inside `03-everyday-types` or become a new top-level `Explanations` chapter? `03-everyday-types` keeps it in the canonical learning path.
- Should `Regex` be a handbook chapter or a standard-library reference page? It is both a literal and an API, so a short handbook page plus `std.text` expansion is the safest route.
