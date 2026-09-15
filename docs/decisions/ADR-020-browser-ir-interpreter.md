# ADR-020: Browser-hosted IR interpretation is tooling, not a Zirk target

## Status

Accepted

## Context

The native Zirk pipeline lowers checked source to typed IR and then produces a
native executable through LLVM. The public playground needs authentic checking
and a deliberately small, local execution surface, but neither LLVM nor the
native runtime ABI belongs in a browser payload.

## Decision

The browser package compiles the authoritative Rust frontend and a verified-IR
interpreter to `wasm32-unknown-unknown`. Zirk source is **not** compiled to
WebAssembly, and `wasm32` is not a Zirk target.

The interpreter receives only IR accepted by `zirk_ir::verify`. Its initial
host grants exactly two effects: bounded `stdout.print` / `stdout.println`
output and cooperative cancellation/budget checks. It grants no input,
filesystem, storage, network, environment, process, shell, clock, random,
DOM, native-library, pointer, or scheduler authority.

Browser compatibility is an explicit policy, not an inference from successful
native compilation. The checked-in support matrix classifies every current IR
type, instruction, terminator, and `extern` declaration. A program requiring
an excluded capability receives a structured capability diagnostic before any
entrypoint instruction executes.

## Consequences

- The browser package depends on the frontend, `zirk-ir`, runtime-core and the
  interpreter, never on LLVM or `zirk-runtime`.
- Native and interpreted execution share conformance fixtures for the approved
  baseline; native execution remains the production authority.
- `parallel`, structured concurrency, timers, channels, asynchronous I/O and
  multi-file/package execution are excluded from the first browser baseline.
- Every published package is built from one clean compiler revision and pinned
  with a manifest and checksums by `zirk-lang-site`; compiler changes do not
  silently alter the deployed playground.

## Related

- [ADR-007](./ADR-007-forma-de-la-ir.md) — typed IR boundary.
- [ADR-019](./ADR-019-parallel-and-multithreaded-gc.md) — native-only worker
  pool and collector model.
- `openspec/changes/add-browser-ir-interpreter/` — implementation plan.
