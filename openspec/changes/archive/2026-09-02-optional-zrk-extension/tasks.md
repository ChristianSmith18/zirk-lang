## 1. Add source path resolution

- [x] 1.1 Add `resolve_source_path` to `crates/zirk-cli/src/frontend.rs`.
- [x] 1.2 Use the resolved path in `crates/zirk-cli/src/main.rs` for `check` and `compile`.
- [x] 1.3 Use the resolved path in `crates/zirk-cli/src/bin/zirk-check.rs`.
- [x] 1.4 Update help text and error messages in `main.rs` and `zirk-check.rs`.

## 2. Update tests

- [x] 2.1 Add an end-to-end test that runs `zirk run` without `.zrk` on a valid program.
- [x] 2.2 Add a test that runs `zirk check` without `.zrk` on a valid program.
- [x] 2.3 Add a test that confirms `main.zrk` still works (no double extension).
- [x] 2.4 Run the full `zirk-cli` test suite.

## 3. Verify the workspace

- [x] 3.1 Run `cargo fmt --all --check`.
- [x] 3.2 Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] 3.3 Run `cargo test --workspace -- --test-threads=1`.

## 4. Closeout

- [x] 4.1 Run `openspec validate optional-zrk-extension --strict`.
- [ ] 4.2 Ask the user for explicit permission before archiving or committing.
