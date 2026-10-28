## Context

The current WASM dependency graph includes default features for `zirk-sema` and `zirk-runtime-core`. `chrono-tz` is used by constant evaluation for IANA zone validation and zoned date-time operations. `psl2`, `http`, and `url` are dependencies of shared HTTP, cookie, origin-policy, and WebSocket code. Browser policy already excludes network host effects, but Cargo still makes these modules available to the browser build.

Measured on a clean detached worktree at compiler revision `59275f292ec3059a6717ffa03ae8d7a323762a92`:

| Build | Raw WASM | Brotli | Result |
|---|---:|---:|---|
| Release profile + `wasm-opt -Oz` | 7,131,289 B | 1,550,723 B | Fails 4,000,000 B raw and 1,200,000 B Brotli budgets |
| Release `opt-level=z` + `wasm-opt -Oz` | 5,295,324 B | 1,235,113 B | Still fails both budgets |
| `wasm-opt -O4` + debug/producer stripping (existing optimized artifact) | 7,180,236 B | 1,550,723 B | No useful reduction |
| Browser-minimal profile + `chrono` without clock defaults, `wasm-opt -Oz` (`89662c0def8973cf251972aa91c53f99ff90b266`) | 4,154,454 B | 1,086,252 B | Behavior digest matches 363 cases; raw still 154,454 B over budget, gzip 1,449,867 B passes by 133 B |

The first three rows are historical pre-feature baselines. The 89662c0d row was an intermediate package: its integrity and 363-case behavior digest (`496be73859900980805a321d935e04d7093e326b4443de41714347070c3f5269`) passed, but raw WASM remained over budget. Final acceptance is recorded below.

## Decisions

### Add opt-in features without changing native defaults

Introduce narrowly named Cargo features for zone-database constant evaluation and native HTTP value/policy implementations. Native-facing crates retain their current defaults. The browser dependency chain explicitly disables defaults and enables only the features required by parsing, type checking, IR lowering, and the browser interpreter.

Before gating a feature, trace its symbols through every crate and test target. Cargo feature unification means `default-features = false` in `zirk-browser` is insufficient if `zirk-ir`, `zirk-interpreter`, or `zirk-sema` independently re-enable the same feature; every edge in the browser graph must be reviewed.

### Keep unsupported-source behavior intentional

Browser imports for excluded modules must be rejected during browser preflight with the existing capability diagnostic mechanism. Time-zone-dependent compile-time constants and operations must likewise receive an explicit browser capability diagnostic. Do not remove their parser/AST representation or change native evaluation.

### Measure before/after and keep package proof separate

Record `stat` raw bytes plus gzip and Brotli measurements. Require the same browser behavior digest before/after `wasm-opt`; run `verify-browser-package.sh` from a clean committed revision. If feature-gating does not meet every current budget, inspect WASM size composition before considering further cuts; do not raise budgets or claim the package is fixed based only on compressed transfer size.

### Update user-facing claims only after evidence

The browser support matrix and Game Site provenance describe the exact verified package. Mark time-zone and HTTP APIs excluded only if their preflight tests and package behavior prove that boundary. Keep the native feature-status catalog unchanged unless a separate native behavior defect is found.

## Risks

- Feature unification may silently retain dependencies through another crate edge. Inspect the resolved `cargo tree --target wasm32-unknown-unknown -e features` and binary size after each change.
- Removing timezone evaluation may affect unrelated temporal literals if code paths are not separated narrowly. Add positive tests for non-zone temporal values and negative tests for zoned operations.
- The Game Site package may use source constructs outside its current course. Run all lesson and corpus checks against the final package before updating provenance.
- LTO and `wasm-opt` may retain code referenced by a shared dispatch table. Prefer explicit capability/features and validate behavior over linker-only assumptions.

## Final acceptance evidence (2026-10-28)

The final browser package was built from compiler revision
`d236632d41e32172436b226992192142895f35af`. Disabling regex acceleration
backends for the WASM profile, while retaining the full Unicode regex feature,
closed the raw size gap without reducing accepted regex syntax. The package
behavior corpus, including Unicode general categories, scripts, binary
properties, and case folding, passes 363 cases with digest
`62b109c9e2bd2a0a54159ebb01b104cf4725bc9e147bc17f8922b5870ddefe75`.

| Metric | Final package | Error budget | Warning threshold | Result |
| --- | ---: | ---: | ---: | --- |
| Raw WASM | 3,920,114 B | 4,000,000 B | 3,700,000 B | Error budget passes; warning remains |
| gzip | 1,357,621 B | 1,450,000 B | 1,300,000 B | Error budget passes; warning remains |
| Brotli | 1,026,160 B | 1,200,000 B | 950,000 B | Error budget passes; warning remains |

`verify-browser-package.mjs` passed integrity and behavior checks, and
`check-browser-package-size.sh` passed all enforced error budgets. The native
feature defaults still enable timezone, HTTP values/policy, and regex
acceleration. The browser package is built with `--no-default-features`; its
normal/build dependency tree contains none of `chrono-tz`, `psl2`, `http`, or
`url`. The wider workspace `cargo tree` can show these packages through other
workspace members or test-only dependencies, so package review uses the
browser package's normal/build edges. Browser HTTP operations continue to fail
capability preflight with `ZB002`; timezone names remain unavailable, while
ordinary temporal operations pass.

The sibling Game Site checks the exact checked-in package, not a host
interpreter: `yarn curriculum:verify:browser` passed 53 levels, 8 findings, 14
diagnostics, and 226 browser runs. The site unit suite passed 82 files and 240
tests (2 files / 8 tests skipped); the production build prerendered 93 static
routes and generated a sitemap for 92 public learning routes. Node emitted a
non-fatal `MODULE_TYPELESS_PACKAGE_JSON` warning while loading the generated
ES module binding; package integrity and all checks still passed.

The browser feature map behind the size result is:

| Dependency/capability | Native implementation | Browser boundary |
| --- | --- | --- |
| `chrono-tz` | `zirk-sema` timezone constant evaluation in `src/const_eval.rs`; propagated through IR and interpreter timezone features | Optional `timezone` feature is off; zoned names/operations are rejected in browser preflight |
| `http`, `url`, `mime`, `sfv` | `zirk-runtime-core/src/http_values.rs` supports shared HTTP value types | Optional `http-values` is off for the browser profile |
| `psl2`, `httpdate` | `zirk-runtime-core/src/http_cookie.rs` and native HTTP policy/cookie implementation | Behind `native-http`, not in the browser normal/build graph |
| `http-fetch` | `zirk-browser` injected fetch integration | Off by default in the shipped package; HTTP source operations diagnose `ZB002` before execution |
| `regex/perf` | Native defaults retain acceleration backends | Off in browser; Unicode syntax and semantics remain enabled |

The original package and source audit also confirms the exclusion is not a
source-language deletion: parser/AST support remains, and browser diagnostics
make the profile boundary explicit. The positive temporal fixture is
`crates/zirk-browser/fixtures/temporal-range.json`; HTTP exclusion tests live
with browser preflight tests in `crates/zirk-browser/src/lib.rs`.

Native regression ran with workspace defaults on macOS aarch64:
`cargo test --offline -p zirk-sema -p zirk-runtime-core -p zirk-interpreter
--lib` reported 345 passed, 1 ignored. Native feature-tree review confirmed
that the default interpreter retains timezone, HTTP values, and regex
acceleration. A broader `cargo test --offline --workspace` was attempted, but
its `zirk-cli` end-to-end suite remained blocked for over seven minutes in
three loopback HTTP server tests (`bundled_http_server_invokes_handler_on_accepted_stream`,
`bundled_http_server_routes_query_with_content_and_content_type`, and another
accepted-stream server case) waiting on child `zirk run .` processes. That
run was interrupted; it is not counted as passing evidence. The focused
default-feature regression passed and is the evidence for this task.
