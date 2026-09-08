# ADR-004 — Portability strategy

- **Status:** accepted
- **Date:** August 12, 2026
- **Phase:** 0

## Context

The requirement is that Zirk work on any machine, not only the author's: Windows, Linux and macOS on the base architectures from the spec (`ZIRK_COMPILER_SPEC.md` section 6).

That requirement hides **two distinct portabilities** that don't cost the same and aren't solved the same way. Conflating them is the main trap.

```
PORTABILITY A — the compiler is BUILT on all 3 platforms
    who can compile zirkc from source?
    ← this is where llvm-sys hurts on Windows

PORTABILITY B — the compiler PRODUCES binaries for all 3 platforms
    which targets does Zirk emit for? (COMPILER_SPEC section 6)
    ← does NOT require the compiler to run on Windows
```

**B is the one that matters to the Zirk user. A only matters to whoever develops the compiler.**

## Finding that motivates the decision

It was empirically verified that **a single LLVM 20.1 installation on `aarch64-macos` emits valid native objects for all nine targets in the spec**:

| Zirk Target | Produced object |
|---|---|
| `aarch64-macos` | Mach-O 64-bit object arm64 |
| `x86_64-macos` | Mach-O 64-bit object x86_64 |
| `x86_64-linux` | ELF 64-bit LSB, x86-64 |
| `aarch64-linux` | ELF 64-bit LSB, ARM aarch64 |
| `armv7-linux` | ELF 32-bit LSB, ARM EABI5 |
| `x86-linux` | ELF 32-bit LSB, Intel 80386 |
| `x86_64-windows` | Intel amd64 COFF object file |
| `x86-windows` | Intel 80386 COFF object file |
| `aarch64-windows` | Aarch64 COFF object file |

In other words: **portability B is structurally solved from day one** by LLVM + lld, with no additional compiler work.

## Decision

1. **Portability B is a continuously verified requirement.** Emitting objects for the nine targets is covered by tests starting in Phase 1, not deferred to Phase 6.

2. **Portability A is solved via CI on the three platforms + prebuilt binaries.** The Zirk user never compiles the compiler: they download an already-built `zirkc`. Only whoever contributes to the compiler needs the full toolchain from [TOOLCHAIN.md](../TOOLCHAIN.md).

3. **`lld` is the default linker**, not whatever `cc` happens to be on the host. It provides `ld.lld` (ELF), `ld64.lld` (Mach-O) and `lld-link` (COFF) from the same binary and the same version, which makes linking reproducible across platforms. Depending on each host's `cc` reintroduces, through the back door, exactly the variability this ADR seeks to eliminate.

## Open risk: sysroots for cross-linking

Emitting the object is solved; **linking** an executable for another platform additionally requires the destination sysroot (libc and system libraries). This is not solved and is real Phase 6 work.

It does not block Phase 1, which only compiles for the host. It is recorded here so it isn't discovered late.

## Consequences

- Verification must cover `{windows, linux, macos}` starting in Phase 0, split between CI and the development machine (see "Verification split").
- No development can depend on a machine-specific absolute path. In particular, `LLVM_SYS_201_PREFIX` is resolved from the environment and is **not** versioned in `.cargo/config.toml`.
- Windows is the highest-friction platform for portability A; see [ADR-001](./ADR-001-pin-llvm.md) for the LLVM source that actually works there.

## Verification split

The four platforms are verified in CI:

```
   Linux x86_64   ──┐
   Linux aarch64  ──┼─▶  GitHub Actions
   macOS aarch64  ──┤
   Windows x86_64 ──┘
```

macOS was outside the matrix while the repository was private, because its runners consume minutes at a 10x rate and `macos-13` (Intel) rarely managed to get a runner. Once the repository went public, Actions became free and unlimited and that restriction disappeared.

`./scripts/check-local.sh` runs the same thing as CI and remains the way to verify before opening a PR, but it is no longer the only macOS coverage.

## Verification status

| Portability | Status | Evidence |
|---|---|---|
| **B** — emission for the 9 targets | ✅ verified | `target_matrix` test: emits and validates container and architecture for the nine targets, on the four platforms in the matrix. |
| **A** — Linux x86_64 | ✅ verified | CI green. |
| **A** — Linux aarch64 | ✅ verified | CI green. |
| **A** — macOS aarch64 | ✅ verified | CI green (returned to the matrix once the repository went public). |
| **A** — Windows x86_64 | ✅ verified | CI green. Required changing the LLVM source; see [ADR-001](./ADR-001-pin-llvm.md) and issue #2. |
| **A** — macOS x86_64 | 🚫 out of scope | Intel is a retiring platform and its runners are scarce. |
| **A** — Windows aarch64 | 🚫 outside the initial matrix | Added when there is real demand. |

**Portability is now verified.** It was the most expensive point of Phase 0 and the one that justified building the foundations before the language: Windows required ten iterations and uncovered three real defects in the code itself, which Linux and macOS tolerated by accident.

## Addendum (September 8, 2026) — task context-switch shim

[ADR-017](./ADR-017-modelo-de-suspension.md) chose stackful coroutines for
`task` / `await`. Suspending a task is a **context switch**: save the running
task's callee-saved registers, stack pointer, and resume address, then restore
another context. This is inherently per-architecture and per-calling-convention
code, and it adds a **third** portability concern to the two this ADR already
separates:

```
PORTABILITY A — the compiler is BUILT on all platforms
PORTABILITY B — the compiler PRODUCES binaries for all target triples
PORTABILITY C — the runtime SWITCHES TASK CONTEXTS on every target triple   ← new
```

Portability C is bounded and does not reopen A or B:

1. The shim is isolated behind a single seam,
   `fn switch(from: *mut Context, to: *const Context)` in
   `crates/zirk-runtime/src/context.rs`, plus
   `fn make_context(stack, size, entry, arg) -> Context`.
2. It must exist for the four triples this ADR verifies in CI: **macOS aarch64,
   Linux x86_64, Linux aarch64, Windows x86_64**. The 32-bit and secondary
   targets in the portability-B table are emission targets, not runtime hosts for
   the executor, and are out of scope until there is demand.
3. A **host-only Rust fallback** covers `cargo test` on any host that is not one
   of the four, so the runtime crate's own tests never depend on the asm path.
4. Whether each implementation is hand-written assembly (`global_asm!` or a
   build-script object, ~40 lines per arch/os pair) or a vetted `no_std` crate
   (`corosensei`, which already abstracts exactly these four targets) is decided
   in the `fase-5-async-core` change's `design.md`. `ADR-002`
   (self-contained staticlib) leans toward hand-written; the seam is identical
   either way.

### Verification split for portability C

The CI matrix gains one test before anything else in Phase 5 step 1 builds on
top of it: **executor↔task ping-pong for N round trips on each matrix target**,
asserting every callee-saved register and the stack pointer round-trip exactly.

| Triple | Executor context switch | Evidence |
|---|---|---|
| Linux x86_64 | ⏳ pending `fase-5-async-core` | ping-pong test in CI |
| Linux aarch64 | ⏳ pending `fase-5-async-core` | ping-pong test in CI |
| macOS aarch64 | ⏳ pending `fase-5-async-core` | ping-pong test in CI |
| Windows x86_64 | ⏳ pending `fase-5-async-core` | ping-pong test in CI |

No `task` / `await` codegen is merged until every row above is green, the same
discipline portability B followed from Phase 1.
