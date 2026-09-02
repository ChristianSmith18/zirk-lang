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

## Status: Phase 1 complete

**The example above compiles and runs.** `zirk run hello.zrk` produces a native binary and runs it.

```
   .zrk ──▶ [lexer] ──▶ [parser] ──▶ [sema] ──▶ [ir] ──▶ [codegen] ──▶ binary
               ✅          ✅          ✅         ✅         ✅          ✅
```

Implemented subset: `fn`, `Void`, `Int32`, `Boolean`, `String`, `mut`/`inmut`, literals, arithmetic with overflow checking, comparison, logic, `if`/`else`, calls, `return`, and `stdout.println`.

```sh
zirk build hello.zrk    # compiles to a native executable
zirk run hello.zrk      # compiles and runs
```

Artifacts land in `build/`. Not yet implemented: generics, classes, `Result`, concurrency, decorators, multi-file modules, loops, `match`, or nullability. These arrive by phase, per the [roadmap](docs/init/ZIRK_ROADMAP.md).

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
