# Zirk

Compiled, object-oriented programming language with static typing and inference. High-level by default, with optional access to low-level control. Compiles to native binaries via LLVM.

> **Easy by default, explicit when you need control.**

Zirk's structured concurrency surface uses `concurrent` blocks, `spawn`,
`Job<T>`, `Timer`, and `parallel` for real multi-core CPU regions, plus typed
channels for cooperative communication. Explicit thread and synchronization
APIs remain planned; scalar atomics currently exist only as an internal
runtime/ABI foundation, not as Zirk source types. See the
[concurrency implementation status](docs/init/ZIRK_FEATURE_STATUS.md#phase-5-concurrency-and-parallelism)
and [runtime limits](docs/ZIRK_RUNTIME_SPEC.md) for exact guarantees.
Binaries are standalone:
they require no Node.js, Python, Java, or any other installation.

```zirk
import { stdout } from std.io;

stdout.println("Hello from Zirk");
```

---

## Current project model

Zirk is under active implementation. The current `.zkinit` model describes
project metadata, an application `entry` or package `root`, source roots,
dependencies, development dependencies, globals, build targets, and one finite
`requires` list. Run `zirk init` to create a manifest interactively, or
`zirk init -y` to accept its defaults. `zirk run` selects the configured entry;
the selected file executes its top-level statements, so a source `fn main` is
not required. Runtime consent is requested for exact resources when protected
operations are reached. Filesystem and process APIs are not yet exposed by the
compiler. See [ZIRK_FEATURE_STATUS.md](docs/init/ZIRK_FEATURE_STATUS.md) for
implementation coverage and validation evidence.

```
   .zrk ──▶ [lexer] ──▶ [parser] ──▶ [sema] ──▶ [ir] ──▶ [codegen] ──▶ native binary
               ✅          ✅          ✅         ✅         ✅
                                                    └──▶ [interpreter] ──▶ browser WASM package
                                                         optional; no Zirk-to-WASM code generation
```

The complete, feature-by-feature status is in [docs/init/ZIRK_FEATURE_STATUS.md](docs/init/ZIRK_FEATURE_STATUS.md). Use it before assuming a construct is available; this README no longer repeats that enumeration.

```sh
zirk init               # create .zkinit interactively
zirk init -y            # create .zkinit with defaults
zirk run                # run the configured project entry
zirk run src/other.zrk  # run one explicit source file
```

Artifacts land in `build/`. The delivered project slice includes quoted local
imports, package roots declared in `.zkinit`, `share mod` barrel exports,
unquoted package imports and lockfile validation for local/vendored
dependencies. Registry/Git acquisition, reproducible `.zpkg` publication and
the rest of the standard library remain pending. See the feature catalog for
the exact, per-pipeline-stage status of each one.

## Architecture

A Cargo workspace with one crate per pipeline stage of `ZIRK_COMPILER_SPEC.md` section 2:

| Crate | Responsibility |
|---|---|
| `zirk-lexer` | source text → tokens |
| `zirk-parser` | tokens → syntax tree |
| `zirk-ast` | shape of the syntax tree |
| `zirk-sema` | name resolution, types, flow analysis |
| `zirk-ir` | typed, portable intermediate representation |
| `zirk-codegen-llvm` | IR → LLVM → object file |
| `zirk-runtime-core` | pure, platform-neutral runtime value semantics |
| `zirk-interpreter` | verified IR execution with explicit bounded host effects |
| `zirk-browser` | in-memory frontend and optional WASM browser contract |
| `zirk-diagnostics` | error and warning formatting |
| `zirk-cli` | the `zirk` executable |
| `zirk-runtime` | runtime linked into produced binaries |

Dependencies flow in a single direction along the pipeline. `zirk-diagnostics` is the only cross-cutting one.
The browser branch uses the interpreter and `zirk-runtime-core`; it does not
depend on LLVM, the native ABI runtime, or browser ambient authority. It is an
optional execution engine for compatible source, not a native-target replacement.

## Getting started

Requires Rust 1.94+ and **LLVM 20.1** with static libraries. Per-platform installation is in **[docs/TOOLCHAIN.md](docs/TOOLCHAIN.md)** — read it before building, especially on Windows, where the official LLVM `.exe` installer **does not work**.

```sh
export LLVM_SYS_201_PREFIX=$(brew --prefix llvm@20)   # macOS
cargo test --workspace
```

## Documentation

**Normative specification** — describes mature Zirk, not what exists today:

- [ZIRK_SPEC_FINAL.md](docs/ZIRK_SPEC_FINAL.md) — scope, exclusions, philosophy
- [ZIRK_LANGUAGE_SPEC.md](docs/ZIRK_LANGUAGE_SPEC.md) — syntax, types, objects, errors
- [DECORATOR_SEMANTICS.md](docs/DECORATOR_SEMANTICS.md) — decorators, validated expansion, and static generation
- [ZIRK_COMPILER_SPEC.md](docs/ZIRK_COMPILER_SPEC.md) — pipeline, IR, LLVM, targets, CLI
- [ZIRK_RUNTIME_SPEC.md](docs/ZIRK_RUNTIME_SPEC.md) — memory, tasks, scheduler, resources
- [ZIRK_STDLIB_SPEC.md](docs/ZIRK_STDLIB_SPEC.md) — standard library

**Build process:**

- [ZIRK_ROADMAP.md](docs/init/ZIRK_ROADMAP.md) — the 13 phases, from here to self-hosting
- [docs/decisions/](docs/decisions/) — ADRs: decisions that cascade to the rest of the project
- [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md) — toolchain installation
- [browser-package-handoff.md](docs/browser-package-handoff.md) — verified browser package handoff

## Language

The specs, the roadmap, the code, the OpenSpec change artifacts, this README, `CONTRIBUTING.md`, every ADR, and `docs/TOOLCHAIN.md` are **in English**. Only commit messages stay in Spanish, by convention. See [ADR-006](docs/decisions/ADR-006-language-of-the-codebase.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The project uses git flow and moves forward by phases: features from future phases are not implemented even when they are already specified.

## License

Apache-2.0. See [LICENSE](LICENSE).
