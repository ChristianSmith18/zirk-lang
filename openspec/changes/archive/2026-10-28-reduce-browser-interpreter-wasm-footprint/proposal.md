## Why

The browser interpreter package built from compiler revision `59275f292ec3059a6717ffa03ae8d7a323762a92` is too large for its reviewed delivery budget. The verified `wasm-opt -Oz` package is 7,131,289 bytes, exceeding the 4,000,000-byte raw-WASM limit. Cargo currently enables default features throughout the browser dependency graph, pulling in `chrono-tz` through `zirk-sema` and `psl2`, `http`, and `url` through `zirk-runtime-core`. The Game Site does not need time-zone databases or native HTTP/cookie capabilities to run its course.

A release-only `CARGO_PROFILE_RELEASE_OPT_LEVEL=z` experiment reduced the package to 5,295,324 bytes, still 1,295,324 bytes over budget. `wasm-opt -O4` and metadata stripping on the original package only reduced it to 7,180,236 bytes. Compiler optimization alone is not a sufficient remedy.

## What Changes

- Add an explicit browser-minimal feature profile and make compiler/runtime capabilities opt-in where necessary, preserving native defaults.
- Exclude IANA time-zone database evaluation and native HTTP, cookies, URL-policy, and WebSocket implementations from the browser package when they are not required by the browser contract.
- Keep parsing and checking deterministic: unsupported imports/constructs must return stable capability diagnostics, not missing-symbol errors or panics.
- Keep the existing 4,000,000-byte raw, 1,450,000-byte gzip, and 1,200,000-byte Brotli error budgets unless a separately reviewed product decision changes them.
- Rebuild and behavior-verify the browser package, including all browser conformance fixtures and the valid native corpus parity contract; record raw, gzip, and Brotli sizes and exact compiler revision.
- Update the browser support matrix and Game Site package provenance to list any newly excluded feature families.

## Capabilities

### New Capabilities

None. This narrows the browser host package only; it does not change native language semantics or the native standard library.

### Modified Capabilities

None. The browser host delivery contract and package verification are engineering interfaces, not language specifications. `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `crates/zirk-browser`, `crates/zirk-sema`, `crates/zirk-ir`, `crates/zirk-interpreter`, and `crates/zirk-runtime-core` Cargo features and target configuration.
- Browser preflight, package conformance fixtures, corpus parity and package-size verification.
- `docs/browser-interpreter-support-matrix.md`, `docs/browser-oop-coverage.md`, and the companion Game Site browser package/provenance.

## Acceptance Criteria

1. The default native workspace behavior and tests remain unchanged.
2. The browser package passes package smoke, browser fixtures, and the supported native-corpus parity suite.
3. Browser requests requiring excluded time-zone or native HTTP/network facilities fail with documented deterministic diagnostics.
4. The optimized package meets all existing size error budgets without raising them.
5. The Game Site curriculum examples that remain playable pass through the exact delivered package.
