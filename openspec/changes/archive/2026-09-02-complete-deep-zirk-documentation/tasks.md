## 1. Block One — Standard Library

- [x] 1.1 Inventory every module and public contract in `ZIRK_STDLIB_SPEC.md`, map each to its handbook owner, and record gaps without inventing APIs.
- [x] 1.2 Deepen common API conventions plus `std.io`, `std.fs`, `std.path`, and resource-oriented APIs with signatures, results, permissions, cancellation, examples, failures, and platform behavior.
- [x] 1.3 Deepen `std.text`, `std.collections`, `std.time`, `std.task`, and `std.parallel` with type relationships, operations, complexity, mutation/iteration rules, concurrency behavior, and examples.
- [x] 1.4 Deepen `std.net`, `std.http`, `std.json`, and `std.crypto` with security limits, streaming/backpressure, permissions, errors, cancellation, configuration, and complete examples.
- [x] 1.5 Deepen `std.testing`, `std.reflect`, `std.system`, and environment/process APIs with their exact boundaries, generated-descriptor model, permissions, diagnostics, and examples.
- [x] 1.6 Add standard-library indexes and cross-links by task, module, type, error, permission, blocking/cancellation, and complexity where useful.

## 2. Block One — Toolchain

- [x] 2.1 Deepen the compiler overview and frontend chapters for lexing, parsing, name resolution, type/flow analysis, decorator expansion, and diagnostic recovery.
- [x] 2.2 Deepen portable IR, LLVM backend, native code generation, ABI/IR compatibility, debug/release profiles, optimization, and performance-goal chapters.
- [x] 2.3 Deepen incremental compilation, cache invalidation, reproducibility, targets, cross-compilation, linkers, SDK/native dependency checks, and artifact layout.
- [x] 2.4 Deepen CLI, formatter, linter, LSP, debugger, documentation generation, structured output, exit behavior, and editor workflows with commands and examples.
- [x] 2.5 Audit Block One examples, terminology, implementation status, source fidelity, SUMMARY membership, previous/next navigation, local links, and code fences before beginning Block Two.

## 3. Block Two — Testing

- [x] 3.1 Deepen unit, E2E, assertion/expectation, fixtures, setup/cleanup, failure reporting, filtering, and runner behavior with complete test files and CLI sessions.
- [x] 3.2 Deepen test permissions, sandboxing, deterministic time/randomness/environment, concurrent-test safety, cancellation, leak detection, and CI policy.
- [x] 3.3 Deepen benchmarks with warm-up, sampling, optimization profiles, statistical output, regression thresholds, resource measurement, and reproducible examples.

## 4. Block Two — Native and Low-Level

- [x] 4.1 Deepen C ABI import/export, layout, calling conventions, name mangling, ownership, errors, callbacks, strings, arrays, and complete interop examples.
- [x] 4.2 Deepen static/dynamic/native dependencies, loader behavior, packaging, target compatibility, symbol/version failures, and platform-specific diagnostics.
- [x] 4.3 Deepen pointers, native views, unsafe boundaries, rollback/commit, intrinsics, SIMD, vectorization, atomics, and undefined-behavior limits with valid and invalid examples.
- [x] 4.4 Audit Block Two examples, terminology, safety claims, implementation status, source fidelity, navigation, local links, and code fences before beginning Block Three.

## 5. Block Three — Tutorials

- [x] 5.1 Rewrite the first-project and file-processor tutorials as complete progressive projects with layout, `init.zrk`, commands, output, errors, permissions, tests, and troubleshooting.
- [x] 5.2 Rewrite HTTP service, concurrent pipeline, library publication, C interop, and decorator tutorials as end-to-end projects using only documented contracts.
- [x] 5.3 Add explicit prerequisite, concept-owner, next-step, implementation-status, expected-output, and failure-recovery links to every tutorial.

## 6. Block Three — Reference and Finalization

- [x] 6.1 Complete grammar, keywords, operators, literals, decorators, diagnostics, CLI, `init.zrk`, permissions, standard-library, compatibility, and feature-status reference pages.
- [x] 6.2 Complete glossary, normative-source index, implementation reading orders, exclusions, migration/compatibility guidance, and current-limitations appendices.
- [x] 6.3 Replace every archive-generated `Purpose: TBD` in active main OpenSpec specs with a concise capability-specific purpose and validate all specs strictly.
- [x] 6.4 Audit all substantive short chapters and either deepen them or confirm they are intentional indexes/orientation pages; do not pad them to meet a length target.
- [x] 6.5 Verify all examples, local links, anchors, SUMMARY entries, previous/next navigation, terminology, source ownership, code fences, and implementation-status notices across the full handbook.
- [x] 6.6 Run repository documentation checks, `git diff --check`, and `openspec validate --all --strict`; record unrelated implementation failures without modifying the existing Rust worktree changes.
- [x] 6.7 Publish a final documentation coverage report and mark the Markdown handbook ready to serve as the content source for the future website.
