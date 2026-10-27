## 1. Baseline and support matrix

- [x] 1.1 Inventory structural union/intersection alternatives accepted by `zirk-sema`, source operations, current IR forms, interpreter behavior, native layouts, and package boundaries; verify the delivered matrix is recorded in `design.md` before modifying support.
- [x] 1.2 Preserve and run the focused discriminated structural-union characterization fixtures; verify `cargo test -p zirk-cli --test end_to_end structural_union_discriminants_narrow_before_field_reads` and `cargo test -p zirk-cli --test object_engine_parity structural_union_discriminants_and_narrowing_match_across_engines` pass. The initial full end-to-end run exposed a nested generic-list specialization issue; it was repaired and the focused regression now passes.
- [x] 1.3 Add focused characterization tests for structural intersections and mixed alternatives currently rejected or unsupported; verify each unsupported combination fails at a source-located phase rather than panicking. Native/interpreter fixtures now cover a shape intersection and a shape/scalar union; `check::unsupported_structural_mixed_union_is_rejected_at_its_type_declaration` confirms unsupported `Shape | List<Int32>` reports E0423 at the alias.
- [x] 1.4 Turn the inventory into an explicit per-operation/per-representation matrix covering construction, assignment, reads/writes, narrowing, match, calls, returns, storage, captures, and target ABI; verify every matrix cell is marked supported with evidence or rejected before backend emission in the baseline table in `design.md`.

## 2. Structural type relations and contextual construction

- [x] 2.1 Complete canonical N-ary structural union/intersection relation solving, preserving alias diagnostics and source-order independence; verified by `cargo test -p zirk-sema` (845 passed).
- [x] 2.2 Complete structural source/destination assignability for union and intersection types; union branches and intersection constituents are covered by the semantic suite.
- [x] 2.3 Implement property capability composition for required/optional, read-only/mutable, and overlapping structural members; safe reads/writes and conflicts are covered by the semantic suite.
- [x] 2.4 Complete common callable-member checking using function variance without synthesizing overloads; compatible and incompatible signatures are covered by the semantic suite.
- [x] 2.5 Complete nested contextual object-literal selection against N-ary structural unions; unique candidate selection and ambiguous/mismatched diagnostics are covered by the semantic suite.
- [x] 2.6 Extend flow narrowing and exhaustiveness across structural union alternatives, nested discriminants, and property-presence tests; branch and exhaustiveness behavior is covered by the semantic suite.
- [x] 2.7 Ensure relation, normalization, and recursive-type limits abort without a partial answer; deterministic limit diagnostics are covered by the semantic suite.

## 3. Verified portable IR

- [x] 3.1 Define typed IR operations and metadata for structural union injection, tag inspection, projection, coercion, and intersection views; `cargo test -p zirk-ir` passes (294 tests).
- [x] 3.2 Extend the IR verifier to reject invalid tags, payload types, projections, and managed-reference descriptors before execution; malformed-module cases are covered by the IR suite.
- [x] 3.3 Complete lowering for supported structural composite construction, assignment, function arguments/results, local slots, captures, pattern matching, and common member reads/writes; IR and focused CLI fixtures pass.
- [x] 3.4 Ensure matrix cells without a verified representation emit a source-located diagnostic before backend emission; unsupported structural match and mixed union cases have negative CLI coverage.

## 4. Interpreter parity

- [x] 4.1 Extend interpreted composite values to preserve selected structural alternatives and active-payload managed-reference metadata; interpreter tests and native/interpreter parity cover slots, calls, returns, and captures.
- [x] 4.2 Implement structural union narrowing, projection, matching, and intersection views in the interpreter; focused parity fixtures verify selected arms and shared object identity.
- [x] 4.3 Add interpreter/native differential fixtures for structural and mixed-representation unions, intersections, and captured managed payloads; unsupported structural combinations are separately verified to fail before execution.

## 5. Native layouts, tracing, and package ABI

- [x] 5.1 Define target layout metadata for supported structural and mixed union alternatives, including size, alignment, tags, and active managed-reference paths; `cargo test -p zirk-codegen-llvm` passes (97 tests), with deterministic layout tests.
- [x] 5.2 Implement native construction, storage, argument/result ABI, capture, tag dispatch, and payload projection for the supported structural and mixed alternatives; focused native and parity fixtures pass.
- [x] 5.3 Make managed-reference tracing tag-aware; IR reference-map coverage and the forced-GC captured-payload E2E confirm active structural references remain live.
- [x] 5.4 Reject layouts that cannot use the supported representation before backend emission; unsupported mixed types and oversized payloads have source-level diagnostics.
- [x] 5.5 Enforce package representation compatibility metadata for union-typed exports; the stale structural metadata integration fixture rejects before execution.
- [x] 5.6 Run the focused native structural-composite suite on every supported CI target, including Windows x86_64 where the project claims support; the filtered structural run passes on Linux x86_64, Linux aarch64, Windows x86_64, and macOS aarch64 (GitHub Actions run 40497897350).
- [x] 5.7 Verify structural intersections compile to views over the original value with no duplicate allocation; native and interpreter parity confirms alias-visible mutation (`Ada:8`).

## 6. Open template types and generic inference

- [x] 6.1 Extend type-template parsing to retain fixed text segments, interpolation identity, and source spans for `"{K}Changed"`; verified in the parser suite (252 passed).
- [x] 6.2 Preserve finite template expansion and its budget while allowing a symbolic single open interpolation; parser and semantic suites pass.
- [x] 6.3 Implement deterministic inverse matching for one open interpolation with fixed prefix/suffix and string-singleton arguments; `"ageChanged"` infers exactly `"age"` in semantic tests.
- [x] 6.4 Integrate inverse inference with generic argument constraints, `String & keyof T`, aliases, and indexed access `T[K]`; valid and invalid key cases are covered by semantic tests.
- [x] 6.5 Apply the inferred indexed property type as contextual callback information; numeric callback inference and invalid callback uses are covered by semantic tests.
- [x] 6.6 Reject broad-String inference, mismatched prefix/suffix, multiple open holes, ambiguous patterns, and conflicting substitutions without widening; focused diagnostics are covered by semantic tests.
- [x] 6.7 Enforce deterministic pattern/generic relation budgets with no partial substitutions; the oversized-pattern limit test and semantic suite pass.
- [x] 6.8 Confirm open template types erase before IR/runtime and add no runtime value or package ABI payload; the IR specialization test verifies only the resolved callable type remains.
- [x] 6.9 Preserve method-local type parameters and their constraints in parameterized contract method signatures, including substitution of enclosing contract parameters such as `T` without capturing or overwriting method parameter `K`; alpha-equivalent class implementations are covered by the semantic fixture.
- [x] 6.10 Infer method-local substitutions after receiver/contract substitution, including inverse inference of `K` from `"{K}Changed"`, indexed access `T[K]`, and callback contextual typing for `person.on("ageChanged", ...)`; semantic and native/interpreter fixtures cover the concrete event keys `age` and `name`.
- [x] 6.11 Validate generic contract-method implementation conformance, including parameter/return compatibility and constraint equivalence, while retaining the existing no-overload rule; matching and incompatible generic bounds are covered by semantic fixtures.
- [x] 6.12 Lower generic contract calls and implementations so interpreter and native dynamic dispatch target the correct compile-time specialization without adding a runtime generic type value; native/interpreter parity passes, and an exported source-package contract fixture verifies compiled signature fingerprints and rejects tampered metadata before client emission.

## 7. LSP integration

- [x] 7.1 Consume canonical checker substitutions and structural alternatives for LSP diagnostics, hover, inlay information, and completion; `cargo test -p zirk-lsp` passes (45 tests).
- [x] 7.2 Add LSP fixtures for valid `person.on("ageChanged", ...)` inference, invalid event keys, generic contract-method hover/signature help, ambiguous structural literals, and narrowed structural completion; verify source ranges and categories.

## 8. Public documentation and examples

- [x] 8.1 Add handbook and demo examples for structural unions/intersections, discriminated construction, common-member reads, and open-template event inference; `demo/42-uniones-estructurales-y-plantillas-abiertas.zrk` runs successfully on the host.
- [x] 8.2 Update type-system and compiler-pipeline documentation to distinguish semantic acceptance, portable-IR support, interpreter support, native support, and target/package ABI evidence.
- [x] 8.3 Update feature status, current limitations, and roadmap after reviewing the support matrix; unsupported target/runtime evidence remains explicitly marked as incomplete.
- [x] 8.4 Add the public `Watched<T>.on<K ...>` example after generic contract methods are implemented, and verify the handbook, demo, CLI, and LSP all show the inferred callback property type.
- [x] 8.5 Commit the authoritative Zirk documentation/status changes, run `./scripts/sync-website-content.sh --audit-date 2026-10-27`, and review the generated `../zirk-lang-site` diff and status catalog; the generator validated 374 documents and 374 routes, recorded source revision `7e96bc9dce1f3c53ec87d142a8f361f408e903e9` and checksums, and the generated diff contains only expected provenance/checksum updates.
- [x] 8.6 Commit website synchronization separately as `d7df8608475d38aa0320b9745ce96c78059e1657`; `git diff --check` passed and the source and site worktrees are clean apart from this task's final evidence update.

## 9. Integrated validation

- [x] 9.1 Run focused crate suites for `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-interpreter`, `zirk-codegen-llvm`, feature-specific `zirk-cli` tests, and `zirk-lsp`; all focused feature tests pass. The full CLI suite was attempted but hung in an unrelated HTTP/WebSocket server integration test with no process timeout.
- [x] 9.2 Run the feature-filtered supported-target CI matrix and interpreter/native conformance fixtures; the structural filter (run 40497897350) and generic contract/open-template filter (run 40497901428) pass on Linux x86_64, Linux aarch64, Windows x86_64, and macOS aarch64. Host parity fixtures pass, and unsupported structural combinations have source-level rejection coverage; these focused jobs do not claim full-workspace CI.
- [x] 9.3 `openspec validate structural-type-runtime-and-open-templates --strict`, `cargo fmt --all -- --check`, `git diff --check`, the host demo, and the website generator's 374-document/374-route validation pass. Public status now distinguishes focused four-target CI evidence from the unrun full workspace matrix, and records remaining ABI/container/GC stress limitations.
