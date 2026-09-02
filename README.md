# Zirk

Compiled, object-oriented programming language with static typing and inference. High-level by default, with optional access to low-level control. Compiles to native binaries via LLVM.

> **Easy by default, explicit when you need control.**

Structured concurrency (`task` / `await`) and multi-core parallelism (`parallel` / `thread`) are first-class features. Binaries are standalone: they require no Node.js, Python, Java, or any other installation.

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

---

## Status: Phase 4e in progress

**The example above compiles and runs.** `zirk run hello.zrk` produces a native binary and runs it. The full compiler pipeline is green and 995 tests pass.

Phase 4d — callable and binding completion — is **complete for its scoped delivery** (`fase-4d-callables` and `fase-4d-declaraciones-multiples` archived). The active development phase is now **4e — managed memory and unsafe boundaries**.

```
   .zrk ──▶ [lexer] ──▶ [parser] ──▶ [sema] ──▶ [ir] ──▶ [codegen] ──▶ binary
               ✅          ✅          ✅         ✅         ✅          ✅
```

The complete, feature-by-feature status is in [docs/init/ZIRK_FEATURE_STATUS.md](docs/init/ZIRK_FEATURE_STATUS.md). Use it before assuming a construct is available; this README no longer repeats that enumeration.

Implemented through Phase 4d: the full Phase 1 pipeline, Phase 2 control flow and closures, Phase 3 objects/generics/records/value classes/abstract dispatch/structural equality, Phase 3b scalar/text families, Phase 4a `Result<T,E>`, 4b exceptions, 4c single-resource `match ... with`, and 4d `Fn(...) => R` annotations, escaping closures, multiple declarations and simultaneous assignment.

```sh
zirk build hello.zrk    # compiles to a native executable
zirk run hello.zrk      # compiles and runs
```

Artifacts land in `build/`. Still in progress or not yet implemented: dependent references and automatic bounded native pinning (Phase 4e), concurrency (`task`/`await`/`parallel`/channels/atomics, Phase 5), the multi-file project system and `init.zrk` (Phase 6), the standard library collections and temporal family (Phase 7), generators and the pipe operator (Phase 7b), and packaging/decorators/tooling (Phases 8–10). See the feature catalog for the exact, per-pipeline-stage status of each one.

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
| `zirk-diagnostics` | error and warning formatting |
| `zirk-cli` | the `zirk` executable |
| `zirk-runtime` | runtime linked into produced binaries |

Dependencies flow in a single direction along the pipeline. `zirk-diagnostics` is the only cross-cutting one.

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

## Language

The specs, the roadmap, the code, the OpenSpec change artifacts, this README, `CONTRIBUTING.md`, every ADR, and `docs/TOOLCHAIN.md` are **in English**. Only commit messages stay in Spanish, by convention. See [ADR-006](docs/decisions/ADR-006-language-of-the-codebase.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The project uses git flow and moves forward by phases: features from future phases are not implemented even when they are already specified.

## License

Apache-2.0. See [LICENSE](LICENSE).
