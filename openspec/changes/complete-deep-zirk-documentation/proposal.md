## Why

The handbook has complete navigation and a mature semantic core, but many standard-library, toolchain, testing, low-level, tutorial, and reference chapters remain outlines rather than TypeScript-Handbook-depth documentation. Before building the public website, these chapters must become implementation-grade explanations with signatures, examples, edge cases, diagnostics, permissions, performance contracts, and reliable cross-navigation.

## What Changes

- Deepen the documentation in three sequential blocks chosen by the language author.
- Block 1: fully document the standard library and toolchain, including APIs, compiler stages, build behavior, tooling workflows, permissions, errors, complexity, and examples.
- Block 2: fully document testing and native/low-level development, including executable workflows, ABI and safety boundaries, benchmarks, concurrency, permissions, packaging, and platform behavior.
- Block 3: build progressive end-to-end tutorials and complete the reference material, including grammar/API indexes, diagnostic guidance, feature status, compatibility, glossary, and normative-source navigation.
- Replace remaining outline-only chapters with content sized to their subject rather than a fixed template or artificial line target.
- Replace all archived-spec `Purpose: TBD` text with concise descriptions of the capability each spec governs.
- Audit examples, local links, previous/next navigation, terminology, contradictions, and future website readiness after every block.
- Preserve the existing canonical semantic documents; this change explains and connects them rather than reopening accepted language decisions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `language-documentation-information-architecture`: Require the published handbook to provide substantive, example-driven coverage for standard-library, toolchain, testing, low-level, tutorial, and reference material in three auditable blocks.
- `handbook-editorial-system`: Add chapter-depth, example, API-contract, navigation, source-attribution, and completion criteria suitable for a public documentation website.

## Impact

- Affects the handbook sections under `04-standard-library`, `05-native-and-low-level`, `07-toolchain`, `08-testing`, `10-tutorials`, `11-reference`, and related package/project pages.
- Updates `docs/handbook/SUMMARY.md`, editorial source maps, root indexes, specialized specs, and the purposes of main OpenSpec capabilities where currently marked `TBD`.
- Adds no compiler implementation and changes no accepted language semantics.
- Produces content intended to become the direct source for the future documentation website.
