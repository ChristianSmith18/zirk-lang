## 1. Architecture and coverage inventory

- [x] 1.1 Add an ADR that distinguishes browser-hosted IR interpretation from a future Zirk-to-WebAssembly compilation target.
- [x] 1.2 Inventory every current `IrType` variant and classify it as directly executable, executable through runtime-core, or browser-incompatible.
- [x] 1.3 Inventory every current `InstKind` variant and classify it as directly executable, executable through runtime-core, or browser-incompatible.
- [x] 1.4 Inventory every current `Terminator`, intrinsic, extern declaration, and host effect and record its browser policy.
- [x] 1.5 Define the first public browser compatibility baseline from the completed inventories and current feature-status evidence.
- [x] 1.6 Record default and hard-maximum limits for source bytes, diagnostics, instructions, call depth, heap, output bytes, output events, and stack frames.
- [x] 1.7 Select mature pinned WASM binding and contract-serialization dependencies after compatibility, license, and release-age review.
- [x] 1.8 Add CI assertions that the checked-in support matrix remains exhaustive when an IR enum gains a variant.

## 2. Platform-neutral runtime semantics

- [x] 2.1 Add the `zirk-runtime-core` crate with documented responsibility and no native ABI, allocator, process, or browser dependency.
- [x] 2.2 Add workspace dependency edges from `zirk-runtime` and the future interpreter to `zirk-runtime-core` without changing public native symbols.
- [x] 2.3 Move pure scalar formatting and conversion behavior into runtime-core and keep native ABI wrappers delegating to it.
- [x] 2.4 Add parity tests proving native scalar formatting and conversion results are unchanged after extraction.
- [x] 2.5 Move exact-decimal representation and pure arithmetic behavior into runtime-core.
- [x] 2.6 Add exact-decimal parity tests for parsing, arithmetic, comparison, rounding, formatting, overflow, and division by zero.
- [x] 2.7 Move pure Unicode normalization, grapheme traversal, and string content operations into runtime-core.
- [x] 2.8 Add string parity tests for NFC/NFD equality, grapheme indexing, mutation helpers, search, and non-ASCII formatting.
- [x] 2.9 Move pure `Char` classification and conversion behavior into runtime-core and add native parity tests.
- [x] 2.10 Move pure regex construction and match/find/replace behavior into runtime-core and add native parity tests.
- [x] 2.11 Move pure duration, range, and delivered temporal value behavior into runtime-core and add native parity tests.
- [x] 2.12 Extract pure array, list, map, and set value operations that the delivered compiler surface requires and add native parity tests.
- [x] 2.13 Verify `zirk-runtime` still builds as both `staticlib` and `rlib` with unchanged exported symbol names.
- [x] 2.14 Verify `zirk-runtime-core` builds for `wasm32-unknown-unknown` without conditional native stubs.

## 3. Interpreter foundations

- [x] 3.1 Add the `zirk-interpreter` crate with documented boundaries and dependencies only on IR, diagnostics, and runtime-core.
- [x] 3.2 Define typed interpreter configuration with validated defaults and hard maxima for every resource limit.
- [x] 3.3 Define the closed interpreter `Value` representation for all directly supported scalar and nullable IR types.
- [x] 3.4 Define stable arena identifiers and heap entry types for identity-bearing strings, closures, objects, enums, collections, and runtime errors.
- [x] 3.5 Implement weighted heap accounting and reject allocations that would exceed the configured heap limit.
- [x] 3.6 Define frame state for function, block, instruction cursor, arguments, slots, produced values, environment, and return destination.
- [x] 3.7 Define structured completion, runtime failure, limit failure, capability failure, and internal failure result types.
- [x] 3.8 Define bounded Zirk stack-frame capture using function identities and IR source spans.
- [x] 3.9 Add constructor tests that reject invalid limit configurations and malformed unverified modules.
- [x] 3.10 Add lifecycle tests proving dropping an execution releases all arena-owned values.

## 4. Control flow and storage execution

- [x] 4.1 Write failing tests for constant instructions, result lookup, and typed operand materialization.
- [x] 4.2 Implement constant instruction execution for every browser-compatible scalar literal.
- [x] 4.3 Write failing tests for local slot initialization, load, store, and simultaneous-assignment behavior.
- [x] 4.4 Implement local slot initialization, load, store, and required copy/value semantics.
- [x] 4.5 Write failing tests for `Jump`, `Branch`, `Return`, loop back-edges, and root completion.
- [x] 4.6 Implement the instruction cursor and terminator state machine for basic-block execution.
- [x] 4.7 Write a failing test for execution reaching `Unreachable`.
- [x] 4.8 Return a structured internal failure with source context when `Unreachable` is executed.
- [x] 4.9 Charge each executed instruction and stop exactly at instruction-budget exhaustion.
- [x] 4.10 Add deterministic tests for finite loops, infinite loops, zero budget, and the exact exhaustion boundary.

## 5. Scalar and conversion semantics

- [x] 5.1 Write table-driven failing tests for unary operations across every supported integer and binary-float width.
- [x] 5.2 Implement unary operations with width-aware overflow and controlled invalid-domain failures.
- [x] 5.3 Write table-driven failing tests for integer arithmetic, comparison, shifts, division, modulo, and overflow across signedness and widths.
- [x] 5.4 Implement integer binary operations and conversions without silent wrapping.
- [x] 5.5 Write failing tests for binary-float arithmetic, comparison, conversion, and controlled NaN/invalid-domain behavior.
- [x] 5.6 Implement binary-float operations according to the IR's explicit checks and declared widths.
- [x] 5.7 Write failing tests for exact-decimal constants, arithmetic, comparison, conversion, and formatting.
- [x] 5.8 Implement exact-decimal instructions by delegating to runtime-core.
- [x] 5.9 Write failing tests for boolean logic, null tests, checked casts, and type-test operations.
- [x] 5.10 Implement boolean, nullability, checked-cast, and type-test instructions.
- [x] 5.11 Add differential fixtures for scalar output, overflow, division by zero, conversion boundaries, and exact decimal behavior.

## 6. Calls, closures, and object model

- [x] 6.1 Write failing tests for direct calls, parameters, return values, void returns, and nested calls.
- [x] 6.2 Implement explicit call-frame push, argument binding, return propagation, and frame cleanup.
- [x] 6.3 Enforce call-depth limits before frame allocation and add exact-boundary recursion tests.
- [x] 6.4 Write failing tests for closure creation with empty and populated capture environments.
- [x] 6.5 Implement closure heap entries, capture copying, and indirect closure invocation.
- [x] 6.6 Write failing tests for callable values and indirect function invocation.
- [x] 6.7 Implement callable materialization and indirect call dispatch.
- [x] 6.8 Write failing tests for object allocation, inherited field layout, field reads, and field writes.
- [x] 6.9 Implement object heap allocation and offset-based field access using IR layouts.
- [x] 6.10 Write failing tests for record/value construction, copying, field access, and structural equality.
- [x] 6.11 Implement supported record/value semantics without introducing observable heap identity where the language forbids it.
- [x] 6.12 Write failing tests for enum construction, discriminants, payloads, and lowered match control flow.
- [x] 6.13 Implement enum values and discriminant/payload instructions.
- [x] 6.14 Write failing tests for overridden object methods and contract-table dispatch.
- [x] 6.15 Implement virtual and contract dispatch from the module's layout and table metadata.
- [x] 6.16 Add differential fixtures for closures, escaping captures, inheritance, records, enums, structural equality, and dispatch.

## 7. Exceptions, cleanup, and runtime errors

- [x] 7.1 Write failing tests for fatal failures, typed throws, catches represented by lowered control flow, and uncaught root failures.
- [x] 7.2 Implement interpreter execution signals for throw, rethrow, fatal failure, return, and ordinary continuation.
- [x] 7.3 Preserve throwable identity, type, message, source span, and bounded interpreted stack frames.
- [x] 7.4 Write failing tests for primary and suppressed failure ordering during cleanup.
- [x] 7.5 Implement suppressed-failure attachment required by existing IR cleanup operations that are browser-compatible.
- [x] 7.6 Ensure all user-triggerable arithmetic, bounds, cast, regex, and collection failures return structured runtime failures rather than panicking.
- [x] 7.7 Add malformed-module and fuzz-style regression tests around operand kinds, block targets, layouts, and failure propagation.
- [x] 7.8 Add differential fixtures for throw/catch, exact rethrow, fatal errors, bounds errors, failed casts, and suppressed cleanup failures.

## 8. Text, regex, ranges, temporal values, and collections

- [x] 8.1 Write failing interpreter tests for string construction, concatenation, grapheme count/indexing, mutation, search, repetition, equality, identity, and formatting.
- [x] 8.2 Implement delivered string instructions and intrinsics through runtime-core with stable interpreter identities.
- [x] 8.3 Write failing interpreter tests for `Char` construction, classification, case conversion, and formatting.
- [x] 8.4 Implement delivered `Char` instructions and intrinsics through runtime-core.
- [x] 8.5 Write failing interpreter tests for regex construction, matches, find, replace, invalid patterns, and configured input bounds.
- [x] 8.6 Implement delivered regex behavior through runtime-core with weighted work or explicit input limits.
- [x] 8.7 Write failing interpreter tests for duration, range, and delivered temporal operations.
- [x] 8.8 Implement delivered duration, range, and temporal behavior through runtime-core.
- [x] 8.9 Write failing tests for array construction, indexing, mutation, bounds failures, iteration primitives, and equality.
- [x] 8.10 Implement the delivered array instruction and intrinsic surface.
- [x] 8.11 Write failing tests for list construction, mutation, indexing, iteration primitives, and equality.
- [x] 8.12 Implement the delivered list instruction and intrinsic surface.
- [x] 8.13 Write failing tests for every delivered map and set operation included in the compatibility baseline.
- [x] 8.14 Implement browser-compatible delivered map and set behavior through runtime-core.
- [x] 8.15 Add differential fixtures for text, regex, duration, range, temporal, array, list, map, and set behavior included in the baseline.

## 9. Host output and compatibility preflight

- [x] 9.1 Define a host trait exposing only bounded output emission and cooperative cancellation checks.
- [x] 9.2 Write failing tests for ordered `print`, `println`, empty strings, multiple newlines, and non-ASCII output.
- [x] 9.3 Implement stdout intrinsics against the host output sink without writing native stdout.
- [x] 9.4 Enforce output byte and event limits before accepting each event and preserve an explicit truncation marker.
- [x] 9.5 Implement module preflight over every `IrType`, instruction, terminator, intrinsic, and extern declaration from the support matrix.
- [x] 9.6 Return deterministic capability diagnostics sorted by source location and stable capability code.
- [x] 9.7 Add rejection tests for extern calls, pointers, native slices, dependent references, pinning, unsafe journals, process, shell, filesystem, environment, secrets, network, and other excluded effects present in current IR.
- [x] 9.8 Add a test proving preflight reports all bounded incompatibilities and executes no entrypoint instruction.
- [x] 9.9 Add a test proving a module containing only baseline-compatible operations passes preflight.

## 10. In-memory browser compiler facade

- [x] 10.1 Add the `zirk-browser` crate with documented responsibility and `cdylib`/Rust library configuration appropriate for WASM tests.
- [x] 10.2 Refactor or add a shared in-memory frontend entrypoint that accepts source text and a virtual source name without filesystem access.
- [x] 10.3 Add parity tests proving file-backed CLI checking and in-memory checking return equivalent ordered diagnostics.
- [x] 10.4 Define versioned `check` and `run` request types with caller limits that can only tighten package maxima.
- [x] 10.5 Define versioned diagnostics, output events, completion, runtime failure, capability failure, limit failure, and internal failure response types.
- [x] 10.6 Implement source-byte validation before lexing.
- [x] 10.7 Implement bounded deterministic diagnostic conversion with compiler stage and source span fields.
- [x] 10.8 Implement `check` through the shared frontend and prohibit lowering or execution after compiler errors.
- [x] 10.9 Implement `run` through the shared frontend, IR lowering, verification, preflight, and interpreter.
- [x] 10.10 Add contract tests proving invalid source produces equivalent diagnostics from `check` and `run` and no runtime output.
- [x] 10.11 Add contract tests for normal completion, runtime failure, capability failure, each resource limit, and unexpected internal failure sanitization.
- [x] 10.12 Verify browser operations perform no filesystem, storage, HTTP, environment, process, shell, clock, random, or DOM access.

## 11. WebAssembly bindings and package production

- [x] 11.1 Add worker-compatible WASM initialization bindings without `window` or `document` access.
- [x] 11.2 Export versioned `check` and `run` operations and generated strict TypeScript declarations.
- [x] 11.3 Add an explicit initialization-state error for calls made before WASM initialization completes.
- [x] 11.4 Add boundary tests for malformed requests, unknown fields according to contract policy, and unsupported API versions.
- [x] 11.5 Add a pinned repository command that installs or locates the approved WASM build tools without floating versions.
- [x] 11.6 Add a reproducible package build command that emits the `.wasm`, ES-module glue, declarations, and required license notices.
- [x] 11.7 Generate a manifest containing API/compiler versions, clean source revision, tool versions, operations, limits, exclusions, sizes, and SHA-256 checksums.
- [x] 11.8 Add manifest verification that fails for stale revisions, missing files, size drift outside policy, or checksum mismatch.
- [x] 11.9 Add a dependency audit proving browser artifacts exclude LLVM, native linker code, native runtime ABI, collector, pointer, and stdout machinery.
- [x] 11.10 Add a module-worker smoke test that initializes the package and runs a Hello World fixture.
- [x] 11.11 Measure raw, gzip, and Brotli artifact sizes and enforce reviewed browser-package warning and error budgets.
- [x] 11.12 Document the exact clean-revision package handoff procedure for `zirk-lang-site`.

## 12. Conformance and verification

- [x] 12.1 Define the browser-compatible fixture metadata format with expected output, completion/failure category, and exclusions.
- [x] 12.2 Convert representative existing compiler/runtime examples into shared conformance fixtures without weakening native assertions.
- [x] 12.3 Add a host interpreter fixture runner.
- [x] 12.4 Add a native `zirk run` fixture runner with normalized observable results.
- [x] 12.5 Add a differential test harness that reports fixture-level output and failure differences.
- [x] 12.6 Add a browser-package contract runner for the generated ES module and WASM artifact.
- [x] 12.7 Add deterministic repeated-run tests proving no state, output, heap identity, or diagnostics leak between executions.
- [x] 12.8 Add stress tests for maximum accepted source, diagnostics, instruction count, call depth, heap use, output, regex input, and collection size.
- [x] 12.9 Run `cargo test -p zirk-runtime-core` and resolve all failures.
- [x] 12.10 Run `cargo test -p zirk-runtime` and resolve all regressions.
- [x] 12.11 Run `cargo test -p zirk-interpreter` and resolve all failures.
- [x] 12.12 Run `cargo test -p zirk-browser` on the host and through its WASM contract runner.
- [x] 12.13 Run `cargo test --workspace` and resolve all regressions.
- [x] 12.14 Run formatting, clippy, browser dependency audit, package integrity verification, and bundle-budget checks.

## 13. Documentation, status, and companion-site handoff

- [x] 13.1 Update compiler architecture documentation with runtime-core, interpreter, browser facade, and dependency boundaries.
- [x] 13.2 Update toolchain documentation with pinned browser-package prerequisites and reproducible build/verification commands.
- [x] 13.3 Update the README pipeline description to distinguish native code generation from optional browser interpretation.
- [x] 13.4 Update Feature Status with reviewed interpreter coverage and explicit browser exclusions backed by conformance evidence.
- [x] 13.5 Update Current Limitations to distinguish browser tooling from the deferred Zirk WebAssembly target.
- [x] 13.6 Update the compiler and handbook roadmaps with the reviewed phase placement for browser-hosted interpretation.
- [x] 13.7 Update relevant normative compiler/runtime documentation only where the new execution-engine contract requires it, without changing source-language semantics.
- [x] 13.8 Commit the completed `zirk-lang` change on the worktree branch and record its clean revision for artifact provenance.
- [x] 13.9 Build and verify the browser package from that exact clean compiler revision.
- [ ] 13.10 Apply and complete `add-browser-playground-execution` in `../zirk-lang-site` on its normal `develop` branch using the verified package.
- [ ] 13.11 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` from the committed compiler revision after human review of site-owned status claims.
- [ ] 13.12 Reconcile generated website content with the site playground integration without overwriting site-owned changes.
- [ ] 13.13 Run the complete compiler and website verification suites and review both diffs for contradictory availability or target claims.
- [ ] 13.14 Commit the companion website changes separately and record both repository revisions for an auditable release pair.
