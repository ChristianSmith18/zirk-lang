# Compiler Improvement Suggestions

This document records implementation guidance discovered while comparing the
compiler and its tests with the current Zirk language and compiler
specifications. It is written for agents working on the compiler.

It is not a replacement for the normative specifications, an authorization to
rewrite working subsystems, or a claim that every future feature must be built
immediately. Before acting on an item, inspect the current branch and the active
OpenSpec change that owns the affected phase.

## Observed baseline — 17 August 2026

The compiler is a real staged implementation, not an empty scaffold. The
workspace contains dedicated crates for diagnostics, lexing, parsing, AST,
semantic analysis, typed IR, LLVM code generation, CLI coordination, and the
runtime. The implementation and tests already establish important rules that
new work must preserve.

Verified existing behavior includes:

- byte-based source spans with Unicode-aware human columns;
- continued lexing, parsing, and semantic checking after recoverable errors;
- a parser nesting limit of 128;
- a compilation-wide limit of 100 diagnostics, with deduplication and an
  explicit suppressed-diagnostic count;
- stable diagnostic codes and human/JSON rendering;
- recognition of later language constructs with targeted roadmap diagnostics;
- lexical longest-match rules and canonical Unicode text literals;
- recursive-descent parsing with explicit precedence and synchronization;
- scopes, shadowing, definite initialization, return-path checking, basic
  inference, nullability, closures, enums, classes, visibility, constructors,
  inheritance, and method calls;
- typed three-address IR with source locations and an independent verifier;
- class/object layouts, constructor/method lowering, and virtual dispatch;
- LLVM object emission, platform linking, and a static runtime.

LLVM 20 is available on the inspected Apple Silicon development machine at:

```text
/opt/homebrew/opt/llvm@20
```

Commands that build the LLVM crate require:

```bash
export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20
```

The installed `llvm-config` reports LLVM 20.1.8.

With LLVM configured, the workspace currently stops compiling in
`zirk-cli/src/driver.rs`: the active AST work added `Program.contracts`, while
the driver's `merge` initializer has not yet added that field. Separately, one
parser test still expects `interface` and `trait` to be pending even though the
active parser work has begun accepting them. These are expected integration
points of unfinished object/type-system work. Preserve and complete the active
changes; do not delete or overwrite them to restore an older test expectation.

## Principles for follow-up work

1. Preserve validated behavior unless a newer normative decision replaces it.
2. Do not combine a feature's parser work with an unrelated frontend rewrite.
3. Treat compiler recovery types and recovered syntax as invalid for codegen.
4. Keep tooling paths independent from LLVM when they do not generate code.
5. Do not call a feature implemented until every required pipeline stage is
   complete and tested.
6. Keep public syntax and diagnostics derived from the language specifications;
   internal cache/tree representations may evolve without becoming language API.

## P0 — Complete active contract integration safely

The AST/parser work for `interface`, `trait`, and class `implements` is only the
first pipeline segment.

Required follow-up:

1. Add `contracts` to every construction, merge, visitor, serialization, and
   test fixture for `Program`.
2. Preserve each declaration's source file/module identity during merging.
3. Collect contract signatures before checking bodies so forward and mutual
   references behave consistently with classes and functions.
4. Validate interface bodies, trait default bodies, class conformance,
   visibility, signature compatibility, ambiguity, and cycles using the current
   object/type-system specification.
5. Represent contract calls in typed semantic data and portable IR.
6. Lower the selected implementation/default method without losing source maps.
7. Add IR verification and backend tests before changing feature status.
8. Update pending-feature tests only when the applicable stage actually accepts
   the feature.

Acceptance criteria:

- the workspace compiles with LLVM configured;
- parser, semantic, IR, backend, and CLI tests cover valid and invalid contract
  programs;
- a parsed-only construct cannot reach code generation;
- existing class behavior remains green;
- no active worktree edits are discarded.

## P0 — Add an LLVM-independent `zirk check`

The lexer, parser, module loader, and semantic checker already do useful work
without IR or native emission. `zirk check` should become the fastest complete
frontend validation command and must not require LLVM, a linker, SDKs, or the
runtime archive.

Recommended architecture:

```text
source loading
    -> lex/parse
    -> module/name resolution
    -> type/flow checking
    -> checked frontend result
         |-> zirk check
         |-> formatter/linter/LSP consumers
         `-> IR -> LLVM -> link for build/run only
```

Avoid solving this only with a runtime branch inside the current LLVM-dependent
binary graph. The frontend driver/library must itself be buildable and testable
without compiling `zirk-codegen-llvm`.

Acceptance criteria:

- `zirk check` works when `LLVM_SYS_201_PREFIX` is absent;
- it performs no object emission or linking;
- it returns nonzero for frontend errors and zero for a valid program;
- diagnostics match `zirk build` through the end of semantic checking;
- later build-only stages can add diagnostics without changing earlier codes.

## P0 — Preserve the diagnostic contract

Do not replace the established diagnostic structure with a separate public
`status` line merely to describe incomplete features. The current common shape
is:

```text
severity + stable code + message + location/source snippet + cause + help
```

Pending or partially available features should use that same shape. During the
private development period, roadmap phases remain useful and may appear in
`cause` or `help`:

```text
error[E0302]: `task` is not implemented yet
  = cause: the construct is recognized by this compiler
  = help: task semantics arrive in Phase 5
```

There is no need to design final public wording now. Before the public release,
audit phase-oriented messages so they describe the shipped compiler version and
feature status without making internal roadmap numbering permanent language
semantics.

Retain these current guarantees:

- stable codes are never reused for a different meaning;
- warnings are promoted only by central policy;
- structured output never contains terminal color escapes;
- diagnostics are bounded, deduplicated, and deterministically ordered;
- recovery avoids cascades but never permits invalid codegen.

Recommended limits:

- batch `check`/`build`: retain the current maximum of 100 diagnostics;
- LSP publication: default to at most 20 actionable diagnostics per file,
  retaining an explicit truncation indicator.

## P1 — Introduce a lossless syntax layer without replacing semantic AST

The current AST is appropriate for semantic analysis. It intentionally does not
preserve all whitespace, comments, invalid tokens, or recovery structure. A
formatter, incremental LSP, precise refactorer, and public Syntax API need that
information.

Recommended model:

```text
Source text
    -> lossless immutable syntax tree
       (tokens, comments, whitespace, invalid/recovered nodes)
    -> semantic AST lowering
    -> name/type/flow analysis
```

Do not rewrite the working recursive-descent grammar solely to adopt a trendy
parser architecture. It may emit events/nodes for a persistent lossless tree and
then lower to the existing semantic AST. Keep grammar recovery and source spans
covered by the current tests.

Acceptance criteria:

- parsing and formatting never lose comments;
- malformed documents still produce a traversable tree for tooling;
- semantic AST users do not depend on trivia;
- unchanged syntax subtrees can be reused incrementally;
- generated syntax remains immutable and must pass normal validation.

## P1 — Implement real module identity and per-module resolution

The loader correctly walks imports from the entry file, reads each discovered
file once, and tolerates mutual imports. The current driver then flattens units
into one crate-wide namespace. Replace that temporary behavior without losing
the good graph traversal.

Required model:

- retain a module node for each source file;
- retain imports and published/private declarations per module;
- collect declarations/signatures before checking bodies;
- resolve imported names through the importing module rather than a global
  flattened table;
- preserve mutual imports where semantic dependency rules permit them;
- keep unused/unreachable `.zrk` files outside the compilation.

For an existing file, preserve the user-written path for diagnostics but derive
a canonical filesystem identity after successful resolution. The same physical
file reached through aliases or symlinks must be one module. Imports must not
escape the project root except through a dependency explicitly declared in
`init.zrk`. Missing targets still need diagnostics at the importing span.

Loading behavior:

- `zirk check` should continue discovering and parsing reachable imports after
  recoverable syntax errors so one run reports project-wide problems;
- `zirk build` may complete loading/parsing for diagnostics, but must stop before
  semantic/IR/codegen once that stage has errors.

## P1 — Centralize feature-stage status

Current feature availability is encoded in several places: keyword/token phase
methods, pending types, parser diagnostics, semantic diagnostics, CLI commands,
tests, and documentation. The active interface/trait test drift demonstrates
the maintenance risk.

Create one compiler-owned feature catalog or generated table with at least:

```text
feature identity
recognized by lexer
parsed
semantically checked
lowered to IR
supported by backend/runtime
available through CLI/tooling
roadmap phase
```

“Implemented” means every stage required to execute or otherwise use the
feature is complete. “Recognized”, “parsed”, “checked”, and “lowered” are useful
internal statuses, but none alone means implemented.

During private development, user-facing errors may continue to describe the
roadmap phase through the standard cause/help fields. Do not spend time building
a polished public partial-feature UI before release. The important immediate
goal is one source of truth that prevents code and tests from disagreeing.

## P1 — Keep recovery poison out of emitted programs

`Type::Unknown` currently prevents one semantic error from producing many
secondary mismatches. Preserve that approach, or an equivalent explicit error
type, with a hard boundary:

- unknown/error types may flow only while accumulating diagnostics;
- their operations must not be treated as evidence for overload, ownership,
  permission, or safety decisions;
- any frontend error prevents IR lowering;
- the IR verifier must reject an unknown/error type if a bug allows one through.

The same rule applies to missing/recovered syntax nodes.

## P1 — Make source positions serve terminal and LSP correctly

Keep byte offsets as the canonical internal span representation. Add indexed,
central conversions for:

- byte offset;
- Unicode-scalar human column;
- UTF-16 line/character position required by LSP clients.

Do not let each tool recompute positions independently. Cache per-line indices
in the persistent source snapshot, handle invalid/incomplete editor text, and
test astral Unicode, combining marks, tabs, CRLF, and edits around multibyte
characters.

## P1 — Make formatter validation conventional

For formatter CI, prefer the ecosystem-standard spelling:

```text
zirk format --check
```

`--check` means “exit nonzero if canonical formatting would change files” and
does not write. Tools such as rustfmt, Black, and Prettier have made this meaning
recognizable.

If previewing the actual proposed output/diff is useful, additionally support:

```text
zirk format --dry-run
```

Do not make `--dry-run` the only CI validation mode: “dry run” says that writes
are disabled but does not universally define whether differences are failure,
printed content, or only logged actions.

Both modes must preserve comments and use the same canonical formatter engine.

## P2 — Add explainable and cleanable incremental caches

The cache representation must remain internal, content-addressed, target/profile
aware, versioned, and safe to delete. Public tooling should expose behavior, not
the on-disk schema.

Recommended commands:

```text
zirk build --verbose
zirk check --verbose
zirk clean --cache
```

Verbose output should explain reuse and invalidation at a useful granularity:

```text
reused: module domain.user (content and dependency fingerprints match)
rechecked: module api.routes (imported public API changed)
invalidated: decorator Route (observed input fingerprint changed)
```

`zirk clean --cache` should remove only rebuildable compiler caches. A plain
`zirk clean` may remove all project build artifacts according to its documented
scope. Neither command may remove sources, lockfiles, permission approvals, or
credentials. Cache corruption must fall back to safe recomputation rather than
making compilation incorrect.

## P2 — Improve internal compiler error reports without telemetry

The purpose of additional ICE metadata is to make a local bug reproducible and
route it to the failing stage. It is not analytics.

Include when available:

- compiler version and build/profile;
- active target;
- pipeline stage;
- stable diagnostic/ICE category;
- deterministic local correlation identifier when multiple reports belong to
  the same failed compilation.

Never automatically attach or transmit source code, environment variables,
paths outside the minimally redacted diagnostic, credentials, permission
approvals, or machine fingerprints. Zirk must not send crash reports over the
network without a future explicit opt-in design. Since the compiler is currently
private and operated by one developer, a globally meaningful crash ID is not an
immediate priority; version, phase, target, and local reproduction steps provide
most of the current value.

## Suggested implementation order

1. Finish the active contract integration and restore a compiling workspace.
2. Extract an LLVM-independent frontend path and ship `zirk check`.
3. Centralize feature-stage status before more phases move simultaneously.
4. Replace flattened crate name resolution with real module identity.
5. Add the lossless persistent syntax layer and source-position indices.
6. Build formatter/LSP incremental behavior on that layer.
7. Add cache explanations and safe cache cleaning once caching exists.
8. Improve ICE metadata as the compiler reaches broader use.

This order minimizes rewrites: it preserves the working batch compiler, closes
the active pipeline gap, and creates the frontend boundary needed by later
tooling before committing to cache or editor representations.
