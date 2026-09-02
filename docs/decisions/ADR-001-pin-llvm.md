# ADR-001 — Pinning LLVM 20.1

- **Status:** accepted
- **Date:** August 12, 2026
- **Phase:** 0

## Context

`ZIRK_COMPILER_SPEC.md` section 5 fixes LLVM as the only initial backend. The implementation stack uses `inkwell` on top of `llvm-sys`, which links against the C++ ABI of **one specific major version** of LLVM: a different version does not compile, this is not a style preference.

`inkwell` 0.10 supports from `llvm15-0` to `llvm22-1`. Homebrew offers from `llvm@14` to `llvm@22`.

## Decision

**LLVM 20.1.x** is pinned, with the `llvm20-1` feature of `inkwell` 0.10.

The selection criterion is **availability on all three platforms over novelty**. Given the portability requirement from [ADR-004](./ADR-004-portabilidad.md), a version that is hard to obtain on any of the three platforms blocks the entire project; a version with slightly older APIs only costs convenience.

LLVM 20.1 is available as a development package with static libraries on:

- macOS — `brew install llvm@20`
- Linux — `apt.llvm.org`, package `llvm-20-dev`
- Windows — **not** the official distribution. See below.

## Verification

The sanity check required by the roadmap (Phase 0) was run and passed on `aarch64-macos`:

```
inkwell 0.10 → LLVM IR → Mach-O arm64 object → link → native binary → runs (exit 0)
```

The resulting binary is a `Mach-O 64-bit executable arm64` that only depends on `libSystem`.

Verified local installation: LLVM 20.1.8 with 203 static libraries present, lld 20.1.8 with all four drivers (`ld.lld`, `ld64.lld`, `lld-link`, `wasm-ld`).

## Consequences

- Every development machine and every CI runner must provide LLVM 20.1 with static libraries and expose `LLVM_SYS_201_PREFIX`. See [TOOLCHAIN.md](../TOOLCHAIN.md).
- Bumping the LLVM major version is a deliberate change with its own ADR, not a routine update.
- On Windows, **no** official LLVM distribution works with `llvm-sys`. The `.exe` installer does not include static libraries, and the development tarball, which does include them, is compiled against the static CRT while Rust uses the dynamic one: the process aborts with `STATUS_ACCESS_VIOLATION` on the first LLVM call that returns a string.

  `TyrsDev/llvm-package-windows` is used, a build maintained specifically so that `inkwell` works on Windows, at the same pinned version. It is a single-maintainer dependency and constitutes a consciously accepted supply-chain risk; the alternative is building LLVM from source with `LLVM_USE_CRT_RELEASE=MD`, which takes hours per build.

  The four discarded combinations and their reasoning are in issue #2.
