# Zirk — Compiler and tooling specification

## 1. Goals

The compiler must prioritize safe diagnostics, incremental compilation,
efficient native binaries and reproducible results. Formatter, linter, `check`
and LSP must operate on the incremental frontend without invoking LLVM.

Performance goals such as "milliseconds for roughly 100 files" are measured
targets, not guarantees independent of hardware. The project will maintain
public benchmarks for startup, parsing, formatting, incremental analysis, full
build, no-change build, LSP, peak memory and binary size.

## 2. Pipeline

```text
source .zrk
    ↓
incremental lexer
    ↓
parser producing a syntax tree
    ↓
module and name resolution
    ↓
type checker and flow analysis
    ↓
validated decorator expansion
    (Inspect → Augment → Wrap → revalidation)
    ↓
typed, portable IR
    ↓
LLVM IR
    ↓
object, linker and Mach-O / ELF / PE binary
```

The frontend shares persistent structures, symbol interning, content-addressed
caches and a fine-grained dependency graph. Only affected files and symbols are
invalidated.

## 3. Trees and Syntax API

The internal semantic AST is private and may evolve with the compiler. Tools,
IDEs and decorators use a **public, versioned, immutable and validated Syntax
API**.

The public API allows:

- inspecting tokens, nodes, types, signatures, attributes and locations;
- walking public declarations and authorized metadata;
- building transformations through typed builders;
- emitting diagnostics attached to source spans;
- generating explicit typed descriptors or registries when runtime structure is required.

It does not allow mutating internal memory, fabricating invalid nodes, bypassing
the type checker, or reaching the filesystem, network or processes without
an application `permissions` grant whose operation is marked `during: build`.

Transformations are re-parsed, re-resolved, re-typed and re-validated. The
compiler preserves traceability between original and generated code for
diagnostics and debugging.

Decorator target blocks are limited to class, attribute, function, method and
parameter. The compiler completes inspection, commits compatible augmentation,
and then wraps executable bodies. Every compiler service is an explicit variant
payload. Hygienic private identities prevent capture; public generated-name
collisions fail and identify all origins.

Decorator expressions preserve source order and compose with the nearest
application innermost. `requires`, `before`, and `after` validate that visible
order without rewriting it. Self-edges and graph cycles fail with the complete
cycle. Contiguous repeatable applications form one typed group with explicit
`applications` payloads.

Explicitly generated decorator applications enter bounded later rounds;
structural cycle detection rejects recursion. Expansion fingerprints cover the
decorator implementation/version, arguments, typed target, configuration,
permission grants, observed build inputs and transitive dependencies. Changed
fingerprints invalidate affected expansions only.

Decorator applications are erased. Runtime descriptors and framework
registries are ordinary generated typed API, participate in `public.api`,
documentation and compatibility analysis, and remain eligible for dead-code
elimination.

Permission effects are inferred through declarations and higher-order callable
metadata. Before executing compile-time or runtime code, the CLI compares
signed project-location, manifest, lockfile, permission, requester-subgraph and
approval fingerprints. Exact matches take the no-change fast path; only changed
graph segments are recomputed. A moved or renamed project, widened grant, new
or updated requester, changed integrity/path/phase, or invalid approval stops
before code execution and requires explicit consent.

## 4. IR and packages

The IR is typed, target-independent and versioned. It preserves enough
information for generic specialization, devirtualization, escape analysis,
safety checks, vectorization and debug info generation.

The frontend/IR also preserve reference provenance and dependent lifetime,
unsafe transaction boundaries, reversible write/effect classification,
irreversible commit points, structured task scope, cancellation edges,
selection guards, and compiler-derived transfer/share facts. Optimization may
remove journals or checks only after proving the same observable safety.

The exact base-ten `Float` type (surface name `Float`) is a dedicated IR type
whose literals travel as verbatim text and whose operations lower to
`zirk_rt_decimal_*` runtime calls, with the same zero-divisor guard integer and
`Duration` division carry. The internal Rust identifiers keep the name `Float`
for the IEEE 754 binary family whose surface name is `Float*` — a
behaviour-neutral rename of those identifiers is deferred to its own pass.

A `.zpkg` conceptually contains:

```text
package.zpkg
├── manifest
├── public.api
├── portable.ir
├── README.md
├── LICENSE
└── documentation
```

`public.api` exposes only types, signatures, traits, interfaces, decorator
contracts and documentation. The portable implementation is transformed to the
target during the application build. Core, runtime, stdlib, dependencies and
application code must all align to the same target and ABI.

## 5. Backend

LLVM is the only initial backend. An internal interface allows adding another
backend in the future without changing public semantics, but several will not be
maintained initially.

- Debug: minimal optimization, complete symbols and a clear correspondence with
  source.
- Release: high optimization, dead-code elimination, LTO where appropriate,
  vectorization and size/memory optimization without altering guarantees.

Inline textual assembly is not implemented. Zirk offers portable intrinsics and
SIMD vectors; the backend selects instructions per architecture and emits a safe
fallback when no equivalent instruction exists.

## 6. Targets and cross-compilation

Suggested canonical format:

```text
x86-windows
x86_64-windows
x86-linux
x86_64-linux
armv7-linux
aarch64-linux
aarch64-macos
x86_64-macos
```

Validity depends on the operating system, LLVM, the linker, the SDK and native
dependencies. Modern 32-bit macOS is not promised.

Resolution:

1. `zirk build --target ...`;
2. every `build_targets` entry in `.zkinit`;
3. host detection.

A CLI target temporarily replaces `build_targets`. The compiler checks early for
incompatible native libraries and explains which dependency blocks the target.

WebAssembly, browser, DOM and platform directives are outside Zirk 1.x.

## 7. Incremental and reproducible builds

Cache keys include content, compiler version, relevant flags, target, dependency
API, IR version and configuration. The system reuses parsing, types, IR, objects
and packages that were not invalidated.

A build locked by `zirk.lock` must use exactly the recorded versions and hashes.
`zirk update` recomputes resolution. Release artifacts intended for distribution
must be reproducible from the same source, lockfile, toolchain and target.

## 8. Diagnostics

Minimum format:

```text
error[E1234]: precise description
  src/users.zrk:18:12
   |
18 |     problematic expression
   |            ^ localized explanation
   |
   = cause: semantic reason
   = help: concrete action
```

Every diagnostic must include a severity, a stable code, a location, a cause and
help where a clear fix exists. Decorator errors show the generated declaration,
decorator declaration/application, target, expansion path and actionable source
span. Missing dependencies, invalid order, cycles and public conflicts name all
relevant origins. Decorators cannot suppress compiler diagnostics.

Safety diagnostics identify the owner and escape path for an invalid dependent
reference, the exact unsafe operation lacking a boundary, an irreversible
effect lacking `commit`, and any suspension/publication inside a reversible
transaction. Concurrency diagnostics identify both conflicting accesses and
suggest exclusive transfer, `inmut::strict`, synchronization, a channel, or
`clone()` as applicable.

Warnings do not change semantics. Configurable categories include unreachable
code, unused symbol, confusing shadowing, redundant cast, unnecessary permission
and ignored operation result. `--warnings-as-errors` may promote them.

## 9. Official CLI

Minimum commands:

```text
zirk new <name>       create a new project
zirk init             initialize Zirk in an existing directory
zirk run [source]     compile incrementally and run
zirk build [source]   produce artifacts
zirk check            analyze without generating code
zirk test             run tests
zirk bench            run benchmarks
zirk format           apply canonical formatting
zirk lint             run static rules
zirk prepare          audit permissions, targets and publishing
zirk add/remove       modify dependencies
zirk install          resolve dependencies
zirk update           update zirk.lock explicitly
zirk package          create a .zpkg
zirk publish          publish a package
zirk doc              generate documentation
```

The CLI must start fast, produce deterministic output and offer a structured
mode (`--json`) for tooling.

When `run` or `build` receives a Zirk source operand, the `.zrk` extension is
optional. For example, `zirk run hello` and `zirk run hello.zrk` resolve the
same source file, as do `zirk build hello` and `zirk build hello.zrk`. This
shorthand changes command-line resolution only; the physical source file still
uses the `.zrk` extension.

## 10. Formatter, linter and LSP

The formatter is canonical, idempotent and free of configuration that would
fragment style. It adds `;`, and normalizes spacing, braces, line breaks and
ordering where semantically neutral.

The linter shares parser, resolution and types with the compiler. Fast rules run
by default; expensive analyses are explicit. Automatic fixes must be safe and
reviewable.

The LSP uses incremental snapshots, cancellation of stale requests and
interactive priorities. It offers diagnostics, completion, hover, navigation,
references, rename, formatting, semantic tokens, signatures and code actions.

## 11. Debugger

The toolchain generates symbols and a faithful mapping to `.zrk`, including
async/tasks and decorated code. It must support breakpoints, stepping, variable
inspection, stack traces, threads, tasks and channels. Release optimizations may
limit observability and must say so.

## 12. Compiler tests

Suites are required for lexer/parser, diagnostic snapshots, type checker,
safety, IR, codegen per target, ABI, incrementality, reproducibility, formatter
idempotence, fuzzing and differential debug/release tests where applicable.

Every rule of the language must have at least one valid and one invalid case.
Required safety suites cover clone graph topology, weak-reference upgrade,
dependent escapes, transactional rollback, commit effects, task sibling
failure, cancellation cleanup, settled aggregation, fair select, channel
closure/backpressure, transfer/share derivation, mutex-across-await, atomic
ordering, and data-race rejection.
