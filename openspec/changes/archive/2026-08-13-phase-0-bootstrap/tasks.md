## 1. Workspace skeleton

- [x] 1.1 Create the root `Cargo.toml` as a workspace with resolver 2 and shared dependencies in `[workspace.dependencies]`
- [x] 1.2 Create `rust-toolchain.toml` pinning the Rust version and the `rustfmt` and `clippy` components
- [x] 1.3 Create the nine crates in `crates/`: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`, `zirk-runtime`
- [x] 1.4 Document each crate's responsibility and boundary in its `lib.rs`, per D1 of the design
- [x] 1.5 Declare dependencies between crates respecting the pipeline's single direction
- [x] 1.6 Verify that `cargo build` and `cargo clippy` pass cleanly on the whole workspace

## 2. Diagnostics

- [x] 2.1 Define in `zirk-diagnostics` the severity types, stable code, and source location
- [x] 2.2 Define the diagnostic structure with cause and optional help
- [x] 2.3 Implement the human-readable rendering of the `COMPILER_SPEC` §8 format, with a source excerpt and a column marker
- [x] 2.4 Implement rendering for when the source is not available
- [x] 2.5 Implement structured output for tooling
- [x] 2.6 Implement warning-to-error elevation
- [x] 2.7 Snapshot tests of the rendered format, with and without a source excerpt

## 3. LLVM integration

- [x] 3.1 Add `inkwell` 0.10 with the `llvm20-1` feature and no default features to `zirk-codegen-llvm`
- [x] 3.2 Write the build script that checks the LLVM major version and fails with a message referencing `docs/TOOLCHAIN.md` (D3)
- [x] 3.3 Verify the build script fails with a clear message when `LLVM_SYS_201_PREFIX` is missing
- [x] 3.4 Port the sanity check from the spike: build module, verify, emit object, link, and run
- [x] 3.5 Turn the sanity check into a permanent test of the crate, not an example binary (D2)

## 4. Target matrix

- [x] 4.1 Define the table of the nine targets in the spec with their corresponding LLVM triple
- [x] 4.2 Implement object emission per target
- [x] 4.3 Test that emits an object for each of the nine targets and validates the container format and architecture
- [x] 4.4 Test that verifies failure with a diagnostic for an unrecognized triple

## 5. Runtime

- [x] 5.1 Configure `zirk-runtime` as a `staticlib` in its `Cargo.toml`
- [x] 5.2 Define `zirk_rt_init` and `zirk_rt_shutdown` as `extern "C"` with no mangling, with an empty body (D-Open Questions)
- [x] 5.3 Test that verifies the produced artifact is a linkable static library
- [x] 5.4 Test that verifies the exported symbols appear without mangling

## 6. Continuous integration

- [x] 6.1 Create `.github/workflows/ci.yml` with the `{linux, macos, windows}` matrix
- [x] 6.2 Install LLVM 20.1 per platform: `apt.llvm.org` on Linux, `brew install llvm@20` on macOS, an inkwell-compatible build on Windows (the official distribution does not work, see #2)
- [x] 6.3 Define `LLVM_SYS_201_PREFIX` per platform in the workflow, never versioned in the repo (D4)
- [x] 6.4 Cache the LLVM install and Cargo dependencies (D5)
- [x] 6.5 Run `cargo build`, `cargo test`, `cargo clippy`, and `cargo fmt --check` in every job
- [x] 6.6 Extend the matrix to aarch64 on Linux and macOS
- [x] 6.7 Resolve whether `aarch64-windows` enters the matrix depending on runner availability
- [x] 6.8 Confirm that the Windows job builds `llvm-sys` correctly — the main risk of this change

## 7. Closure

- [x] 7.1 Verify that CI is green on all three platforms; until then, portability is considered unverified
- [x] 7.2 Update `docs/decisions/README.md` marking the pending items on workspace layout and diagnostics format as resolved
- [x] 7.3 Record in ADR-004 the actual result of the portability verification
- [x] 7.4 Update `docs/TOOLCHAIN.md` with any correction that arises from setting up CI
</content>
