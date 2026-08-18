## Why

Zirk currently describes decorators only at a high level, leaving their syntax, targets, expansion phases, composition, repeatability, dependency validation, generated API, permissions, and erasure behavior underspecified. A complete public contract is needed so compiler implementers, framework authors, documentation writers, and tooling agents can implement the same deterministic and safe semantics.

## What Changes

- Define `fn dec` as compile-time behavior and `repeatable fn dec` as grouped, ordered decorator expansion.
- Restrict Zirk 1.x decorator targets to `class`, `attribute`, `function`, `method`, and `parameter`, including initialization parameters.
- Define the `Inspect -> Augment -> Wrap` expansion pipeline and the `Before`, `After`, `Catch`, and exclusive `Around` wrapper phases.
- Define explicit result transformation, wildcard payload omission, typed target context, hygienic generated names, public-name conflict handling, and source-aware diagnostics.
- Define top-to-bottom source evaluation with outer-to-inner composition, plus declarative `requires`, `before`, and `after` validation without automatic reordering.
- Reject decorator self-dependencies and direct or indirect dependency cycles.
- Define grouped repeatable applications with explicit `applications` phase payloads and typed `application.arguments` access.
- Define generic expansion, override behavior, generated public API visibility, recursive-expansion limits, deterministic fingerprints, incremental caching, and build-phase permission enforcement.
- **BREAKING**: Remove automatic runtime retention of decorators and general `Reflection.decorators(...)` behavior; decorators are erased after compilation unless they explicitly generate ordinary runtime descriptors or registries.
- Replace shallow or contradictory decorator documentation with a complete English source of truth and examples suitable for application and framework authors.

## Capabilities

### New Capabilities

- `zirk-decorators`: Complete language semantics for decorator declaration, targets, phases, wrappers, composition, dependencies, repeatability, hygiene, validation, caching, permissions, generated API, and compile-time erasure.

### Modified Capabilities

- `zirk-grammar`: Add the normative grammar surface for `fn dec`, `repeatable fn dec`, decorator applications, supported target phases, dependency clauses, and explicit phase payload patterns.

## Impact

- Affects the lexer/parser, AST and public Syntax API, name and type resolution, decorator expansion engine, compiler dependency graph and cache, permission analysis, diagnostics, source maps, API emission, formatter, language server, and framework-facing generated registries.
- Replaces decorator and reflection-retention claims across `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_COMPILER_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md`, `docs/01_plantilla_zirk.md`, and the metaprogramming, tutorial, reference, permission, project, and toolchain handbook pages.
- Does not implement decorators in the compiler in this documentation change; it establishes the normative source of truth and updates all affected documentation consistently.
