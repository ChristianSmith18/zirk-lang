## Context

The Zirk compiler is a Rust workspace with one crate per pipeline stage. Today the only public entry points are in `zirk-cli`, which depends on the full chain: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, and the runtime archive. `COMPILER_IMPROVEMENT_SUGGESTIONS.md` already identified this as the main pipeline gap: a developer cannot run `cargo build -p zirk-cli` and get a useful command on a machine without LLVM, even though the compiler frontend is complete.

`zirk check` is not a new language feature; it is a new boundary in the existing compiler. The front end already supports every phase implemented up to the current roadmap, and `zirk-diagnostics` already owns the diagnostic contract. The work is therefore to expose the frontend as a self-contained command with a build graph that stops at `zirk-sema`.

## Goals / Non-Goals

**Goals:**
- Add a `zirk check` command that runs the full frontend and exits before IR lowering.
- Make `zirk check` buildable and runnable without `LLVM_SYS_201_PREFIX` and without compiling `zirk-ir` or `zirk-codegen-llvm`.
- Reuse the existing `zirk-diagnostics` format and codes so `zirk check` and `zirk build` produce indistinguishable output.
- Prove the boundary with workspace tests that the `zirk-check` binary builds when the backend is not available.
- Keep `zirk build` and `zirk run` behavior unchanged.

**Non-Goals:**
- Implement a language server, formatter, or linter; those consume the same `zirk check` result but belong to later phases.
- Change the diagnostic format or add new diagnostic codes.
- Modify `.github/workflows/ci.yml`; this is not a CI change.
- Add public documentation in the handbook beyond the CLI reference; a follow-up change may do that after the command is proven.

## Decisions

### 1. Use a feature-gated `zirk-cli` with two binaries instead of a new crate

A separate `zirk-check` crate would duplicate CLI argument parsing and dispatch. Instead, `zirk-cli` will expose two binaries: `zirk` (the existing full compiler) and `zirk-check` (frontend only). The `zirk` binary requires a new `backend` Cargo feature; the `zirk-check` binary does not.

- `zirk-ir`, `zirk-codegen-llvm`, and the runtime archive dependencies move under the `backend` feature in `zirk-cli/Cargo.toml`.
- A shared library entry point in `zirk-cli` runs the frontend (load, lex, parse, merge, semantic) and returns a `FrontendResult`.
- `zirk-check` calls this shared entry and renders diagnostics.
- `zirk` calls the same entry, then continues with lowering and backend.

**Alternatives considered:**
- *Create a new `crates/zirk-check` crate.* Rejected: it would duplicate argument parsing and force a new driver abstraction before we know which parts are truly reusable. The feature-gated approach keeps the CLI surface in one crate and lets the shared driver emerge from this change.

### 2. The command accepts one entry file and discovers imports recursively

`zirk check` takes the same entry-file argument as `zirk build`. The loader walks `import` statements from the entry file, reads each discovered file, and feeds them to the pipeline. The frontend already does this; `zirk check` does not need a different loading strategy.

**Alternatives considered:**
- *Accept a list of files to check in isolation.* Rejected: it would require a different module-resolution semantics and would not match the project-wide `zirk build` behavior.

### 3. Exit codes follow the existing CLI convention

- `0` if the frontend completes with no errors.
- `1` if any error diagnostic is emitted.
- `2` if the command is invoked incorrectly (bad path, missing entry, wrong arguments).

`zirk build` already uses this same convention for the build-time case.

### 4. `--json` structured output is not a new format

`zirk check --json` uses the same JSON diagnostic envelope as `zirk build` (or explicitly fails with a roadmap diagnostic if JSON output is not yet implemented). The design requires that the JSON schema be shared, so no new consumer code is needed.

### 5. Frontend errors prevent any later stage

If `zirk check` emits a frontend error, the command exits without invoking `zirk-ir` or `zirk-codegen-llvm`. This is already the rule for `zirk build`, but the `zirk-check` binary codifies it by not having those crates in its dependency graph at all.

## Risks / Trade-offs

- **[Risk] Splitting `zirk-cli` by feature could break the default build if features are misconfigured.** → Mitigation: `zirk-cli/Cargo.toml` keeps `backend` as a default feature so `cargo build` and `cargo test` in `zirk-cli` still produce the full `zirk` binary. The `zirk-check` binary is built explicitly with `--no-default-features` in CI or tests.
- **[Risk] Conditional compilation in `zirk-cli` becomes complex and hard to read.** → Mitigation: the backend-only code is isolated in a `backend` module and `#[cfg(feature = "backend")]` is applied at module boundaries, not scattered inside functions.
- **[Risk] `zirk check` and `zirk build` drift if the frontend path is not actually shared.** → Mitigation: the shared driver is a single `run_frontend` function called by both binaries, and the `zirk-check` test suite asserts that the same input produces the same diagnostics in both commands.
- **[Risk] A consumer expects `zirk check` to type-check code that uses backend-only intrinsics.** → Mitigation: the spec states that `zirk check` covers the same language subset as `zirk build`. If the program references a backend-only feature, the same roadmap diagnostic is emitted as today.
- **[Trade-off] Feature-gating is more invasive than a runtime branch.** → Accepted: the `COMPILER_IMPROVEMENT_SUGGESTIONS.md` explicitly warns against a runtime branch, because it would not make the frontend buildable without the backend.

## Migration Plan

1. Add the `backend` feature to `zirk-cli/Cargo.toml` and move backend-only dependencies under it.
2. Refactor `zirk-cli/src/main.rs` into a shared `lib.rs` with `run_frontend` plus two binary targets.
3. Implement `zirk-check` binary that calls `run_frontend` and reports diagnostics.
4. Add a `crates/zirk-cli/tests/check.rs` test that runs `zirk-check` on valid, invalid, and import-containing fixtures and asserts exit codes.
5. Verify `cargo build -p zirk-cli --bin zirk-check --no-default-features` succeeds without `LLVM_SYS_201_PREFIX`.
6. Run `cargo fmt`, `cargo clippy`, `cargo build --workspace`, and `cargo test --workspace`.
7. If the CLI reference in the handbook needs updating, do it in a follow-up change; this change does not touch public documentation.

## Open Questions

- Should `zirk check` support `check --watch` in this change? No, that is Phase 9 territory and is recorded as a future possibility only.
- Does the public website list `zirk` subcommands explicitly? If so, a follow-up change will add `zirk check` after this one lands; this change itself does not affect public status evidence.

**Resolved:** the `backend` Cargo feature is the cleanest split. `zirk-cli` depends on `zirk-ir` and `zirk-codegen-llvm` only through the `driver` module and the `list_targets` / `build` / `run` paths in `main.rs`. All of those are gated behind `#[cfg(feature = "backend")]`, and the `zirk-check` binary builds without the feature enabled.
