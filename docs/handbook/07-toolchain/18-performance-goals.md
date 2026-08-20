# Performance Goals

Startup, parsing, formatting, incremental checks, full/no-change builds, LSP latency, memory, and binary size are public benchmarks—not hardware-independent promises. Results record toolchain, target, hardware, dataset, and variance.

The benchmark suite separates cold and warm startup, full and no-change builds,
single-file and public-API edits, interactive p50/p95/p99 latency, peak resident
memory, cache size, emitted object size, link time, and final binary size.
Datasets include small CLI, multi-module service, generic-heavy code,
decorator-heavy code, and invalid-source recovery.

Every published result records commit, compiler/profile, LLVM/linker, target,
host hardware/OS, dataset hash, warm-up, sample count, summary statistics, and
noise controls. A regression threshold opens investigation; it is not silently
relaxed to make CI green.

Correctness and safety gates run before performance comparison. An optimization
that changes diagnostics nondeterministically, skips permission checks, loses
cleanup, or changes debug/release behavior is rejected regardless of speed.

---

**Previous:** [← ABI and IR Compatibility](17-abi-and-ir-compatibility.md) · **Next:** [ Testing](../08-testing/README.md)
