## 1. Baseline and feature map

- [x] 1.1 Record baseline compiler revision and package size for raw WASM, gzip, and Brotli from a clean worktree using the reviewed build scripts.
- [x] 1.2 Trace `chrono-tz`, `psl2`, `http`, and `url` from Cargo dependency edges to the exact runtime/sema modules and browser-visible operations; include the complete resolved feature tree.
- [x] 1.3 Inventory temporal and HTTP/network browser fixtures and the diagnostic path used when the browser package excludes a capability.

## 2. Implement browser-minimal dependency profile

- [x] 2.1 Add opt-in features for zone-database evaluation and native HTTP/policy modules while preserving current native default behavior.
- [x] 2.2 Disable unneeded defaults across every crate in the browser dependency graph; verify feature unification does not re-enable excluded dependencies.
- [x] 2.3 Gate only browser-inapplicable code paths and return deterministic capability diagnostics for excluded timezone and HTTP/network operations.
- [x] 2.4 Add positive browser fixtures for ordinary dates/times, collections, enums, objects/contracts, and course APIs; add negative fixtures for excluded zoned/HTTP operations.

## 3. Verify and publish evidence

- [x] 3.1 Run focused crate tests for modified sema/runtime feature combinations and the full browser crate test suite, including corpus parity.
- [x] 3.2 Build the clean browser package, require package behavior equivalence, verify raw/gzip/Brotli error budgets, and record the exact source revision and measurements.
- [x] 3.3 Run all curriculum solutions and revised lesson programs through the delivered browser package.
- [x] 3.4 Update the browser support matrix, package handoff documentation, and Game Site generated provenance with the verified exclusion boundary.
- [x] 3.5 Run final native regression checks for default features and review Cargo feature trees for native and browser targets.
