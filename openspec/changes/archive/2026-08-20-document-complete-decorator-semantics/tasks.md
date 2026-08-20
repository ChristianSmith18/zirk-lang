## 1. Establish the Canonical Decorator Source

- [x] 1.1 Create a complete English `docs/DECORATOR_SEMANTICS.md` covering declarations, the five targets, phase ordering, wrapper variants, contract preservation, composition, dependencies, repeatability, hygiene, expansion, permissions, caching, generics, inheritance, diagnostics, erasure, and generated runtime registries.
- [x] 1.2 Add normative examples for validation, routing, dependency injection, authorization/cache ordering, result transformation, catch/recovery, repeatable middleware, duplicate policy, dependency errors, cycles, generated API, and build permissions.
- [x] 1.3 Link the canonical decorator source from the repository documentation indexes and source map so agents and implementers can discover its authority and relationship to the language, compiler, permission, and handbook documents.

## 2. Rewrite the Metaprogramming Handbook

- [x] 2.1 Rewrite the metaprogramming overview, decorator introduction, and `fn dec` pages with the accepted compile-time-only model and explicit target-block syntax.
- [x] 2.2 Rewrite the target and Syntax API pages around `class`, `attribute`, `function`, `method`, and `parameter`, including initialization parameters, typed immutable handles, hygienic builders, generated contracts, and prohibited transformations.
- [x] 2.3 Document `Inspect -> Augment -> Wrap`, `match target.wrap`, `Before`, both `After` forms, `Catch`, exclusive `Around`, exact payload arity, `_`, and visible error/permission propagation.
- [x] 2.4 Document source-order composition, `requires`/`before`/`after`, no automatic reordering, self-reference rejection, full cycle diagnostics, override behavior, and generic declaration expansion.
- [x] 2.5 Document `repeatable fn dec`, contiguous grouped applications, explicit phase payloads, typed `application.arguments`, application cardinality and duplicate policy, and the absence of implicit variables.
- [x] 2.6 Rewrite compile-time reflection, runtime reflection, reflection-retention, diagnostics, and security pages to remove automatic decorator retention while preserving explicitly generated ordinary descriptors and registries.
- [x] 2.7 Update `docs/handbook/SUMMARY.md` and previous/next navigation for any pages added, renamed, removed, or reordered during the rewrite.

## 3. Update Language and Compiler Specifications

- [x] 3.1 Replace the decorator section of `docs/ZIRK_LANGUAGE_SPEC.md` with the complete accepted syntax and semantics and explicitly exclude general `comptime`, unsupported targets, and automatic runtime retention.
- [x] 3.2 Update `docs/ZIRK_COMPILER_SPEC.md` with expansion stages, validation boundaries, dependency graphs, hygiene, recursion limits, fingerprints, incremental invalidation, generated API emission, source maps, and decorator diagnostics.
- [x] 3.3 Update `docs/ZIRK_STDLIB_SPEC.md` and any reflection/descriptor APIs so runtime framework support uses explicitly generated typed structures instead of retained decorator applications.
- [x] 3.4 Reconcile the decorator and reflection decisions in `docs/01_plantilla_zirk.md` with the canonical English source, removing `constructor` targets and automatic runtime decorator metadata claims.

## 4. Update Guides, References, and Framework Examples

- [x] 4.1 Rewrite the handbook decorator tutorial as a complete working progression from a simple validator through a wrapper and generated registry, with explicit compiler-provided bindings.
- [x] 4.2 Expand the attributes/decorators and grammar references with declaration/application syntax, supported targets, phase payloads, repeatability, dependency clauses, erasure, and diagnostic cases.
- [x] 4.3 Update project, library, public API, permission, compiler-pipeline, debugger, diagnostics, and standard-library pages affected by decorator build effects, generated public API, or source provenance.
- [x] 4.4 Add framework-oriented examples demonstrating how class/member/callable/parameter targets support NestJS-, Spring Boot-, FastAPI-, and Angular-like static registries without language-level module or constructor targets.

## 5. Remove Contradictions and Verify the Documentation

- [x] 5.1 Search all Markdown documentation for decorator, annotation, reflection-retention, constructor-target, implicit-application, runtime-metadata, phase-order, and wrapper terminology and resolve every contradiction against `docs/DECORATOR_SEMANTICS.md`.
- [x] 5.2 Verify that every compiler-provided example binding originates in a target block, match payload, parameter, loop binding, or other visible expression and that all Zirk examples follow the current grammar conventions.
- [x] 5.3 Verify all handbook previous/next links, summary links, root documentation links, anchors, and code-fence formatting.
- [x] 5.4 Run the repository documentation checks and `openspec validate --all --strict`, fix all failures in scope, and record a clean final status without modifying the unrelated Rust work already present in the worktree.

Verification note: handbook local links, OpenSpec strict validation, and `git diff --check` pass. The full workspace script reaches an unrelated pre-existing Rust formatting diff in `crates/zirk-parser/src/parser.rs`; direct Clippy additionally reaches the incomplete pre-existing AST/parser work (`zirk_ast::Program` now requires `contracts` in `zirk-cli`). Neither Rust change was modified by this documentation change.
