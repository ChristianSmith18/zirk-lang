## Why

The CLI currently requires the `.zrk` extension on every source argument. This is a small but unnecessary papercut: users naturally want to type `zirk run main` instead of `zirk run main.zrk`. Making the extension optional improves ergonomics without changing the language, the file format, or the compiler pipeline.

## What Changes

- Allow all `zirk` subcommands that accept a source path (`build`, `run`, `check`) to receive an argument with or without the `.zrk` extension.
- If the argument does not end in `.zrk`, the CLI appends `.zrk` internally before loading.
- If the argument already ends in `.zrk`, it is used exactly as given (no double extension).
- Apply the same resolution to the standalone `zirk-check` binary.
- Update help text and diagnostics that currently assume `.zrk` is mandatory.
- Update `zirk-cli-commands` spec to reflect the extension-agnostic path handling.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-cli-commands`: the source-path requirements for `build`, `run`, and `check` are relaxed to accept both `name` and `name.zrk`.

## Impact

- `crates/zirk-cli/src/main.rs`, `crates/zirk-cli/src/bin/zirk-check.rs`, and `crates/zirk-cli/src/frontend.rs`.
- Help text in `zirk --help`, `zirk-check --help`, and `zirk-cli-commands` spec.
- Tests in `crates/zirk-cli/tests/end_to_end.rs` and `crates/zirk-cli/tests/check.rs`.
- `zirk-lang-site` only if the public handbook or CLI reference is updated separately.
