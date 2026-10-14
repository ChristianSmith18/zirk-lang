## Why

Quoted imports are intended to load files from the current crate, but the loader currently accepts lexical `..` traversal and follows symlinks outside the crate. Compiling an untrusted project can therefore read and parse unrelated local source files, so path containment must be enforced before source access.

## What Changes

- Restrict quoted imports and module re-exports to the project root, or to the entry file's directory for single-file and overlay loading.
- Permit files in explicitly resolved package roots while keeping each package confined to its own root.
- Resolve existing files canonically and reject symlinks that escape the applicable root.
- Emit a dedicated diagnostic at the requesting import and document the behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-modules`: define root containment for local imports and re-exports, including symlink handling.

## Impact

The change affects `zirk-cli` module loading and diagnostics, module resolver tests, the `zirk-modules` OpenSpec contract, and compiler documentation. It changes behavior for projects that intentionally import files outside their declared project/package roots. The companion website repository is also impacted because it mirrors public compiler documentation; its public content must be synchronized after the compiler documentation is committed.
