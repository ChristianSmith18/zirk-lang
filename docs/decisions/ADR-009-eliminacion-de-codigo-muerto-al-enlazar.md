# ADR-009 — Dead code elimination at link time

- **Status:** accepted
- **Date:** August 14, 2026
- **Phase:** 1

## Context

A minimal Zirk program — `fn main(): Void { stdout.println("..."); }` — linked to **~1.4 MB**, versus 336 KB for an equivalent `println!` in plain Rust and 33 KB in C. A 4x difference over plain Rust is not `std`'s known cost: it's a linking defect.

The cause: `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)) exposes several independent `extern "C"` symbols — `zirk_rt_init`, `zirk_str_from_i32`, `zirk_str_eq`, and the rest. A static archive (`.a`) is linked at **whole object file** granularity: if any symbol from a `.o` is referenced, the linker keeps that entire `.o`. With several distinct entry points, that retains much more of `std` than a specific program needs, and without requesting dead code elimination, that excess reaches the final binary intact.

## Decision

**Dead code elimination is requested from the linker**, with the flag corresponding to each container format:

| Platform | Container | Flag |
|---|---|---|
| macOS | Mach-O | `-Wl,-dead_strip` |
| Linux | ELF | `-Wl,--gc-sections` |
| Windows | COFF (`lld-link`) | `-Wl,/OPT:REF` |

`lld-link` does not implement `--gc-sections`; its equivalent is `/OPT:REF`. All three are passed through the same link driver (`clang`) already required by [ADR-004](./ADR-004-portabilidad.md).

## The real savings depend on the platform, and are not even

The first version of this ADR assumed the same flag would achieve comparable savings on all three platforms. That's not the case, and it's worth explaining rather than hiding it: whoever reads this ADR a year from now and compares sizes across platforms shouldn't have to rediscover it.

| Platform | Mechanism | Result |
|---|---|---|
| macOS | `ld64` eliminates per **symbol**, even within a single section — it doesn't depend on code being split into separate sections | ~1.4 MB → ~446 KB |
| Windows | LLVM emits functions in **COMDAT** sections by default for this target — `/OPT:REF` gets that fine granularity with nothing extra | improvement comparable to macOS |
| Linux | Neither mechanism applies: the precompiled `std` that `rustup` distributes does **not** have one section per function | marginal improvement, well below the other two |

This last point was verified, not assumed: the object files of the precompiled `libstd` were inspected (`ar x` on the sysroot's `.rlib`) and it was confirmed that a whole compilation module — 1332 symbols in the tested case — lives in a single `__text`. Without separate sections, `--gc-sections` on ELF can only discard whole object files, which is the same granularity that linking a `.a` already had **before** this flag.

Achieving on Linux the same result as on the other two platforms would require recompiling `std` with `-ffunction-sections`, via `-Z build-std` — a nightly feature, outside the stable toolchain this project pins (`rust-toolchain.toml`, [ADR-001](./ADR-001-pin-llvm.md) by analogy). It's out of scope for this phase.

## Verification

Measured on `aarch64-macos`, with the same build profile CI uses:

```
without the flag:  ~1.4 MB
with the flag:     ~446 KB    (in line with plain Rust)
```

In CI, Linux (x86_64 and aarch64) linked to ~3.9 MB with the flag — a real improvement, but bounded by the reason above — and Windows came in under 1 MB.

There is an end-to-end test with a 5 MB ceiling for the reference program's executable. It's deliberately generous: meant to catch a real regression — the flag disappears or stops being applied — and not to fake a uniform minimum size that the current architecture, with the stable toolchain, cannot offer on Linux.

## Consequences

- No behavior change: the flag only eliminates unreachable code.
- The savings grow with every new symbol `zirk-runtime` exposes on macOS and Windows; on Linux that growth is not mitigated by this flag.
- This doesn't compete with the Phase 8 optimization (LTO, binary size in `COMPILER_SPEC` section 5): this is dead code elimination at link time, orthogonal to optimizing the code that actually runs.
- **Noted as a future improvement**: if the size on Linux becomes a real problem — not just cosmetic — the paths are `-Z build-std` on nightly (changes the pinned toolchain) or reducing the number of `extern "C"` symbols the runtime exposes, grouping related functions into fewer object files so that file-level granularity matters less.
