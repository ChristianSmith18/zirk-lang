## Why

Zirk's public playground must eventually compile and execute user source without a paid or trusted server-side compiler service, while the current LLVM backend and native runtime cannot run inside a browser. The typed, target-independent Zirk IR already provides the right semantic boundary for a browser-safe interpreter that reuses the compiler frontend and preserves the native backend as the authoritative production target.

## What Changes

- Add a platform-neutral runtime-core crate so the native runtime and interpreter can share exact decimal, Unicode, regex, collection, scalar, range, and temporal semantics without sharing native ABI or allocator machinery.
- Add a Rust IR interpreter that executes verified Zirk IR independently of LLVM and exposes structured diagnostics, runtime output, termination status, and resource-limit failures.
- Add a browser compiler facade that accepts source text in memory, runs the existing lexer/parser/sema/lowering pipeline, verifies the IR, and invokes the interpreter without filesystem, process, network, environment, or native linker access.
- Add a `wasm32-unknown-unknown` package and stable JavaScript/TypeScript-facing contract suitable for execution inside a Web Worker.
- Capture `stdout.print` and `stdout.println` through a bounded host output sink instead of native standard output.
- Enforce source-size, instruction, call-depth, heap, output, and diagnostic limits so malformed or non-terminating programs cannot monopolize the browser.
- Report compiler-stage diagnostics separately from runtime failures and preserve source spans in both result forms.
- Add differential conformance fixtures that run supported programs through both the interpreter and native compiler and compare observable output and failures.
- Explicitly reject browser-incompatible language effects, including `extern`, raw native pointers, process, shell, filesystem, environment, secrets, and network access, with structured capability diagnostics rather than silently simulating them.
- Preserve the existing LLVM native compilation path unchanged; this change does not introduce WebAssembly as a Zirk language target and does not claim browser support for all future Zirk runtime facilities.
- Update architecture, toolchain, implementation-status, limitation, and roadmap documentation, then synchronize the companion `../zirk-lang-site` repository from the committed compiler revision.

## Capabilities

### New Capabilities

- `zirk-ir-interpreter`: Defines deterministic execution of verified typed IR, runtime values, calls, control flow, heap objects, failures, host output, and resource limits.
- `zirk-browser-compiler`: Defines the in-memory source-to-result facade, browser compatibility policy, structured result schema, WebAssembly packaging, and worker-oriented distribution contract.

### Modified Capabilities

- `compiler-workspace`: Adds interpreter and browser-binding crates while preserving one-way pipeline dependencies and LLVM isolation.
- `zirk-runtime-io`: Extends the observable standard-output contract to a bounded host sink when interpreted outside a native process.
- `zirk-feature-phasing`: Records browser-hosted interpreted execution as tooling work that does not make WebAssembly a Zirk 1.x compilation target.

## Impact

- **Compiler crates:** new `zirk-runtime-core`, `zirk-interpreter`, and `zirk-browser` crates; integration with `zirk-lexer`, `zirk-parser`, `zirk-sema`, `zirk-ir`, and `zirk-diagnostics`; no dependency from the interpreter to `zirk-codegen-llvm`.
- **Runtime semantics:** reusable pure helpers may be extracted from `zirk-runtime`, but its native C ABI, collector, and static library remain independent from compiler crates.
- **Build and dependencies:** adds a pinned, reviewed WASM binding/build toolchain and a reproducible browser artifact build; native workspace builds and frontend-only checks remain supported.
- **Public API:** introduces a versioned serialized request/result contract and generated JavaScript/TypeScript bindings consumed by `zirk-lang-site`.
- **Testing:** adds instruction-level interpreter tests, browser-target build checks, resource-limit tests, unsupported-capability tests, and native/interpreter differential fixtures.
- **Documentation/status:** affects compiler architecture, toolchain instructions, implementation status, current limitations, and roadmap wording. It does not change Zirk source syntax or normative language semantics.
- **Companion repository:** `../zirk-lang-site` must consume an exact committed browser package, activate local Check and Run in its existing playground, preserve SSR boundaries, and update its truthful availability messaging in the coordinated `add-browser-playground-execution` change.
