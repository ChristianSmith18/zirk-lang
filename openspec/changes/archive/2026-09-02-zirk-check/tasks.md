## 1. Validate the feature-gating strategy

- [x] 1.1 Inspect `crates/zirk-cli/Cargo.toml` and `crates/zirk-cli/src/main.rs` to determine how `zirk-ir`, `zirk-codegen-llvm`, and the runtime archive are currently imported and used.
- [x] 1.2 Estimate whether moving backend dependencies under a `backend` Cargo feature is feasible without a large-scale refactor, or whether a separate `crates/zirk-check` crate is more practical.
- [x] 1.3 Document the chosen approach in `design.md` and update the Open Question about crate split vs feature flag.

## 2. Split the CLI build graph

- [x] 2.1 Add a `backend` feature to `crates/zirk-cli/Cargo.toml` and make `zirk-ir`, `zirk-codegen-llvm`, and any runtime archive dependency optional, gated by that feature.
- [x] 2.2 Keep `backend` enabled by default so that `cargo build -p zirk-cli` still produces the existing `zirk` binary.
- [x] 2.3 Add a `[[bin]]` entry for `zirk-check` that does not require the `backend` feature.
- [x] 2.4 Verify `cargo build -p zirk-cli --bin zirk-check --no-default-features` succeeds in an environment without `LLVM_SYS_201_PREFIX`.

## 3. Extract a shared frontend driver

- [x] 3.1 Create a shared `run_frontend` function (or driver) in `crates/zirk-cli/src/lib.rs` that loads the entry file, walks imports, lexes, parses, resolves names, and runs `zirk-sema` type and flow checking.
- [x] 3.2 Return a `FrontendResult` that contains the collected diagnostics and enough state for `zirk build` to continue with lowering if requested.
- [x] 3.3 Ensure the function uses `zirk-diagnostics` for all emitted diagnostics and preserves the existing diagnostic format and codes.

## 4. Add the `zirk-check` binary

- [x] 4.1 Create `crates/zirk-cli/src/bin/zirk-check.rs` that parses CLI arguments for the `check` subcommand and calls the shared `run_frontend` driver.
- [x] 4.2 Render diagnostics to the terminal in the same human-readable form as `zirk build`.
- [x] 4.3 Implement exit codes: `0` for a clean check, `1` for frontend errors, `2` for bad invocation.
- [x] 4.4 Add support for `zirk check --json` that emits the same structured output as `zirk build`, or emit a roadmap diagnostic if that flag is not yet implemented.

## 5. Preserve the full compiler behavior

- [x] 5.1 Update `crates/zirk-cli/src/main.rs` (or `bin/zirk.rs`) to call `run_frontend` and then continue with `zirk-ir` lowering, `zirk-codegen-llvm` emission, and linking when the `backend` feature is enabled.
- [x] 5.2 Verify that `zirk build` and `zirk run` produce the same output and exit codes as before for the existing test corpus.
- [x] 5.3 Ensure no `zirk build` test fails because of the new feature gating.

## 6. Add tests for `zirk check`

- [x] 6.1 Add `crates/zirk-cli/tests/check.rs` with a valid program that returns code `0`.
- [x] 6.2 Add a test for a program with a syntax error that returns a non-zero code and emits a diagnostic.
- [x] 6.3 Add a test for a program with a type error that returns a non-zero code and emits a diagnostic.
- [x] 6.4 Add a test for a nonexistent file that returns code `2`.
- [x] 6.5 Add a test that verifies `zirk check` and `zirk build` produce the same diagnostics for the same invalid input.

## 7. Verify the no-LLVM build

- [x] 7.1 Run `cargo build -p zirk-cli --bin zirk-check --no-default-features` in an environment where `LLVM_SYS_201_PREFIX` is unset.
- [x] 7.2 Run the new `zirk-cli` tests with the default features to ensure the full compiler still builds.
- [x] 7.3 Add a CI step (or at least document in `design.md`) that builds `zirk-check` without the backend to prevent regressions.

## 8. Update `scripts/check-local.sh` (optional but recommended)

- [x] 8.1 Add `zirk check` as an optional fast first gate before `cargo build`, or document that `zirk check` can be used by contributors for quick validation.
- [x] 8.2 If the script is updated, verify it still matches `.github/workflows/ci.yml` in step order and intent.

## 9. Full verification

- [x] 9.1 Run `cargo fmt --all --check`.
- [x] 9.2 Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] 9.3 Run `cargo build --workspace`.
- [x] 9.4 Run `cargo test --workspace -- --test-threads=1`.
- [x] 9.5 Run `cargo build -p zirk-cli --bin zirk-check --no-default-features` again to confirm the no-LLVM build is still green.

## 10. Closeout

- [x] 10.1 Re-read `docs/TOOLCHAIN.md` and add a note that `zirk check` does not require LLVM, if not already clear.
- [x] 10.2 If the CLI reference in the handbook mentions subcommands, decide whether to update it in this change or in a follow-up.
- [x] 10.3 Run `openspec validate zirk-check --strict`.
- [x] 10.4 Mark all tasks complete and archive the change with `openspec archive zirk-check --yes`.
