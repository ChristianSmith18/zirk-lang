## 1. Public stream model and standard source

- [x] 1.1 Define `IoError` and generic `ReadResult<T>` in the standard type surface, including value, end, decoding, limit, and exact-read EOF outcomes; verify parser, semantic, and API-metadata tests cover every public variant
- [x] 1.2 Replace placeholder declarations in `stdlib/std/io/{stdin,stdout,stderr}.zrk` with the complete shared stream API and verify the parsed source signatures match the compiler-owned registry
- [x] 1.3 Require an explicit `std.io` import for `stdout.print` and `stdout.println` while preserving default member opening after import; migrate and verify output corpus programs

## 2. Semantic provenance and lowering

- [x] 2.1 Record fully-qualified standard stream provenance during import resolution so dot calls, function values, and type references resolve only from `std.io`; verify an external same-name package never receives a stream binding
- [x] 2.2 Extend the standard-library registry with every delivered stdin/stdout/stderr signature and validate registry-to-source agreement in unit tests
- [x] 2.3 Lower output, flush, and input operations through checked registry bindings rather than short names; verify ordinary external calls retain ordinary call lowering
- [x] 2.4 Model `ReadResult<T>` and `IoError` in semantic analysis, IR layouts, pattern matching, and diagnostics; verify empty line, EOF, invalid UTF-8, oversize, and premature exact EOF outcomes

## 3. Native runtime and interpreter

- [x] 3.1 Add stable native runtime ABI operations for stdout, stderr, flushing, and bounded stdin reads; verify exported symbol names and native stdout/stderr destination separation
- [x] 3.2 Implement strict UTF-8 line, character, byte, and exact-byte reads with configured allocation limits; verify EOF and invalid input behavior using deterministic injected test streams
- [x] 3.3 Extend the interpreter host capability interface with bounded stdin fixtures and target-labelled stdout/stderr events; verify native and interpreter conformance for rendering, line endings, EOF, and failures
- [x] 3.4 Add native end-to-end fixtures for piped input, output flushing, stderr, Unicode, and failure propagation; verify each expected output and typed error

## 4. Browser capability boundary

- [x] 4.1 Mark reachable stdin reads as an excluded browser effect before interpretation and verify no prompt, DOM, storage, or network API is used
- [x] 4.2 Preserve bounded browser stdout/stderr event capture with destination metadata and verify call-order conformance against interpreter fixtures
- [x] 4.3 Audit the browser dependency graph and capability preflight tests to verify `zirk-runtime`, native streams, and ambient input remain excluded

## 5. Documentation, status, and release evidence

- [x] 5.1 Update `docs/ZIRK_STDLIB_SPEC.md`, handbook `std.io` pages, API examples, and `ADR-021` references with the final stream signatures and `module.member` calling convention; verify every example with `zirk check` or `zirk run`
- [x] 5.2 Update feature status, roadmap, current limitations, and README only after native/interpreter/browser evidence is available; verify claims distinguish delivered synchronous streams from deferred async and terminal features
- [x] 5.3 Run `cargo fmt --all --check`, focused crate tests, the full affected workspace suite, Clippy with `-D warnings`, and `openspec validate add-stdin-stdout-standard-library --strict`; record any platform limitation
- [x] 5.4 Commit the clean zirk-lang implementation and documentation, run `./scripts/sync-website-content.sh`, review the `../zirk-lang-site` diff with a human-reviewed `--audit-date YYYY-MM-DD` when status evidence changes, validate both repositories, and commit the website synchronization separately
