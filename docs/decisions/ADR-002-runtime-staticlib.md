# ADR-002 — `zirk-runtime` as a Rust staticlib with a C ABI boundary

- **Status:** accepted
- **Date:** August 12, 2026
- **Phase:** 0

## Context

`ZIRK_ROADMAP.md` Phase 1 requires that `stdout.println("...")` reach a real syscall, but does not define **where that code lives**. The roadmap also does not include a runtime crate in the proposed workspace layout.

Options considered:

| Option | For | Against |
|---|---|---|
| **(a)** codegen emits libc `call @puts` / `@printf` | shortest path to the first binary | debt that must be entirely dismantled later; there is nowhere for the application lifecycle to live |
| **(b)** `zirk-runtime` crate in Rust compiled to a staticlib | fixes the C ABI boundary from the start; is the place where scheduler, GC and channels will later live | more work in Phase 1 |
| **(c)** runtime in C | trivial ABI | loses everything that motivated choosing Rust for the compiler |

## Decision

**(b)** is adopted: a `zirk-runtime` crate compiled to `staticlib` (`.a` / `.lib`), linked into every binary Zirk produces. Codegen emits calls to stable `extern "C"` symbols, e.g. `zirk_io_println(ptr, len)`.

`zirk-runtime` is the **ninth crate** of the workspace, in addition to the eight the roadmap proposes.

## Rationale

The stated goal of Phase 1 is *to validate that the whole architecture works end to end*. With option (a) the pipeline is validated but **not the architecture**: the normative lifecycle from `ZIRK_RUNTIME_SPEC.md` section 2 (validate permissions → load runtime → initialize globals → `main` → orderly shutdown → flush → exit code) has nowhere to exist, and it would have to be reintroduced by taking codegen apart.

With (b), Phase 1 already produces an LLVM `main` that calls `zirk_rt_init()` and `zirk_rt_shutdown()`, even though today both do nothing. That empty shape is exactly the hook where Phase 4 (memory) and Phase 5 (concurrency) attach without a refactor.

It also directly satisfies `ZIRK_RUNTIME_SPEC.md` section 1: a runtime that is *small, portable and linkable into standalone binaries*.

The C ABI boundary is the same one `ZIRK_LANGUAGE_SPEC.md` section 13 requires for native interoperability, so this is not disposable infrastructure: it is the definitive boundary, introduced early.

## Consequences

- Every supported target needs its own `zirk-runtime` compiled for that target. This relates to the sysroot risk from [ADR-004](./ADR-004-portabilidad.md).
- The `zirk_rt_*` and `zirk_io_*` symbols are a compatibility surface: changing them breaks already-compiled binaries. They must be versioned once the language stabilizes.
- The runtime cannot freely use Rust's stdlib if binary size is to be reduced later; using it for now is accepted, to be revisited in Phase 11.
