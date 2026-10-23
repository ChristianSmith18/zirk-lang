## Why

`std.io` currently establishes package and module wiring, but `stdin`,
`stdout`, and `stderr` are only skeletal declarations backed by the legacy
printing path. Native applications need a complete, typed stream API with
defined EOF, flushing, failures, encoding, and browser behavior before other
standard-library I/O packages can depend on it.

## What Changes

- Define the complete public `std.io::stdin`, `std.io::stdout`, and
  `std.io::stderr` module APIs as ordinary Zirk declarations, including text
  and byte-oriented operations, line reads, EOF outcomes, flushing, and
  explicit failure results.
- Define `ReadResult<T>` and I/O error types so an empty line, EOF, and a host
  failure are distinguishable; preserve the convenience `stdin.read_line()`
  API as a nullable line read with documented semantics.
- Define deterministic formatting for `stdout.print`/`println` and their
  `stderr` counterparts, including zero arguments, separators, Unicode, and
  flushing behavior.
- Route only the fully-qualified compiler-owned `std.io` declarations to
  native Rust runtime operations and interpreter host capabilities. No Zirk
  `native`, `intrinsic`, ABI, or host-binding syntax is introduced.
- Add native, interpreter, and browser capability behavior: browser execution
  retains output capture but rejects standard input reads before execution
  because it has no ambient input authority.
- Update the standard-library specification, handbook, examples, feature
  status, current limitations, and companion website synchronization.

## Capabilities

### New Capabilities

- `zirk-standard-streams`: Public behavior, result types, and portability
  contract for the `std.io` stdin/stdout/stderr modules.

### Modified Capabilities

- `zirk-standard-library`: Records `std.io` as a delivered, complete stream
  surface and synchronizes its API contract with the standard-library index.
- `zirk-runtime-io`: Extends the runtime boundary from legacy output-only
  intrinsics to fully-qualified standard stream operations and typed input
  outcomes.
- `zirk-browser-compiler`: Defines the browser-compatible output subset and
  deterministic rejection of unavailable standard input effects.

## Impact

- Affects `stdlib/std/io`, semantic standard-library registry resolution, IR
  lowering, native runtime I/O, the interpreter, and browser capability
  preflight.
- Adds public Zirk API and typed outcomes; stream modules are imported from
  `std.io` and are not implicit globals.
- Affects `docs/ZIRK_STDLIB_SPEC.md`, handbook standard-library pages,
  examples, status evidence, and the sibling `../zirk-lang-site` repository.
