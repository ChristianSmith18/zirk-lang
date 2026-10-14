# Compiler improvement roadmap

This roadmap replaces the August 2026 suggestions snapshot. It is an
implementation-planning aid; language contracts remain in the specifications,
ADRs, and `docs/init/ZIRK_FEATURE_STATUS.md`.

## Reconciliation against the current compiler

| Earlier recommendation | Current status | Remaining work |
| --- | --- | --- |
| Complete interface/trait pipeline | Largely delivered: parsing, semantic checks, IR and native dispatch work end to end. | Generic contract dispatch is still partial; see the feature-status matrix for trait defaults under substitution and user-defined generic function bodies. |
| Add an LLVM-independent `zirk check` | Delivered through the shared frontend and `zirk-check` binary; it builds without the LLVM backend. | Keep diagnostics and behavior shared between both entry points. |
| Preserve the diagnostic contract | Existing diagnostics use stable codes, structured severity/location/cause/help, bounded collection and deterministic rendering. | Maintain as a compatibility invariant; no separate implementation project is needed. |
| Add a lossless syntax tree | Not delivered. The lexer can expose comments to the formatter, but the compiler has no persistent lossless tree retaining whitespace, invalid tokens and recovered nodes. | Consider when incremental editing/refactoring needs justify the representation; preserve the semantic AST boundary. |
| Add real module identity and per-module name resolution | Partially delivered. The loader follows reachable imports, records canonical source identity, confines imports to project/dependency roots, and rejects symlink escapes; the driver still merges declarations into one crate namespace. | Implement module-scoped visibility/resolution while preserving import spans. |
| Centralize feature-stage status | Partially delivered as documentation: `ZIRK_FEATURE_STATUS.md` records lexer/parser/sema/lowering/runtime/CLI stages. | Prefer consistency checks or generated status links over a second hand-maintained compiler catalog. |
| Keep recovery poison out of generated programs | The normal frontend returns no checked result after errors, and IR has an independent verifier. | Preserve these gates as invariants; tooling's lenient type-map path must remain non-emittable. |
| Centralize terminal and LSP source positions | Delivered in `SourceFile`, including UTF-16 conversion, inverse mapping and astral-character coverage. | Add cases only when a concrete coordinate bug appears; do not duplicate conversion logic in clients. |
| Use conventional formatter validation | Delivered: `zirk format --check` fails on changes; `--diff` can show the proposed output. | Preserve comment handling and no-write behavior under `--check`. |
| Explain and clean incremental compilation caches | No compiler-result cache is present. `.zirk/packages` caches acquired dependencies, not tokens/types/IR/object compilation results. | Defer cache commands until measured compile latency and a dependency fingerprint design justify them. Never conflate dependency storage with rebuildable compiler caches. |
| Add local ICE context without telemetry | No standalone, stable stage/category report has been established as a user-facing contract. | Defer until compiler failures need broader reproduction support. Never transmit source or environment data implicitly. |

## Work worth proposing next

### 1. Per-module name resolution

This is the largest remaining correctness and isolation improvement from the
earlier audit. Root containment and canonical path deduplication were delivered
under `module-import-root-containment`. Current multi-file loading still
flattens declarations, which makes module visibility and same-name declarations
harder to reason about. A future change should define module-scoped lookup and
visibility before changing sema or IR.

### 2. Persistent lossless syntax for editor workflows

The current formatter parses semantic AST and separately gathers comments. It
does not provide a reusable tree for malformed text, whitespace-preserving
edits, or subtree reuse. This should be a staged tooling project, not a parser
rewrite: define token/trivia ownership, recovered nodes, immutable snapshots,
then adapt formatter/LSP consumers while keeping semantic AST users independent
of trivia.

### 3. Feature-status drift checks

The status matrix already supplies the human-readable source of truth. The
remaining opportunity is to detect disagreement between that matrix, pending
feature diagnostics, fixtures and CLI availability. Start with a narrow
consistency checker for explicitly declared feature rows; avoid duplicating the
entire status table in Rust unless generation can make one representation
authoritative.

## Deferred optimization and maintenance

- Do not enable stronger LLVM optimization levels for release based only on
  compile-time or synthetic wins. The pipeline guide records that the current
  runtime benchmark matrix has not established a repeatable gain; benchmark
  representative programs and safety overhead first.
- A persistent syntax tree, fine-grained incremental compilation cache, and
  richer cache diagnostics are separate investments. Do not add cache deletion
  commands before identifying cache ownership and invalidation rules.
- Splitting the large IR-lowering and LLVM-emission modules may improve
  maintainability, but it is not a correctness fix by itself. Choose seams from
  current ownership boundaries and require behavior-preserving changes.

## Closed or obsolete follow-ups

- `String`/`Char` collector-managed lifetime strategy is closed by the managed
  mark-sweep implementation.
- `value class` was removed from the language surface; use `record` for value
  semantics or `class` for identity. The earlier open design question is void.
- Declared generic variance is checked positionally in the current sema.
- Runtime thread-safety and synchronization need to be audited per new primitive;
  the blanket pre-concurrency audit item is no longer a current compiler task.

These statuses were checked against the current feature-status matrix, toolchain
documentation and implementation sources on 14 October 2026. This document does
not claim that the pending architectural work has been implemented.
