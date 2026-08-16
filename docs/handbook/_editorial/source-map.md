# Handbook Source Map

The handbook synthesizes repository documents with different roles and ages. This map prevents historical material from silently overriding the final language.

## Authority order

1. [`ZIRK_SPEC_FINAL.md`](../../ZIRK_SPEC_FINAL.md) defines Zirk 1.x identity, scope, exclusions, foundational contracts, and conflict resolution.
2. [`ZIRK_LANGUAGE_SPEC.md`](../../ZIRK_LANGUAGE_SPEC.md), [`ZIRK_RUNTIME_SPEC.md`](../../ZIRK_RUNTIME_SPEC.md), [`ZIRK_STDLIB_SPEC.md`](../../ZIRK_STDLIB_SPEC.md), and [`ZIRK_COMPILER_SPEC.md`](../../ZIRK_COMPILER_SPEC.md) define their specialized areas.
3. [`01_plantilla_zirk.md`](../../01_plantilla_zirk.md) is the exhaustive historical topic inventory. It helps find questions the handbook must address, but it is not allowed to restore features excluded by the master specification.

When final documents remain ambiguous, the handbook records the ambiguity or defers the claim. It does not invent a silent resolution.

The language author's numbered annotations dated 2026-08-15 supersede the
older specification snapshot used to create this worktree. Their decisions are
documented here first and synchronized into current normative specs only after
the documentation is transplanted onto the latest `develop` revision.

## Coverage map

| Handbook area | Primary authority | Inventory support |
| --- | --- | --- |
| Identity, scope, project minimum, foundational contracts | Master specification §§1–8 | Template §§0, 37–46 |
| Syntax, bindings, types, control flow, functions, objects, errors, modules, metaprogramming, unsafe code | Language specification §§1–14 | Template §§1–16, 19, 25–35, 44–46 |
| Type taxonomy, conversions, native operators, Unicode text and temporal semantics | Language specification §§2–4 and §8.1; standard-library specification §8 | Template §§3, 18, 28, 49–54 |
| Memory, resources, tasks, scheduling, cancellation, I/O, threads, shutdown | Runtime specification | Template §§15–18, 22–24, 33–34 |
| Standard-library modules and operational contracts | Standard-library specification | Template §§22–24, 30–31, 39 |
| Compiler pipeline, IR, targets, diagnostics, CLI, formatter, linter, LSP, debugger | Compiler specification §§1–12 | Template §§21, 40–43 |
| Projects, packages, permissions, distribution | Master and specialized specifications | Template §§20, 38, 46 |

## Explicit Zirk 1.x exclusions

Historical material about WebAssembly/browser integration, public runtime directives, a standalone `worker` primitive, `async fn`, a public event loop, textual inline assembly, general `comptime {}`, general `defer`, multiple class inheritance, traditional function overloading, `Result` propagation with `?`, and public ownership/reference-counting semantics must be described as excluded or historical—not as usable Zirk 1.x features.

## Publication audit

The complete handbook was initially audited on 2026-08-13 and received its
authorial correction audit on 2026-08-15:

- **Inventory coverage:** all 334 Markdown documents named by the approved handbook tree are present. The comparison reports zero missing and zero unexpected published chapters.
- **Navigation:** every published document appears exactly once in `SUMMARY.md` and carries adjacent previous/next navigation appropriate to its position.
- **Links:** all local Markdown targets resolve. External links remain intentionally limited to source or ecosystem references that need an authoritative destination.
- **Examples:** fenced examples were reviewed in context as valid, invalid, illustrative, manifest, shell, or output samples. Intentionally invalid examples are introduced as invalid and followed by the expected rule or diagnostic behavior.
- **Semantic claims:** terminology and examples were checked against the authority order above. Searches for historically proposed features confirmed that they appear only as explicit exclusions, comparisons, CLI names, or invalid examples.
- **Implementation gaps:** the handbook distinguishes the normative Zirk 1.x language from current implementation availability. Known limitations and non-promises are collected in [Current Limitations](../13-appendices/07-current-limitations.md) and [Feature Status](../11-reference/12-feature-status.md).

The current compiler is not yet a complete executable oracle for every normative example. Consequently, this audit verifies examples against the final specifications and their grammar rather than claiming that every example can already be compiled by the repository implementation.

## Type-system canonical owners

| Rule family | Canonical handbook owner |
| --- | --- |
| Type tree and categories | `02-handbook/03-everyday-types/01-object-and-type-hierarchy.md` and `01a-type-categories.md` |
| Value/reference identity, aliases and cloning | `01b-value-and-reference-behavior.md` plus Bindings and Values |
| Contracts and native operators | `01c-contracts-and-capabilities.md`, `01e-native-operators.md`, operator reference |
| Numeric conversions and contextual evaluation | `01d-conversions-and-context.md`, signed/unsigned/Float chapters |
| Unicode Char and mutable String | Char and String chapters |
| Calendar and timeline types | `02-handbook/03a-temporal/` |
| Domain value/reference choices | Data Types and Classes and Objects |

---

**Previous:** [Editorial Guide](./editorial-guide.md) · **Handbook:** [The Zirk Handbook](../README.md)
