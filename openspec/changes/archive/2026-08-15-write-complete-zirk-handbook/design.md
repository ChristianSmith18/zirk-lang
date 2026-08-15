## Context

Zirk already has several normative and historical documents, plus a compact documentation website. Those sources describe a broad language—syntax, types, effects, concurrency, resources, packages, tooling, and a standard library—but they are organized for design and implementation work rather than for learning. The public handbook must turn that material into a coherent curriculum without silently changing the language.

The primary content inventory is `docs/01_plantilla_zirk.md`. Normative decisions come from `docs/ZIRK_SPEC_FINAL.md`, `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_RUNTIME_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md`, and `docs/ZIRK_COMPILER_SPEC.md`; when an older planning document conflicts with a final specification, the final specification wins. The handbook is written in English and remains plain Markdown so it can be reviewed independently from any website renderer.

The language author's annotated review dated 2026-08-15 is newer than the source snapshot used by this worktree. For this final editorial pass, those annotations define the intended public behavior. The matching normative specifications are updated only after the documentation is transplanted onto the latest `develop`, preventing an old worktree from overwriting newer compiler-phase specifications.

## Goals / Non-Goals

**Goals:**

- Define a complete, stable information architecture for public Zirk documentation.
- Teach concepts progressively, with problem-led explanations and realistic Zirk code.
- Support both sequential learners and experienced readers looking up a single feature.
- Make every ordered chapter navigable through explicit previous and next links.
- Record normative sources and implementation status without mixing the two concepts.
- Let chapter depth follow conceptual complexity instead of enforcing uniform length.

**Non-Goals:**

- Redesign or integrate the existing `web/` interface in this change.
- Invent syntax or guarantees that are absent from the normative specifications.
- Generate API documentation mechanically from compiler sources.
- Promise that every specified feature is already implemented.
- Translate the handbook into additional languages during this change.

### Apply authorial annotations as corrections, not open proposals

The annotated review identifies underspecified teaching, incorrect examples, and newer language decisions. All numbered annotations are applied consistently across tutorial and reference material. Where a note introduces syntax absent from the worktree's old specifications, the handbook documents the intended language and the later integration step updates the current specifications on `develop`.

This avoids preserving known-wrong public documentation merely because its source snapshot predates the review. Compiler implementation status remains separately disclosed and is not implied by documenting the intended language.

## Decisions

### Use a layered documentation architecture

The handbook will be divided into introductions, learning paths, a progressive language handbook, project guides, standard-library guides, specialized low-level and metaprogramming material, tooling, testing, packages, tutorials, references, explanations, and appendices. A root `SUMMARY.md` will define canonical reading order and serve as the future website navigation source.

This is preferred over one monolithic language guide because readers have different entry points and because reference material should remain directly discoverable. A purely alphabetical catalog was rejected because it does not provide a learning progression.

The concrete file contract is the detailed tree supplied with this change: `00-getting-started`, `01-learning-paths`, nineteen core-handbook areas, and the specialized sections `03-projects` through `13-appendices`. Each leaf topic receives its own Markdown document; section `README.md` files provide orientation rather than replacing the leaf documents.

### Keep Markdown as the editorial source of truth

All handbook content will live under `docs/handbook/`. The current website may consume or reproduce that content in a later change, but this change will not couple chapters to the site's HTML structure.

This is preferred over editing pages directly in `web/` because Markdown is easier to review, diff, link, and validate before presentation concerns are introduced.

### Use a flexible chapter contract

Every chapter must explain its subject in plain language, demonstrate it with valid Zirk code, identify relevant constraints, and provide adjacent navigation. Authors select only the sections needed for the topic. Complex semantic topics may include mental models, desugaring, invalid examples, diagnostics, interactions, and design rationale; short orientation pages need not imitate that length.

A rigid template with identical headings was rejected because it would produce filler and obscure the natural teaching sequence of each concept.

### Separate teaching order from reference order

The progressive handbook introduces concepts when a learner needs them. Reference sections reorganize the same language by syntax, semantics, diagnostics, standard-library modules, CLI commands, and configuration fields. Cross-links connect the two views.

This duplication of access paths is intentional: tutorials optimize for comprehension, while references optimize for retrieval.

### Make source fidelity visible

Every substantive chapter will link to its normative source or to the repository source index. Features will carry an implementation-status note whenever specified behavior is not yet fully available. Examples must use only syntax supported by the cited specification; uncertain examples are marked as conceptual rather than runnable.

### Treat navigation as data with a rendered fallback

`SUMMARY.md` defines the ordered corpus. Each ordered Markdown file also includes human-readable previous and next links at its end, so navigation remains useful on GitHub and in basic Markdown viewers. Link validation will ensure that both representations agree.

## Risks / Trade-offs

- **The source documents may disagree** → Apply the documented authority order, cite the winning source, and record unresolved contradictions rather than guessing.
- **A very broad tree can create shallow pages** → Create chapters in reviewed batches and require each published page to meet the editorial contract before it is marked complete.
- **Examples can drift from the compiler** → Distinguish specification-valid examples from currently executable examples and add automated parsing checks when the compiler supports the relevant feature.
- **Manual previous/next links can drift** → Validate links against `SUMMARY.md` and update navigation in the same task as any reorder.
- **Some repetition is unavoidable** → Keep tutorials explanatory and references concise, then cross-link instead of duplicating long semantic descriptions.

## Migration Plan

1. Establish the handbook root, source index, editorial guide, and canonical summary.
2. Write the orientation and getting-started sequence as the first reviewed vertical slice.
3. Add core language chapters in dependency order, followed by specialized guides and references.
4. Audit coverage against `docs/01_plantilla_zirk.md` and the final specifications.
5. Propose website integration only after the Markdown corpus and navigation are accepted.

The change is additive. Rollback consists of removing `docs/handbook/` and reverting the information-architecture delta; the existing website remains unaffected.

## Open Questions

- Which compiler milestone should be treated as the first executable-example baseline?
- Should future website generation read `SUMMARY.md` directly or use a separate machine-readable manifest derived from it?
- Which chapters should receive runnable test fixtures first once the parser covers the documented syntax?
