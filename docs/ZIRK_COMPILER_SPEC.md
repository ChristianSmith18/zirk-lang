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
- requesting explicit reflection.

It does not allow mutating internal memory, fabricating invalid nodes, bypassing
the type checker, or reaching the filesystem, network or processes without
an application `permissions` grant whose operation is marked `during: build`.

Transformations are re-parsed, re-resolved, re-typed and re-validated. The
compiler preserves traceability between original and generated code for
diagnostics and debugging.

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
2. every `build_targets` entry in `init.zrk`;
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
help where a clear fix exists. Errors produced by decorators show both the
original source and the relevant expansion.

Warnings do not change semantics. Configurable categories include unreachable
code, unused symbol, confusing shadowing, redundant cast, unnecessary permission
and ignored operation result. `--warnings-as-errors` may promote them.

## 9. Official CLI

Minimum commands:

```text
zirk new <name>       create a new project
zirk init             initialize Zirk in an existing directory
zirk run              compile incrementally and run
zirk build            produce artifacts
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
