## Why

The existing documentation site summarizes Zirk accurately but does not yet teach the language with the depth, progression, examples, and topic granularity expected from a public programming-language handbook. Zirk needs an English-first Markdown corpus that can stand on its own as the source material for a future documentation website.

## What Changes

- Add a comprehensive English handbook under `docs/handbook/`, organized into audience-specific introductions, a progressive core handbook, project and standard-library guides, native/low-level topics, metaprogramming, tooling, testing, packages, tutorials, references, explanations, and appendices.
- Define a flexible editorial contract inspired by the TypeScript Handbook: introduce the problem first, teach terminology in context, use complete Zirk examples, contrast valid and invalid code, show expected diagnostics, and connect each chapter to adjacent material.
- Give every ordered handbook document explicit previous and next links, plus related-topic and normative-source references.
- Treat chapter length as content-driven: short chapters remain focused while semantic or architectural topics receive the depth they require.
- Use `docs/01_plantilla_zirk.md` as the exhaustive topic inventory and the final language/runtime/stdlib/compiler specifications as the authority whenever historical decisions conflict.
- Keep the handbook Markdown independent of the current `web/` presentation. Website integration will be proposed only after the Markdown corpus is reviewed.
- Apply the language author's annotated handbook review as the authoritative correction set, including the syntax and semantic clarifications that postdate the specifications originally copied into this worktree.

## Capabilities

### New Capabilities

- `handbook-editorial-system`: Defines the English-language chapter format, teaching method, code-example standards, diagnostics, navigation metadata, review rules, and content-quality expectations.

### Modified Capabilities

- `language-documentation-information-architecture`: Expands the documentation taxonomy from a small set of website pages into a complete multi-document handbook with learning paths, progressive chapters, specialized references, tutorials, and appendices.

## Impact

- Adds a substantial Markdown tree under `docs/handbook/`.
- Adds a handbook inventory and navigation order that can later drive website generation or hand-authored pages.
- Does not change compiler crates, runtime behavior, package formats, or the current website implementation. Normative source synchronization is deliberately performed later on the latest `develop` revision so documentation-only integration remains isolated.
- Establishes English as the public handbook language while preserving existing normative source documents as authoritative inputs.
