# Zirk Documentation and OpenSpec Consistency Audit

**Audit date:** 20 August 2026  
**Scope:** all Markdown documentation under `docs/`, all main OpenSpec capability
specifications under `openspec/specs/`, and all non-archived OpenSpec changes.  
**Mode:** read-only semantic audit. This report does not change language rules,
documentation, specifications, roadmap state, or implementation code.

> **Remediation status — 20 August 2026:** every finding in this baseline was
> addressed by the follow-up documentation change. The original findings remain
> below as an audit trail. See **Applied corrections and rationale** at the end
> for the exact outcome and reading guidance.

## Executive summary

The repository has a strong body of language design, but it is not yet safe to
treat every document or every main OpenSpec spec as an interchangeable source
of truth. The newest checkpoints are internally coherent in many core areas,
but later standard-library and decorator work introduced invalid examples and
some rules were never propagated to every authority layer. Phase closeouts also
advanced faster than the roadmap, agent startup prompt, feature-status pages,
and implementation limitations were updated.

The audit found:

- **5 critical consistency failures** that can lead an implementation agent to
  implement observably different language behavior;
- **9 high-priority authority, roadmap, and syntax problems**;
- **9 medium-priority documentation completeness or lifecycle problems**;
- no broken local Markdown links in the handbook;
- all 34 OpenSpec items pass `openspec validate --all --strict`, demonstrating
  that schema validity does not imply semantic consistency.

The most urgent repair order is:

1. resolve the contradictory resource and environment contracts;
2. normalize `Result` variants and Zirk type/function syntax in examples;
3. establish one explicit authority order that includes decorators and
   standard-library decisions;
4. update/split the roadmap and the implementation-status documents;
5. finish the active deep-documentation change and its final global audit.

## How sources should currently be read

Until the findings below are corrected, use this conservative order:

1. `docs/ZIRK_SPEC_FINAL.md` for scope, exclusions, and authorial checkpoints.
2. The newest consolidated semantics document for the relevant domain:
   `CORE_LANGUAGE_SEMANTICS.md`,
   `ERROR_RESOURCE_PERMISSION_SEMANTICS.md`,
   `MEMORY_AND_UNSAFE_SEMANTICS.md`,
   `STRUCTURED_CONCURRENCY_SEMANTICS.md`, or
   `DECORATOR_SEMANTICS.md`.
3. The specialized language, runtime, compiler, or standard-library spec.
4. Main OpenSpec capability specs, except where they explicitly describe a
   temporary implementation narrowing.
5. The handbook as explanatory material, checking examples against the layers
   above.
6. ADRs, archived OpenSpec changes, `docs/01_plantilla_zirk.md`, phase designs,
   and phase tasks as historical rationale or implementation evidence, not as
   permission to override a later language checkpoint.

This order is an audit recommendation, not a new normative rule. Finding H-3
explains why the repository does not currently state it consistently.

## Critical findings

### C-1 — The main resource spec requires both observable and silent close failures

**Evidence**

- `openspec/specs/zirk-resources/spec.md:44-49` requires a close failure to be
  preserved as `Close`, `BodyAndClose`, or a suppressed exception.
- The same file at `openspec/specs/zirk-resources/spec.md:63-78` requires the
  `close()` result to be discarded and the failure to be silent “in this pass”.
- `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md:151-164` defines preservation of
  every outcome as the final authorial behavior.

**Why it matters**

An implementation agent following the main capability spec can correctly claim
conformance while implementing either opposite behavior. Silent cleanup failure
also contradicts Zirk's stated safety philosophy.

**Recommended resolution**

Keep final language behavior in `openspec/specs/zirk-resources/spec.md`. Move the
temporary Phase 4c narrowing into feature-status or implementation-tracking
material. A normative requirement and its temporary implementation exception
must not coexist as two `SHALL` rules in one main spec.

### C-2 — Standard-library chapters use the wrong success variant for `Result`

**Evidence**

- The authoritative result model is `Ok(T) | Error(E)` in
  `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md:23-28`,
  `docs/ZIRK_LANGUAGE_SPEC.md:420`, and
  `openspec/specs/zirk-errors/spec.md:6-16`.
- `docs/handbook/04-standard-library/11-std-net.md` uses `Value(...)` for
  `IPAddress.parse`, `DNS.resolve`, `TCPStream.connect`, listener binding, and
  UDP/WebSocket operations that are described as returning `Result`.
- `docs/handbook/04-standard-library/12-std-http.md` uses `Value(...)` for URL,
  HTTP, query, JSON, and WebSocket results.
- `docs/handbook/04-standard-library/18-std-environment.md:17-23`, `:43-49`, and
  `:104-110` use `Value(...)` for `Result`-returning environment operations.
- `docs/handbook/04-standard-library/15-std-testing.md:121` explicitly calls
  `Value/Error` the `Result` model.
- `Value/End/Error` is valid only for stream-like enums such as
  `ReadResult<T>` and `ReceiveResult<T>`.

**Why it matters**

These examples teach code that cannot match the declared `Result` enum and blur
the intentional distinction between an operation result and a stream outcome.

**Recommended resolution**

Use `Ok(...)` for every `Result<T,E>` success branch. Retain `Value(...)` only
for explicitly declared multi-state stream/read/receive enums.

### C-3 — Final resource semantics and the implemented-resource status are mixed as one contract

This is broader than C-1. `openspec/specs/zirk-resources/spec.md:10-22` says the
main capability currently permits only one acquisition and excludes
cancellation, grouped acquisition, surfaced close errors, transfer, and
dependent lifetimes. Later requirements in the same spec mandate all of those
final behaviors. The handbook correctly calls grouped acquisition normative but
not implemented at `docs/handbook/02-handbook/16-resources/02-match-with.md:16-19`.

**Recommended resolution**

Make the main spec describe the final contract only. Track compiler coverage in
the feature-status matrix or in phase-specific change artifacts. Apply the same
separation to `openspec/specs/zirk-errors/spec.md:38-53`, where final throwable
immutability, suppressed errors, and real stack traces are immediately narrowed
to Phase 4b stubs.

### C-4 — Environment APIs have two incompatible return contracts

**Evidence**

- `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md:233-242` says
  `Env.get_or_null` returns `String?`, `Env.get_or` returns `String`, and
  `Env.contains` returns `Boolean`.
- The newer `docs/ZIRK_STDLIB_SPEC.md:605-613`,
  `openspec/specs/zirk-permissions/spec.md:82-99`, and
  `docs/handbook/04-standard-library/18-std-environment.md:27-35` say every
  operation returns `Result` to preserve permission, encoding, and platform
  failures.
- The source map ranks the older error/permission checkpoint above the stdlib
  spec, so “newer and safer” does not automatically settle the conflict.

**Why it matters**

This changes public signatures, pattern matching, error propagation, and whether
permission denial can be accidentally hidden.

**Recommended resolution**

Adopt one signature table in the permission checkpoint, stdlib spec, OpenSpec,
and handbook. The all-`Result` contract is consistent with the later security
decision, but that conclusion must be synchronized into the higher-ranked
source rather than inferred by agents.

### C-5 — Two accepted binding features are absent from the normative documentation

Repository-wide searches found no current language/spec/handbook definition for:

- simultaneous assignment/swapping such as `x, y = y, x`, including exact
  arity, evaluate-before-write behavior, type compatibility, and rejection for
  `inmut`/`inmut::strict` destinations;
- multiple declarations such as `mut a, b, c: String;`, including type defaults
  and initializer arity.

The only “swap” hit is an unrelated byte-swap intrinsic in the historical
template. These rules were accepted in design discussion but never became a
current source of truth.

**Why it matters**

An agent cannot implement these features correctly from the repository and may
reasonably reject them as nonexistent.

**Recommended resolution**

Add grammar, typing, evaluation-order, immutability, diagnostic, and roadmap
ownership requirements before implementation.

## High-priority findings

### H-1 — Function declaration syntax is inconsistent

Canonical declarations use `fn name(...): ReturnType`; `=>` belongs to callable
types and lambda bodies.

Invalid declaration examples remain in:

- `docs/CORE_LANGUAGE_SEMANTICS.md:104`, `:172-173`;
- `docs/handbook/02-handbook/07-functions/12-function-types-and-callable-values.md:7`;
- `docs/handbook/04-standard-library/12-std-http.md:323,359`;
- `docs/handbook/04-standard-library/15-std-testing.md:257,313`.

The completed-but-unarchived decorator change additionally uses TypeScript-like
`fn report() -> Report` at
`openspec/changes/document-complete-decorator-semantics/design.md:72`.

### H-2 — Decorator documentation uses invalid generic type syntax

Zirk generic applications use angle brackets, but these files repeatedly use
`Result(...)` and nested `List(...)`:

- `docs/DECORATOR_SEMANTICS.md:19,43,161,194,245,269`;
- `docs/handbook/06-metaprogramming/01-decorators.md:8`;
- `docs/handbook/10-tutorials/07-write-a-decorator.md:17`.

For example, `Result(Report, ReportError)` must be written according to the
canonical `Result<Report, ReportError>` type syntax. These occur in the newest
decorator authority, not only historical material.

### H-3 — The authority hierarchy is inconsistent and incomplete

- `docs/handbook/_editorial/source-map.md:5-16` includes
  `DECORATOR_SEMANTICS.md` in the authority order.
- `docs/ZIRK_SPEC_FINAL.md:88-108` and
  `docs/init/ZIRK_AGENT_PROMPT.md:38-60` omit it from their normative source
  lists and refer to only four consolidated semantic documents.
- `docs/init/ZIRK_AGENT_PROMPT.md:63-64` says ADRs carry the same weight as the
  specs, although ADRs such as the D9 closure investigation describe temporary
  implementation limits superseded by final callable semantics.
- `docs/handbook/_editorial/source-map.md:18-21` still discusses transplanting
  authorial corrections onto a future latest `develop`, wording left over from
  an old worktree workflow.

Agents need an explicit distinction between normative language decisions,
architecture decisions, implementation evidence, and historical phase limits.

### H-4 — Callable types and escaping closures have no unambiguous roadmap owner

- Roadmap Phase 2 claims complete lambdas/closures
  (`docs/init/ZIRK_ROADMAP.md:76-84`).
- The implementation closeout says callable type annotations and escaping
  closures remain blocked by D9 (`docs/init/ZIRK_AGENT_PROMPT.md:187-195` and
  current parser diagnostics).
- No later roadmap phase explicitly owns enabling `Fn(...) => R` in type
  positions and closure escape.
- `openspec/specs/zirk-feature-phasing/spec.md:7-28` requires every normative
  feature to have exactly one owning phase.

This is a direct breach of the phasing spec and recreates the original closure
planning ambiguity.

### H-5 — The startup prompt is stale after Phase 4a/4b/4c

`docs/init/ZIRK_AGENT_PROMPT.md:299-308` still says the next phase is all of
Phase 4, including `Result`, exceptions, and resources. Archived, fully checked
changes now exist for:

- `fase-4a-errores`;
- `fase-4b-excepciones`;
- `fase-4c-recursos`.

The remaining memory/unsafe work must not make the already closed slices appear
unstarted. The prompt also calls a local declaration `let` at line 220 even
though Zirk declarations are `mut` or `inmut`.

### H-6 — The roadmap has not been updated as a living implementation plan

`docs/init/ZIRK_ROADMAP.md` records only Phase 0 as complete. It does not record
the completed Phase 1, 2, 3, 3b, 4a, 4b, and 4c changes, nor split the remaining
Phase 4 memory work from completed error/exception/resource work. This conflicts
with its own statement that it is a living document (`:286-292`).

The handbook roadmap at `docs/handbook/13-appendices/08-roadmap.md` is only a
generic paragraph and cannot substitute for a current phase/status table.

### H-7 — Current Limitations contradicts already completed specification work

`docs/handbook/13-appendices/07-current-limitations.md:3` says normative docs
still lack a precedence table, stable diagnostic-code registry, and final
details for ranges, slicing, traditional `for`, alternative patterns,
value-class syntax, and generators.

The language spec, grammar OpenSpec, operator reference, handbook, and compiler
prompt already document most of these. Stable diagnostic codes are described as
implemented from Phase 0 in `docs/init/ZIRK_AGENT_PROMPT.md:112-122`.

### H-8 — Feature Status is too stale and coarse to support implementation decisions

`docs/handbook/11-reference/12-feature-status.md:20-38` points `Result`,
exceptions, and resources only to a “later compiler/runtime phase”, despite the
completed 4a/4b/4c implementation changes. It also groups features with very
different pipeline coverage under broad labels such as “Phase 3” or “phased
compiler work”.

The page's own contract requires repository revision and evidence before an
implementation claim, but the table provides neither revision nor evidence.

### H-9 — OpenSpec main specs mix final language conformance and temporary pipeline state

In addition to resources/errors:

- `openspec/specs/zirk-object-memory/spec.md:19-34` normatively requires the
  runtime never to free allocated objects “in this phase”.
- `openspec/specs/zirk-callables/spec.md:6-34` defines final escaping closures,
  while the parser intentionally rejects callable annotations today.

Both facts can be documented, but not as one undifferentiated notion of spec
conformance. A language capability spec should state target behavior; a feature
matrix/change/task should state current pipeline coverage.

## Medium-priority findings

### M-1 — Fifteen main OpenSpec capabilities still have placeholder purposes

Fifteen files under `openspec/specs/` contain:

> `TBD - created by archiving change ... Update Purpose after archive.`

This is already task 6.3 of `complete-deep-zirk-documentation`, but remains
unfinished. Examples include `zirk-errors`, `zirk-resources`, `zirk-callables`,
`zirk-scalars`, `zirk-temporal-types`, and `zirk-permissions`.

### M-2 — OpenSpec language policy is inconsistent

The project says normative specifications and the codebase are English, while
many main OpenSpec requirements remain Spanish or mix both languages within one
capability (`zirk-grammar`, `zirk-classes`, `zirk-scalars`, `zirk-object-memory`,
and others). `docs/init/ZIRK_AGENT_PROMPT.md:7-18` exempts OpenSpec working
artifacts, but archived deltas have become main capability specs. The policy
does not say whether main specs keep that exemption after archival.

### M-3 — Standard-library decisions lack dedicated OpenSpec capability coverage

There are 32 main OpenSpec capabilities, but none dedicated to `std.io`,
`std.fs`, `std.net`, `std.http`, `std.json`, `std.crypto`, testing, reflection,
or the standard library as a whole. The latest detailed APIs live primarily in
`docs/ZIRK_STDLIB_SPEC.md` and handbook chapters. This is why the environment
signature conflict and `Result`-variant errors can pass strict OpenSpec
validation.

### M-4 — Completed decorator change is still active

`openspec list --json` reports `document-complete-decorator-semantics` as
22/22 complete but not archived. Its delta specs duplicate already synchronized
main specs, and its design still contains invalid `->` syntax. Keeping a
completed change active makes agents unsure whether main specs or the delta are
the current edit surface.

### M-5 — A no-task website change remains active

`openspec list --json` reports `document-zirk-language-site` with zero tasks,
but no actionable artifacts were found under its change directory. It is stale
lifecycle state rather than a usable proposal.

### M-6 — The deep-documentation change is only 7/28 complete

`openspec/changes/complete-deep-zirk-documentation/tasks.md` shows all standard
library tasks and only the first toolchain task complete. Remaining work covers
backend/toolchain, testing, low-level/native APIs, tutorials, reference,
placeholder purposes, examples, navigation, and final website readiness.

Therefore the current handbook must not yet be described as completely audited
or website-ready.

### M-7 — Large parts of the handbook remain outline-sized

Of 375 handbook Markdown files, **103 have 12 lines or fewer**. Some are valid
indexes, but the concentration matches unfinished tasks:

- 13 toolchain chapters;
- 7 testing chapters;
- 9 native/low-level chapters;
- 10 package chapters;
- 6 tutorial chapters;
- 3 reference chapters;
- 6 appendices.

Examples such as portable IR, LLVM backend, debugger, formatter, package
security, C ABI, benchmarks, and several tutorials are not deep enough to be an
implementation source. Task 6.4 correctly requires reviewing them individually
rather than padding all to a fixed size.

### M-8 — The publication audit certificate is stale

`docs/handbook/_editorial/source-map.md:43-61` certifies an audit of 334
documents on 13/15 August. The handbook now contains 375 Markdown files and has
received major standard-library, toolchain, decorator, and semantic additions
after that certificate. Local links still resolve, but its claims that examples
and semantic statements were globally reviewed no longer describe the current
content.

### M-9 — Master-spec metadata understates its current role

`docs/ZIRK_SPEC_FINAL.md:3-4` still says “Status: initial normative design” and
is dated 12 August, while its body contains authoritative checkpoints from 15,
16, and 17 August. This does not change semantics but makes it difficult for an
agent to determine maturity and last normative revision from the header.

## Confirmed consistent areas

The audit did **not** find active contradictions in these accepted decisions:

- `Float16`–`Float128`, `Float == Float64`, valid infinities, and no valid NaN;
- `Char` as exactly one extended Unicode grapheme;
- mutable shared `String` with `inmut` rebinding protection and
  `inmut::strict` deep immutability;
- whole-reference aliasing versus independent projection reads;
- attributes plus `get_`/`set_` methods, with no property declaration;
- concrete-class `extends` versus contract/abstract/interface/trait
  `implements`;
- `Fn(P...) => R` as the preferred alias once callable annotations are
  available;
- the absence of `async fn`, general `comptime`, general `defer`, Result `?`,
  public ownership/RC, and runtime decorator retention;
- exactly five decorator targets: class, attribute, function, method, parameter;
- signed, external, location-bound, requester-aware permission consent;
- `Task.settled` and structured cancellation semantics;
- canonical imports such as `import { HTTPClient } from std.http;`;
- regex literal syntax `re'...'`;
- named-argument shorthand `url:` meaning `url: url`.

## Validation evidence

- Markdown files examined: **406 under `docs/`** and **178 under `openspec/`**
  at audit start.
- Handbook Markdown files: **375**.
- Broken local handbook links: **0**.
- Active OpenSpec changes:
  - `complete-deep-zirk-documentation`: 7/28;
  - `document-complete-decorator-semantics`: 22/22, still active;
  - `document-zirk-language-site`: 0/0.
- `openspec validate --all --strict --json`: **34/34 valid**, with one
  informational long-requirement warning in `zirk-permissions`.

## Proposed remediation sequence

This section prioritizes future work; it does not apply any correction.

1. Create a small consistency change owning C-1 through C-5 and H-1 through
   H-4. These affect executable meaning or syntax.
2. Update the master authority list and startup prompt, explicitly classifying
   ADRs and phase documents as architecture/history/implementation evidence.
3. Split roadmap Phase 4 into completed and remaining slices; assign callable
   annotations/escaping closures and the missing binding features to phases.
4. Replace the generic feature-status table with per-feature pipeline stages
   and repository evidence.
5. Finish `complete-deep-zirk-documentation` by blocks, then rerun its intended
   full example and navigation audit.
6. Synchronize standard-library capabilities into OpenSpec or explicitly state
   that `ZIRK_STDLIB_SPEC.md` is their sole normative owner.
7. Archive or remove stale active changes only after confirming their deltas are
   synchronized.
8. Reissue the publication audit certificate against the final document count
   and revision.

## Conclusion

The repository is detailed enough to guide substantial implementation work,
but it is **not yet safe for an agent to read arbitrary documentation or main
OpenSpec files without applying source precedence and implementation-status
judgment**. The highest risk is not missing prose; it is that final semantics,
temporary phase limitations, and newer handbook APIs sometimes coexist as if
they had equal authority. Resolving the critical and high-priority findings
will make the documentation a reliable implementation source; completing the
remaining deep-documentation tasks will make it a complete public handbook.

## Applied corrections and rationale

This section records the remediation performed after the baseline audit. It is
the handoff for the principal compiler/documentation agent.

### 1. Final semantics separated from temporary implementation coverage

- Removed the Phase 4c rule that required silent close-failure loss from
  `openspec/specs/zirk-resources/spec.md`. The main capability now specifies
  final grouped cleanup, composed failures, transfer, and lifetime behavior.
- Removed Phase 4b stub behavior from the normative throwable requirement in
  `openspec/specs/zirk-errors/spec.md`.
- Removed “objects are never freed in this phase” from the main object-memory
  capability. Transitional allocation is now reported in Feature Status rather
  than presented as final conformance.
- Expanded Feature Status with separate definition, delivered evidence, and
  remaining material work for every major subsystem.

**Why:** a main capability cannot contain two opposite `SHALL` rules or make a
temporary pipeline limit indistinguishable from final language semantics.

### 2. `Result` and stream outcomes normalized

- Replaced `Value(...)` with `Ok(...)` throughout network, HTTP, environment,
  and testing examples whenever the declared return is `Result<T,E>`.
- Preserved `Value/End/Error` only for explicitly declared stream enums such as
  `ReadResult<T>` and `ReceiveResult<T>`.
- Renamed testing examples to `assert.is_ok` / `to_be_ok`, aligned with the
  canonical `Result` API.
- Added a main and delta `zirk-standard-library` OpenSpec capability that makes
  this distinction an explicit conformance requirement.

**Why:** `Result<T,E>` is `Ok(T) | Error(E)`; stream completion needs the
separate third `End` state and must not redefine Result.

### 3. Environment signatures synchronized

- Updated `ERROR_RESOURCE_PERMISSION_SEMANTICS.md` so `Env.get_or_null`,
  `Env.get_or`, and `Env.contains` return `Result`, matching the newer stdlib,
  permission OpenSpec, and handbook contract.
- Documented that fallback or nullability applies only to authorized absence;
  it never hides denial, encoding, or platform failure.

**Why:** permission non-disclosure and host failure cannot be represented safely
by a bare nullable, fallback value, or Boolean.

### 4. Invalid Zirk examples corrected

- Replaced function declaration `=>`/`->` with `: ReturnType` in core,
  callable, HTTP, testing, and decorator artifacts.
- Replaced decorator examples such as `Result(A, B)` and `List(T)` with
  `Result<A,B>` and `List<T>`.
- Corrected the startup prompt's historical `let` reference to `mut`/`inmut`.
- Corrected the keyword reference so `strict` and `value` remain contextual
  identifiers rather than reserved keywords.

**Why:** `=>` belongs to callable types/lambda bodies; generic applications use
angle brackets; Zirk has no `let` binding keyword.

### 5. Multiple declarations and simultaneous assignment made normative

- Added complete rules to `ZIRK_LANGUAGE_SPEC.md` and
  `CORE_LANGUAGE_SEMANTICS.md`.
- Added grammar and type-system OpenSpec requirements and valid/invalid
  scenarios.
- Added the handbook chapter **Multiple Bindings and Simultaneous Assignment**,
  inserted it into `SUMMARY.md`, and repaired adjacent navigation.
- Defined exact arity, all-sources-before-writes evaluation, left-to-right
  evaluation/commit, positional typing, duplicate-destination rejection,
  independent defaults, `inmut` rebinding rejection, and strict referent
  mutation rejection.

**Why:** these accepted features previously existed only in conversation, so an
agent could neither implement nor diagnose them from repository sources.

### 6. Authority and language-governance rules repaired

- Added `DECORATOR_SEMANTICS.md` to the master spec and startup prompt.
- Updated the source map and normative-sources appendix with all five
  consolidated checkpoints and the specialized-spec layer.
- Clarified that ADRs are authoritative for unresolved internal architecture,
  but phase designs/tasks/archives cannot override later public semantics.
- Removed obsolete worktree-transplant language.
- Added `openspec/README.md`: public docs/code/diagnostics are English; internal
  OpenSpec may retain the reviewed language of an accepted engineering change,
  while new coherent requirements default to English.

**Why:** agents need one deterministic reading order and must not revive a
phase-local limitation from an ADR or archive after a later authorial decision.

### 7. Roadmap and current implementation state updated

- Marked Phases 1, 2, 3, and 3b complete for their explicit scope.
- Split Phase 4 into completed 4a (`Result`), 4b (explicit exceptions), and 4c
  (initial deterministic resources), plus pending 4d (callable/binding
  completion) and 4e (managed memory/unsafe completion).
- Assigned callable annotations/escaping closures and the two new binding forms
  to Phase 4d, satisfying the one-owner phasing rule.
- Updated `ZIRK_AGENT_PROMPT.md`, handbook Roadmap, Current Limitations, and
  Feature Status with exact delivered-versus-final distinctions.
- Updated master metadata to “living normative design”, with the latest
  checkpoint date.

**Why:** “complete phase” now means its scoped tested output, not every final
interaction mentioned by an older monolithic roadmap heading.

### 8. OpenSpec lifecycle and capability hygiene completed

- Replaced every archive-generated `Purpose: TBD` in main capability specs.
- Added the standard-library capability owner and synchronized delta.
- Archived `document-complete-decorator-semantics` after confirming 22/22 tasks,
  complete artifacts, and requirement-for-requirement synchronization with main
  decorator/grammar specs.
- Removed only the empty `document-zirk-language-site` directory; it contained
  no proposal, tasks, specs, or implementation.
- Left the unrelated active `fase-4d-runtimeerror` change untouched.

**Why:** active change listings now describe actionable work and main specs have
enough purpose/context to be discovered correctly.

### 9. Deep-documentation blocks completed

- Deepened portable IR, LLVM/backend, diagnostics, CLI, formatter, linter, LSP,
  debugger, profiles, optimization, incrementality, cross-compilation,
  ABI/IR compatibility, and performance measurement.
- Deepened unit/E2E testing, assertions, permissions, deterministic concurrent
  testing, runner/reporting, and benchmarking.
- Deepened C ABI/export, dynamic loading, mangling, intrinsics, SIMD,
  vectorization, and the external native alternative to inline assembly.
- Deepened package anatomy/API/IR, dependency commands, resolution, lock
  verification, publication, native dependencies, and supply-chain security.
- Rewrote all seven tutorials with status, prerequisites, project/code flow,
  expected success, failure recovery, testing, permissions, concept owners, and
  next steps.
- Added `_editorial/short-chapter-review.md` to distinguish intentionally short
  orientation/index pages from substantive pages that required expansion.

**Why:** a present filename and valid navigation are not enough for public or
implementation-grade documentation.

### 10. Final verification and new baseline

- Handbook published files checked: **372** (excluding `SUMMARY.md` and internal
  editorial records).
- Every published file appears exactly once in `SUMMARY.md`.
- Missing previous/next navigation: **0**.
- Broken local handbook links: **0**.
- Broken local heading anchors: **0**.
- Unbalanced fenced-code blocks: **0**.
- Remaining `Purpose: TBD` in main specs: **0**.
- Obsolete function/import/generic syntax targeted by this audit: **0**.
- `openspec validate --all --strict --json`: **35/35 valid** at remediation
  validation time; the only issue is a non-blocking informational suggestion to
  shorten one long permission requirement.
- The handbook now contains **377 Markdown files**. Short pages fell from 103 to
  58, with the remainder reviewed as orientation/index/focused explanation
  rather than padded to a line target.

**Why:** this is the replacement publication baseline. Future semantic or API
changes must update the canonical owner, derivative documentation, Feature
Status, roadmap ownership, and relevant OpenSpec capability together.

### Principal-agent reading order after remediation

1. `docs/ZIRK_SPEC_FINAL.md`.
2. The relevant consolidated checkpoint among core, errors/resources/
   permissions, memory/unsafe, structured concurrency, and decorators.
3. The specialized language/runtime/stdlib/compiler spec.
4. The corresponding main OpenSpec capability for testable requirements.
5. `docs/handbook/11-reference/12-feature-status.md` and
   `docs/init/ZIRK_ROADMAP.md` for current delivery ownership.
6. Handbook chapters for explanation and examples.
7. ADRs and archived changes only for architecture rationale or implementation
   evidence, never to supersede steps 1–4.
