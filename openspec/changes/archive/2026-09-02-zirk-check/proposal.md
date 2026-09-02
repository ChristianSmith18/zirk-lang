## Why

The Zirk compiler currently has no way to validate a project without attempting a full build, including LLVM IR generation, object emission, native linking, and runtime archive lookup. Every frontend-only check — lexer, parser, name resolution, type checking, and flow analysis — is reachable only as a side effect of `zirk build` or `zirk run`, both of which fail if `LLVM_SYS_201_PREFIX` is not configured. This is expensive for a developer who only wants to know whether a `.zrk` file is well-formed, and it is impossible for CI jobs, editor integrations, or agents that should run the fastest possible validation pass without a complete LLVM toolchain.

`COMPILER_IMPROVEMENT_SUGGESTIONS.md` lists an LLVM-independent `zirk check` as the first P0 item. It is also the natural prerequisite for the editor/LSP work in Phase 9: a language server, formatter, linter, or LSP consumer all need a frontend result they can consume without paying for backend compilation. Without this boundary, every tooling change in the roadmap continues to drag the LLVM dependency with it, and the `zirk-cli-commands` spec keeps treating "compile" and "validate" as the same operation.

Now is the right time because the workspace is green and the frontend crates (`zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`) already form a clean pipeline. The only missing piece is an explicit entry point that stops after semantic checking and an architectural rule that keeps the new command free of `zirk-ir`, `zirk-codegen-llvm`, and the runtime.

## What Changes

- Introduce a new `zirk check` subcommand that loads one or more `.zrk` files, runs the complete frontend pipeline (lex, parse, module/name resolution, type/flow checking), and exits without attempting IR lowering, object emission, or linking.
- Make `zirk check` work when `LLVM_SYS_201_PREFIX` is unset, so that it can be built and run on machines that do not have LLVM installed. This may require a new Cargo feature or a crate split that excludes `zirk-ir` and `zirk-codegen-llvm` from the `zirk check` build graph.
- Add a `zirk check --json` mode that emits diagnostics in the same structured format already used by `zirk build`, so the same editor and CI consumers can parse output from either command.
- Ensure `zirk check` and `zirk build` share the same diagnostic rendering, codes, location formatting, and suppression rules through `zirk-diagnostics`. No new diagnostic structure is introduced; the existing `diagnostics-format` contract is preserved and extended only with the assurance that `zirk check` uses it.
- Update `scripts/check-local.sh` to run `zirk check` as the first, fastest gate (optional in this change, but the command must be available for it). The primary scope is the command itself, not the script.
- Add workspace-level tests that verify `zirk check` returns `0` for valid programs, non-zero for invalid programs, and does not invoke `zirk-ir` or `zirk-codegen-llvm`.
- Record, but do not implement, the relationship to LSP, formatter, and linter consumers as a follow-up note in the change's closeout, so the new spec is not overloaded with Phase 9 work.

## Capabilities

### New Capabilities
- `zirk-check`: Defines the `zirk check` command as a frontend-only validation pass that is independent of LLVM, the linker, the runtime, and backend code generation. It covers command-line interface, exit codes, scope of validation, and the rule that a frontend error must prevent any later stage from producing artifacts.

### Modified Capabilities
- `zirk-cli-commands`: Adds the `check` subcommand to the CLI surface and the requirement that `zirk check` exits cleanly for well-formed programs and with the same diagnostic output as `zirk build`.
- `compiler-workspace`: Adds the architectural rule that a `zirk check` build graph must not depend on `zirk-ir`, `zirk-codegen-llvm`, or `zirk-runtime`, and that the frontend crates remain buildable and testable in isolation from the backend.
- `toolchain-bootstrap`: Clarifies that `zirk check` is supported without `LLVM_SYS_201_PREFIX`, and that the toolchain's LLVM pin applies only to commands that emit native code (build, run, test, bench).

## Impact

- **Affected commands**: `zirk check` is added; `zirk build` and `zirk run` are not modified except to ensure they continue to behave identically.
- **Affected crates**: `zirk-cli` will gain a new entry point. `zirk-lexer`, `zirk-parser`, `zirk-ast`, and `zirk-sema` are reused without logic changes. `zirk-ir`, `zirk-codegen-llvm`, and `zirk-runtime` are explicitly excluded from the `zirk check` build graph.
- **Affected tests**: New CLI and workspace tests for the command and its isolation. No existing tests are removed.
- **Affected scripts**: `scripts/check-local.sh` may optionally use `zirk check` as a fast first gate, but this change does not require that edit.
- **No impact on**: grammar, language semantics, runtime, or public documentation outside the CLI/tooling surface.
- **Sibling repository**: `../zirk-lang-site` may need a small update if the CLI command list is exposed publicly; this will be decided after the command is implemented and any handbook reference is added in a follow-up change. The current change is intentionally not public-facing.
