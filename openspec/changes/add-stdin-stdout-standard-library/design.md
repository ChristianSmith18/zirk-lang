## Context

`std.io` already has package-root source files and a compiler-owned registry,
but the declarations are placeholders and output lowering still reflects the
legacy print surface. Native code uses stable Rust runtime output symbols;
the interpreter owns bounded injected output; the browser deliberately has no
ambient input capability. See proposal.md and the standard-streams capability
for the public contract.

## Goals / Non-Goals

**Goals:**

- Make `stdin`, `stdout`, and `stderr` complete source-defined standard
  modules whose public declarations are checked like any other package API.
- Keep host bindings private to compiler and runtime code, keyed by package,
  module, declaration, and signature.
- Give native and interpreter targets equivalent observable stream semantics,
  while making browser exclusions deterministic.
- Establish reusable stream outcomes and error representations for later
  files, sockets, processes, and terminal APIs.

**Non-Goals:**

- Terminal styling, cursor control, prompts, raw-mode input, and progress
  regions belong to a later `std.terminal` change.
- Asynchronous stream operations, redirection configuration, and arbitrary
  encodings are not part of this change.
- Browser stdin emulation is explicitly out of scope.

## Decisions

### D1: Treat `std.io` files as the API source of truth

`stdlib/std/io/package.zrk` will re-export dedicated stdin, stdout, and stderr
files. Their `share` declarations define the surface observed by parser,
resolver, type checker, documentation tooling, and package API metadata.
Compiler registry validation compares those declarations with the closed list
of host mappings.

Alternatives considered: defining stream methods only in Rust would duplicate
types and hide public signatures from Zirk tooling; exposing a `native` marker
would leak host implementation syntax into the language.

### D2: Use `ReadResult<T>` for streaming absence and `Result<T, IoError>` for contracts

Incremental reads need to distinguish data, EOF, and a failure, so they return
`ReadResult<T>`. APIs that promise an exact quantity use `Result` and make
premature EOF an error. `read_line(): String?` remains a small convenience
projection from `ReadResult<String>` for existing source compatibility.

Alternatives considered: nullable reads alone lose error information; treating
EOF as an exception makes normal interactive and piped input cumbersome.

### D3: Model output as three operations with one formatting path

`print` and `println` share argument rendering and separator logic; `flush`
is explicit and fallible. Native output uses stable runtime symbols, while the
interpreter writes target-labelled events to its bounded sink. `stderr` uses
the same formatting contract but a distinct destination label.

Alternatives considered: a configurable separator/end parameter increases API
surface and diverges from Zirk's interpolation-first style; implicit flushing
after every call harms throughput and obscures failures.

### D4: Lower standard operations only after semantic provenance is verified

Semantic resolution records that a call came from the compiler-owned standard
package. IR lowering consumes this record and the registry key rather than a
short function name. External package functions with matching text lower as
ordinary calls.

Alternatives considered: recognizing names such as `println` at parsing or IR
time would incorrectly grant hidden runtime behavior to user packages.

### D5: Browser capability preflight rejects reachable stdin reads

The browser compiler keeps stdout/stderr as in-memory output events. It marks
stdin reads as an excluded effect in the portable module metadata and rejects
them before the interpreter starts, preserving the no-ambient-authority
boundary.

Alternatives considered: using `prompt`, DOM events, or a synthetic empty
stream would make browser behavior host-dependent and silently change program
semantics.

## Risks / Trade-offs

- [Blocking native input can stall a process thread] → keep this change
  synchronous and document the boundary; design later async stream APIs around
  the task reactor rather than retrofitting a blocking implementation.
- [Generated source declarations and registry can drift] → add a test that
  checks every compiler-owned registry entry against its parsed standard source
  signature.
- [Output differs by platform line ending] → assert logical line output in
  cross-target tests and retain target-specific native newline tests.
- [Input can be unbounded] → enforce configured maximum byte and line bounds
  before allocating a returned buffer.

## Migration Plan

1. Land types and source declarations while preserving existing `stdout.print`
   and `stdout.println` behavior.
2. Add semantic provenance and IR/runtime/interpreter operations behind the
   source API.
3. Add browser preflight and cross-target conformance fixtures.
4. Update normative documentation and status only after executable evidence;
   commit Zirk changes, synchronize `../zirk-lang-site`, review its generated
   diff, and commit it separately.

Rollback consists of retaining the prior output declarations and registry
entries while withholding new input operations; no package import syntax or
user data migration is required.
