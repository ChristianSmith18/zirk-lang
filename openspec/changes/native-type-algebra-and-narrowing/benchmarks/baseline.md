# Type algebra performance baseline

This is a pre-feature baseline for task 1.8, captured on the current implementation before the general type-algebra, narrowing, and union-layout work is complete.

## Environment and method

- Host: macOS 27.0.1, arm64.
- Source revision: `f8d87220` (working tree contained unrelated and in-progress changes; these values are a local baseline, not a clean-build comparison).
- CLI and LSP binaries were already built in debug mode. Measurements exclude Rust compilation and report process startup plus analysis.
- `/usr/bin/time -l` supplied elapsed time and maximum resident set size. The LSP harness starts the language server, sends `initialize`, opens one file, waits for its first `publishDiagnostics`, then shuts down. `/usr/bin/time -l` measures the Python harness process here, not the LSP subprocess tree; the LSP RSS figures below are not server-memory measurements.
- The LSP returns zero diagnostics for each fixture. Times below are single runs and are directional; they are not statistically stable performance claims.

## Fixtures

Generate the stress sources with:

```sh
python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/generate_fixtures.py target/type-algebra-baseline
```

This produces 128 flat aliases with four union members each and 48 nested `Array<T>` aliases. The fixtures isolate parser/checker and editor analysis cost; they do not represent runtime union lowering or stress every planned operator.

## Captured results

| Input | `zirk check` elapsed | CLI maximum RSS | LSP initialize | `didOpen` to diagnostics | LSP harness parent maximum RSS |
|---|---:|---:|---:|---:|---:|
| `demo/02-nullabilidad-y-narrowing.zrk` | 0.07 s | 25,526,272 B | 0.016757 s | 0.002545 s | 25,903,104 B |
| `demo/12-genericos-y-contratos.zrk` | 0.09 s | 25,608,192 B | 0.024698 s | 0.011287 s | 25,624,576 B |
| `flat-128.zrk` | 0.01 s | 24,985,600 B | 0.016836 s | 0.002524 s | 25,804,800 B |
| `nested-48.zrk` | <0.01 s | 24,035,328 B | 0.024220 s | 0.008981 s | 25,837,568 B |

CLI command for each source:

```sh
/usr/bin/time -l target/debug/zirk check <source.zrk>
```

LSP command for each source:

```sh
/usr/bin/time -l python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/lsp_did_open.py <source.zrk>
```

## Interpretation and follow-up

The flat aliases complete below the displayed CLI time resolution and the nested aliases complete in under 10 ms in this small fixture. The representative demos show higher analysis cost, especially for the generic demo's LSP open. These numbers are only a baseline for subsequent comparisons on the same machine and binaries. They do not set final budgets; task 12.6 must add large flat-N unions/intersections, contextual candidates, recursive aliases, conditional/template products, and repeated incremental edits. Run at least 10 warmed samples and report median and p95 before selecting user-facing budgets.

## Task 12.6 stress sweep

The generated fixtures now also cover 64-member structural intersections, 48
nested union/intersection aliases, 128 repeated conditional instantiations, a
64-result template product, guarded recursive object aliases, 32 contextual
structural-union candidates, and five incremental LSP edits. Run the warmed
distribution sweep with:

```sh
python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/generate_fixtures.py target/type-algebra-stress
python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/run_stress_sweep.py target/type-algebra-stress --samples 10 --change-count 5
```

Captured 2026-10-26 on macOS 27.0.1, Apple M5 Pro, arm64, with debug binaries
from the modified worktree. `HEAD` is `f8d87220`; feature changes were
uncommitted during measurement, so these values describe this implementation
snapshot and are not a clean commit-to-commit comparison. CLI elapsed time
includes process startup, checking, and any source-located lowering diagnostic.
The LSP open/change columns measure time from the protocol notification to the
matching diagnostics response, excluding server initialization. Each fixture
has one warm-up followed by 10 samples; each incremental sample sends five
successive full-document `didChange` notifications. The p95 uses nearest-rank
selection.

| Fixture | CLI median | CLI p95 | CLI nonzero samples | LSP `didOpen` median / p95 | LSP edit median / p95 | LSP diagnostics |
|---|---:|---:|---:|---:|---:|---:|
| `flat-128.zrk` | 12.688 ms | 14.276 ms | 0/10 | 2.605 / 2.710 ms | 2.141 / 2.374 ms | 0 |
| `nested-48.zrk` | 11.779 ms | 12.947 ms | 0/10 | 1.451 / 1.811 ms | 0.906 / 1.128 ms | 0 |
| `flat-intersection-64.zrk` | 12.853 ms | 13.599 ms | 10/10 | 3.790 / 3.982 ms | 3.124 / 3.517 ms | 65 |
| `nested-algebra-48.zrk` | 12.026 ms | 14.904 ms | 0/10 | 1.775 / 2.068 ms | 1.272 / 1.602 ms | 0 |
| `conditional-128.zrk` | 12.235 ms | 13.445 ms | 0/10 | 2.272 / 2.404 ms | 1.696 / 1.848 ms | 0 |
| `template-product-64.zrk` | 11.155 ms | 14.014 ms | 0/10 | 1.339 / 1.467 ms | 0.735 / 0.908 ms | 0 |
| `recursive-object-alias.zrk` | 11.288 ms | 11.661 ms | 10/10 | 1.137 / 1.356 ms | 0.522 / 0.660 ms | 1 |
| `contextual-shapes-32.zrk` | 12.472 ms | 13.273 ms | 10/10 | 3.021 / 3.213 ms | 2.376 / 2.675 ms | 33 |

The 10 CLI failures on intersection/contextual structural fixtures and the
recursive alias failure are the existing `E0423` lowering boundary for
anonymous structural types; they are counted rather than discarded. The LSP
diagnostic counts expose that boundary in analysis, too. Other fixtures had no
diagnostics. The successful CLI cases take 11.155–12.688 ms median versus the
single-run pre-feature stress values of under 10 ms; this is a small directional
increase, not a statistically comparable regression claim because the source
revision, feature set, and measurement sampling differ. These results establish
repeatable fixture coverage and a current latency distribution, but do not
select new budgets. The CLI RSS comparison is recorded below; the remaining
non-host ABI/runtime checks remain open.

## Task 12.10 memory review

Captured 2026-10-26 on the same macOS 27.0.1 Apple M5 Pro arm64 host using
debug binaries from clean baseline revision `f8d87220` and the current feature
worktree. Both revisions used the same Rust toolchain, target directory, build
profile, generated fixtures, and measurement script. Each fixture had one
warm-up followed by 10 samples; p95 uses nearest-rank selection. This compares
source revisions, but the feature worktree is still uncommitted and represents
the current implementation snapshot.

```sh
python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/measure_rss.py target/type-algebra-stress --samples 10
```

| Fixture | Baseline CLI median / p95 RSS | Feature CLI median / p95 RSS | Median delta |
|---|---:|---:|---:|
| `conditional-128.zrk` | 21.67 / 21.69 MB | 30.12 / 30.21 MB | +8.45 MB |
| `contextual-shapes-32.zrk` | 21.74 / 21.76 MB | 30.43 / 30.49 MB | +8.68 MB |
| `flat-128.zrk` | 23.93 / 23.99 MB | 30.28 / 30.34 MB | +6.35 MB |
| `flat-intersection-64.zrk` | 21.77 / 21.82 MB | 30.48 / 30.54 MB | +8.71 MB |
| `nested-48.zrk` | 23.33 / 23.36 MB | 29.20 / 29.33 MB | +5.87 MB |
| `nested-algebra-48.zrk` | 21.71 / 21.74 MB | 29.88 / 29.90 MB | +8.18 MB |
| `recursive-object-alias.zrk` | 21.23 / 21.27 MB | 29.29 / 29.34 MB | +8.06 MB |
| `template-product-64.zrk` | 21.20 / 21.25 MB | 29.38 / 29.43 MB | +8.18 MB |

CLI peak RSS increased by 5.87–8.71 MB (about 25–41% of the baseline median)
on these fixtures. The increase is repeatable across all eight cases, but the
measurement does not isolate which compiler structures account for it. No
regression is accepted and no budget is relaxed: before release, profile the
added memory, document the responsible allocations and mitigation, and set an
explicit budget. LSP parent RSS is excluded from this comparison because the
harness measurement does not include its server child; earlier single-run LSP
numbers are not evidence of language-server memory.

## Task 10.9 inline payload boundary

The selected first-release strategy is bounded inline storage with a source
diagnostic above 4096 bytes; it does not select a boxed fallback. The following
reproducible benchmark builds a tuple payload whose enum discriminator makes
the union payload exactly 4096 bytes, then compares it with the next 4-byte
step (4100 bytes), which must fail with `E0513`:

```sh
cargo build -p zirk-cli --bin zirk
python3 openspec/changes/native-type-algebra-and-narrowing/benchmarks/measure_union_payload_limit.py target/type-algebra-payload-benchmark --samples 10
```

Captured 2026-10-26 on the macOS 27.0.1 Apple M5 Pro arm64 host, using the
debug `target/debug/zirk` binary. The script performs two warm-up builds per
case and records ten process wall-time samples; p95 uses nearest-rank. The
accepted case includes LLVM code generation and native linking. The rejected
case includes parsing, checking, layout derivation, and diagnostic rendering;
it does not invoke LLVM or the linker.

| Case | Result | Median build time | p95 build time | Range |
|---|---|---:|---:|---:|
| 4096-byte payload | native build succeeds | 228.56 ms | 282.37 ms | 218.49–282.37 ms |
| 4100-byte payload | `E0513` before code generation | 14.18 ms | 14.90 ms | 13.67–14.90 ms |

These measurements validate the cost of the accepted boundary and the early
rejection path; they are not a comparison against a boxed implementation.
`cargo test -p zirk-cli --test check oversized_union_payload_has_source_level_size_diagnostic -- --exact`
asserts both sides of the exact boundary. A future change that selects managed
boxing must add its own allocation, GC, ABI, and latency evidence before
replacing the current explicit rejection policy.
