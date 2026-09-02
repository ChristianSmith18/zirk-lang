## Context

Zirk is specified in five normative documents that describe the **mature** language. The roadmap translates that into phases; Phase 0 is the only one that does not produce language, but rather the foundation on which everything else is built.

The underlying decisions have already been made and live in `docs/decisions/` as ADRs. This document **does not repeat them**: the ADRs are the durable source (they survive the archiving of this change) and are only referenced here.

| Decision | ADR |
|---|---|
| LLVM 20.1 pin + `llvm20-1` feature | [ADR-001](../../../docs/decisions/ADR-001-pin-llvm.md) |
| `zirk-runtime` staticlib with C ABI boundary | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| Memory: constraints now, implementation in Phase 4 | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| Portability: build vs. produce | [ADR-004](../../../docs/decisions/ADR-004-portabilidad.md) |
| Opaque `String` behind the runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |

State verified at the time of writing: LLVM 20.1.8 and lld 20.1.8 installed, `inkwell` 0.10 producing an executable `Mach-O arm64` binary, and object emission confirmed for the nine targets in the spec from an `aarch64-macos` host.

## Goals / Non-Goals

**Goals:**

- A `cargo build` that compiles the nine crates on all three platforms.
- A test that demonstrates the full chain: inkwell → LLVM IR → object → link → executing binary.
- A test that demonstrates object emission for the nine targets in the spec.
- CI that runs both over `{windows, linux, macos} × {x86_64, aarch64}`.
- `zirk-diagnostics` emitting the format from `COMPILER_SPEC` §8.

**Non-Goals:**

- **Any Zirk syntax.** No `.zrk` file is parsed. The lexer, parser, ast, sema, and ir crates exist with their minimal API and no implementation.
- Cross-**linking** with per-target sysroots (Phase 6). Only object emission is covered.
- `zirk run`, `zirk build`, and any real CLI subcommand (Phase 1 and Phase 6).
- The implementation of the memory strategy (Phase 4).

## Decisions

### D1 — Nine crates from the start, even though six remain nearly empty

The roadmap proposes eight crates; ADR-002 adds `zirk-runtime`.

```
zirk-cli ──▶ zirk-codegen-llvm ──▶ zirk-ir ──▶ zirk-sema ──▶ zirk-ast
    │                                                            ▲
    └──────────────▶ zirk-diagnostics ◀────────────────── zirk-parser ──▶ zirk-lexer

zirk-runtime  (depends on none: it compiles to a staticlib and is linked
               into the binaries Zirk produces, not into the compiler)
```

**Discarded alternative:** starting with a monolithic crate and splitting later. The cost of an empty crate in a Cargo workspace is practically zero, whereas splitting a monolith once the five stages share types is an expensive refactor. The roadmap already resolved this in Phase 0 and it is not up for relitigation.

Structural rule: **dependencies between crates flow in a single direction along the pipeline.** `zirk-diagnostics` is the only permitted cross-cutting dependency.

### D2 — The LLVM sanity check lives in the repo as a test, not as a spike

The roadmap poses it as a disposable pre-check. The decision is to incorporate it as a permanent test of `zirk-codegen-llvm`.

**Reason:** its value is not exhausted after passing once. It is the test that detects that someone has the wrong LLVM version, that a CI runner lost `LLVM_SYS_201_PREFIX`, or that an `inkwell` upgrade broke emission. As a disposable spike, that value is lost the day it is deleted.

### D3 — The LLVM pin is verified, not trusted

`llvm-sys` fails in confusing ways when the wrong major version is present. The build script must check the LLVM version and fail with a message pointing to `docs/TOOLCHAIN.md`, instead of letting the error surface as an unreadable link failure.

This is consistent with the project's own philosophy: `COMPILER_SPEC` §8 requires diagnostics with cause and help. Applying that to the compiler's own build is consistency, not excessive zeal.

### D4 — `LLVM_SYS_201_PREFIX` per environment, never versioned

It does not go into `.cargo/config.toml` because that path is specific to each machine and platform. Versioning it would break exactly the portability requirement from ADR-004. It is documented in `docs/TOOLCHAIN.md` and CI defines it per platform.

### D5 — The CI matrix is the operational definition of "portable"

Portability claimed in a document is not portability. The minimum matrix:

| OS | Architectures | LLVM 20.1 source |
|---|---|---|
| Linux | x86_64, aarch64 | `apt.llvm.org` → `llvm-20-dev` |
| macOS | x86_64, aarch64 | `brew install llvm@20` |
| Windows | x86_64 | `clang+llvm-20.1.8-*-pc-windows-msvc.tar.xz` tarball |

**Windows aarch64 remains a goal, not a blocker** for this change: the official tarball exists, but runner availability is less stable and should not hold back the rest.

Downloading LLVM in CI must be cached; without a cache, every job pays hundreds of MB and the matrix becomes impractical.

## Risks / Trade-offs

- **Windows fails to build `llvm-sys`** → This is the main risk and that is why it is part of this change, not a later one. Mitigation: use the official development tarball (not the `.exe`), which does include the static libraries, and VS 2022 Build Tools. If it still fails, the finding requires revisiting ADR-001 before proceeding to Phase 1.

- **Six empty crates look like bureaucracy** and tempt merging them → Mitigation: each crate starts with its responsibility documented in its `lib.rs`, so the boundary is explicit before there is code to enforce it.

- **The sanity check passes on macOS and gives false confidence** → Mitigation: until CI is green on all three platforms, portability is considered unverified. That is the exit criterion for this change.

- **The CI matrix makes the cycle slow** → Mitigation: cache the LLVM install and Cargo dependencies. If it is still an issue, the full matrix can be left to run on `main` while PRs run only the host platform, but never the other way around.

- **The LLVM 20.1 pin ages** → Trade-off consciously accepted in ADR-001: availability over novelty. Bumping the major version is a deliberate change with its own ADR.

## Migration Plan

Not applicable: there is no prior state to migrate. The repo has no code.

Rollback: the change is purely additive; reverting means deleting the workspace and the CI files.

## Open Questions

- **Windows aarch64 runners?** Their availability determines whether `aarch64-windows` enters the matrix now or is deferred. Resolved when CI is set up.
- **Does `rust-toolchain.toml` pin an exact version or a minimum one?** Pinning exact gives reproducibility and matches the spirit of `COMPILER_SPEC` §7; pinning minimum reduces friction for contributors. Leaning: exact, consistent with the discipline of the LLVM pin.
- **What does `zirk-runtime` expose at this stage?** With Phase 1 out of scope, it might expose nothing. Alternative: already define empty `zirk_rt_init` and `zirk_rt_shutdown` to fix the shape of the lifecycle from `RUNTIME_SPEC` §2. Leaning: define them, it is cheap and it is exactly the hook that ADR-002 justifies.
</content>
