## Context

Zirk currently has a complete native pipeline:

```text
source -> lexer -> parser -> semantic analysis -> typed IR -> LLVM -> object -> native runtime
```

The public Angular site already provides a local editor-only playground, but its Run and Check actions are intentionally unavailable because there is no isolated execution service. The project does not want to operate a paid compilation service at this stage, so compilation and execution must occur entirely in the browser and deploy as static Firebase Hosting assets.

The LLVM backend cannot be embedded in a practical browser payload. The existing typed IR is nevertheless target-independent, verified before code generation, expressed as basic blocks and abstract allocations, and documented as the stable frontend/backend boundary. This permits a second execution engine without changing source semantics or making WebAssembly a Zirk output target.

The current runtime is Rust, but it is shaped as a native static library with a C ABI, raw handles, native stdout, and collector integration. Pure semantic operations for text, exact decimals, regexes, scalars, ranges, and temporal values are reusable in principle, while the ABI, native allocator, pointer model, and process I/O are not.

The browser consumer is `../zirk-lang-site`. Its coordinated change is `add-browser-playground-execution`.

## Goals / Non-Goals

**Goals:**

- Reuse the authoritative lexer, parser, semantic checker, IR lowering, and verifier for browser compilation.
- Execute verified IR without LLVM, a linker, native binaries, JavaScript source generation, or `eval`.
- Match the observable native semantics of the currently supported browser-compatible language subset.
- Capture program output and return structured compiler diagnostics and runtime failures.
- Bound CPU-like work, call depth, interpreted heap use, output, diagnostics, and source size.
- Produce deterministic, versioned, cacheable WASM and JavaScript/TypeScript artifacts for a Web Worker consumer.
- Keep browser execution local: source is neither persisted nor transmitted by the compiler package.
- Make semantic drift measurable through shared fixtures and native/interpreter differential tests.
- Preserve the native LLVM backend and native runtime behavior.

**Non-Goals:**

- Emitting WebAssembly from Zirk source or adding `wasm32` to Zirk's language target matrix.
- Running LLVM, a native linker, native object files, or the native runtime inside the browser.
- Supporting `extern`, raw pointers, native slices, process, shell, filesystem, environment, secrets, network, OS signals, or other host-authority features in the browser interpreter.
- Implementing future concurrency, asynchronous I/O, package resolution, or multi-file project execution ahead of their compiler phases.
- Providing a security boundary for hostile native code; no native code is produced or executed.
- Making the interpreter the performance or semantic authority for production Zirk applications.
- Enabling the playground Format action; formatting needs a separate capability.

## Decisions

### 1. Interpret verified typed IR

`zirk-interpreter` will consume only `zirk_ir::Module` values that pass `zirk_ir::verify`. The browser facade will run the existing source pipeline and never interpret parser or checker output directly.

```text
                         native path
                       +-------------> LLVM backend -> native executable
                       |
source -> frontend -> verified IR
                       |
                       +-------------> IR interpreter -> structured result
                                      browser path
```

This preserves one frontend and makes control flow, generic specialization, resolved dispatch metadata, source spans, and lowered safety checks reusable.

Alternatives considered:

- **AST interpreter:** rejected because it would duplicate name resolution, type-directed behavior, lowering, and control-flow semantics.
- **IR-to-JavaScript transpiler:** rejected because JavaScript numeric, identity, exception, and object semantics would require a second runtime and `eval` or generated module loading.
- **LLVM compiled to WASM:** rejected because of payload size, toolchain fragility, startup cost, and linker/runtime complexity.
- **Server-side native compilation:** deferred because it introduces operating cost and hostile-code isolation requirements.

### 2. Separate the execution engine from browser bindings

The workspace will gain three responsibilities:

- `zirk-runtime-core`: safe, platform-neutral semantic helpers and owned value operations shared by the native runtime and interpreter where behavior can be reused without exposing native handles.
- `zirk-interpreter`: execution state, values, frames, slots, heap identities, dispatch, failures, budgets, and host traits; it has no browser or LLVM dependency.
- `zirk-browser`: in-memory source pipeline, compatibility preflight, WASM exports, request/result conversion, package manifest, and generated TypeScript declarations.

The dependency direction is:

```text
zirk-runtime -----------> zirk-runtime-core <----------- zirk-interpreter
                                                         ^
lexer/parser/sema/ir/diagnostics ------------------------|---- zirk-browser

zirk-codegen-llvm   (no browser dependency)
```

Only logic that is demonstrably platform-neutral moves into `zirk-runtime-core`. Native allocation, C ABI symbols, collector mechanics, stdout locking, process lifecycle, and pointer operations stay in `zirk-runtime`.

Alternative: duplicate runtime helpers in the interpreter. Rejected because exact decimal, Unicode, regex, collection, and temporal behavior would drift.

### 3. Use an explicit interpreter value and heap model

The interpreter will use a closed Rust value representation covering each executable `IrType`, with typed scalar payloads and stable arena identifiers for identity-bearing values. Frames own instruction results and local slots. Heap entries represent strings, closures/environments, objects, records where indirect storage is required, enums, arrays, lists, maps, sets, and runtime errors.

Reference identity (`is`) compares stable interpreter identities; structural equality follows existing language semantics. The interpreter heap does not expose addresses. `Pointer`, `NativeSlice`, `NativeSliceMut`, `Dependent`, `Pin`, `JournalHandle`, and browser-incompatible extern values fail compatibility preflight before execution.

A tracked arena is preferred over mirroring the native mark-sweep collector. Entries are reclaimed when an execution ends; the heap budget prevents unbounded growth during an execution. This is an execution-engine implementation detail and does not alter Zirk's normative managed-memory semantics.

### 4. Execute basic blocks with explicit frames and signals

Each frame contains the current function, basic block, instruction cursor, slots, produced values, arguments, optional closure environment, and return destination. Instructions return an internal signal: continue, call, return, throw, or fatal failure. Terminators change blocks or finish frames. This avoids host-language recursion for ordinary Zirk calls; the configured Zirk call-depth limit remains explicit and testable.

Virtual and contract dispatch use the object and contract tables already present in the IR module. Generic code is interpreted after existing specialization. Runtime errors retain the originating instruction span and Zirk call frames.

### 5. Make host effects explicit and deny by default

`zirk-interpreter` receives a narrow host interface. The first browser host supports bounded text output and deterministic cancellation/budget checks only. It has no APIs for filesystem, network, environment, clocks, random data, process creation, shell execution, dynamic library loading, or native calls.

A compatibility preflight walks the verified module before execution and returns sorted capability diagnostics for unsupported types, instructions, extern declarations, or host effects. It must not begin execution when any incompatible operation is reachable in the module accepted for the browser package. Unsupported behavior is never stubbed with fabricated success.

### 6. Bound execution at both interpreter and worker layers

The request carries limits constrained by package-defined maxima. Defaults and maxima are included in the package manifest. At minimum the interpreter tracks:

- UTF-8 source bytes;
- emitted diagnostics;
- executed instructions;
- Zirk call depth;
- interpreter heap bytes or weighted allocation units;
- output UTF-8 bytes and output events;
- captured Zirk stack frames.

Limit exhaustion returns a typed `limit-exceeded` result naming the exhausted limit and source span when available. Output already produced is returned with a truncation marker where applicable.

The browser consumer additionally runs the package in a dedicated Web Worker and may terminate the worker after a wall-clock deadline. Wall-clock termination is a host containment measure, not a deterministic language result; the site recreates the worker afterward.

### 7. Expose one versioned, operation-oriented browser contract

The WASM package exports initialization plus `check` and `run` operations over UTF-8 source. Requests and results are JSON-compatible typed values with an `apiVersion`, compiler version, build revision, operation, limits, diagnostics, output events, completion state, and optional runtime failure.

`check` runs through semantic analysis and returns no runtime output. `run` runs the same frontend, lowers and verifies only when diagnostics contain no errors, performs compatibility preflight, and then interprets `main`.

The package must not expose Rust panics as expected user errors. User source failures become structured results; an unexpected internal failure becomes an `internal-error` result with no native path or secret data.

### 8. Produce reproducible static artifacts with provenance

A repository script builds `wasm32-unknown-unknown`, runs the selected binding generator, and emits a self-contained package directory containing `.wasm`, ES-module glue, TypeScript declarations, license notices, and a machine-readable manifest. The manifest records API version, Zirk version, source revision, tool versions, supported operations, default/max limits, artifact checksums, and browser compatibility exclusions.

The package is not fetched from an unpinned CDN. The site copies or vendors the exact generated artifact from a clean committed compiler revision and verifies checksums during its content/build pipeline. Firebase serves it as immutable hashed static content.

The implementation will pin mature dependency versions after compatibility and license review rather than selecting unbounded or newly published versions.

### 9. Define conformance by observable behavior

A shared fixture corpus will identify programs expected to be browser-compatible. Each fixture defines expected stdout events, completion/failure category, and relevant diagnostics. CI runs interpreter tests on the host, builds the WASM target, runs browser-package contract tests in a JavaScript runtime where possible, and compares browser-compatible fixtures against `zirk run`.

Differential comparison covers observable values and failures, not implementation-specific native stack addresses, allocator behavior, or diagnostic color escapes. Any deliberate browser exclusion is tested as a capability diagnostic.

### 10. Treat this as tooling scope, not a Zirk WASM target

Documentation will distinguish:

- Zirk source interpreted by compiler tooling compiled to WASM;
- Zirk source compiled to a native executable through LLVM; and
- a future Zirk-to-WebAssembly backend, which remains excluded from Zirk 1.x and deferred.

The implementation-status claim becomes available only after the interpreter, package, conformance suite, documentation, and companion-site integration are complete and reviewed.

## Risks / Trade-offs

- **[Semantic drift between native runtime and interpreter]** -> Extract platform-neutral runtime semantics, share fixtures, and require differential tests before claiming support.
- **[The current IR contains native-only operations]** -> Run explicit compatibility preflight and maintain an audited support matrix keyed to every `IrType`, `InstKind`, terminator, and intrinsic.
- **[Infinite loops or allocation attacks freeze the page]** -> Enforce deterministic interpreter budgets and execute only in a disposable worker with a host timeout.
- **[WASM payload harms site performance]** -> Keep artifacts in the lazy playground path, initialize on demand, publish measured compressed budgets, and avoid LLVM/runtime-native code.
- **[WASM linear memory cannot always be recovered after cancellation]** -> Terminate and recreate the worker after timeout, internal error, or high-water memory conditions.
- **[Pure helper extraction destabilizes the native runtime]** -> Move behavior incrementally with unchanged native ABI tests and native/interpreter parity fixtures.
- **[Browser support is mistaken for a language target]** -> Use precise documentation and manifest terminology; do not add `wasm32` to the Zirk target matrix.
- **[Future IR changes silently break the interpreter]** -> Use exhaustive matches where practical, support-matrix tests, and workspace CI that builds both engines.
- **[Regex or collection operations consume disproportionate resources inside one instruction]** -> Add operation-specific bounds or weighted instruction charges for non-constant-cost helpers.
- **[Unexpected Rust panic crosses the WASM boundary]** -> Validate IR, eliminate user-triggerable panics, test malformed requests, and convert boundary failures into sanitized internal errors.

## Migration Plan

1. Add platform-neutral runtime-core boundaries while preserving all native ABI and runtime tests.
2. Add the interpreter value model, execution loop, budgets, and instruction coverage incrementally behind crate-level tests.
3. Add compatibility preflight and differential fixtures; do not publish browser availability yet.
4. Add the in-memory browser facade and versioned result model.
5. Add reproducible WASM packaging and CI artifact verification.
6. Update compiler documentation and status language without describing WebAssembly as a Zirk target.
7. Commit the compiler change and produce a package from that exact clean revision.
8. Apply `add-browser-playground-execution` in `zirk-lang-site`, pin the package provenance, and validate browser execution, accessibility, bundle isolation, and Firebase deployment.
9. Run `./scripts/sync-website-content.sh --audit-date <reviewed-date>` after the compiler documentation commit, reconcile generated content with site-owned playground changes, and verify both repositories are consistent.

Rollback is additive: the site can disable local Run and Check and return to its truthful editor-only state without affecting native compilation. The new crates and package can remain unpublished until conformance is satisfactory; the LLVM path requires no migration.

## Open Questions

- Which currently implemented IR instructions should form the first public compatibility baseline if complete browser-safe coverage cannot land atomically?
- Should weighted helper costs be calibrated in abstract units only, or supplemented with per-operation input-size maxima for regex and collection methods?
- Should the generated browser package be committed to the site, copied during a release command, or published as a versioned package later? The initial design assumes a checked-in, checksum-verified artifact to keep Firebase deployment self-contained.
- Which browser/runtime matrix is mandatory for release testing beyond the site's existing supported-browser policy?
