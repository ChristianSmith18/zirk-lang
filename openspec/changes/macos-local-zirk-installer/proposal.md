## Why

Zirk currently has no installable macOS toolchain artifact, so users must invoke a build-tree executable through a machine-specific shell alias. A versioned per-user installation is needed to verify that the compiler, bundled standard sources, runtime archive, dynamic dependencies, and native linker work together outside the repository without requiring Rust, Cargo, Homebrew, or a full LLVM installation from the user.

## What Changes

- Add a versioned, arm64 macOS developer release bundle and an English installer under `releases/`.
- Add `zirk uninstall` to remove the managed per-user command link and installed Zirk versions without deleting repository release artifacts.
- Install Zirk without administrator privileges under `~/.local/share/zirk`, expose `zirk` under `~/.local/bin`, preserve the compiler/runtime pairing, and print clear progress, prerequisite, success, and recovery messages.
- Bundle or relocate non-system dynamic dependencies so the compiler does not depend on this machine's Homebrew paths.
- Detect Apple Command Line Tools and macOS SDK availability before native builds; keep frontend-only checking usable without them.
- Record the package version, target, source revision, runtime pairing, standard-library identity, and payload hashes in a manifest.
- Keep this as a local development release; public publishing, a general multi-version manager, automatic project-version switching, Intel/universal artifacts, and a GUI `.pkg` remain out of scope.
- Do not modify shell startup files automatically. The current user's `zirk` alias will be commented only for the requested installation verification.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `toolchain-bootstrap`: define the contents, installation, prerequisite checks, and verification contract for a self-contained per-user macOS compiler distribution.

## Impact

- Adds a macOS arm64 package and installer below `releases/`.
- Affects compiler runtime discovery or package layout only as required to keep `zirk` and `libzirk_runtime.a` paired after installation.
- Updates toolchain installation guidance only if the local release changes a user-facing supported workflow; this development artifact is not an official public release and does not change the website status catalog.
- The verification installs files under the current user's `~/.local` and comments the single existing `alias zirk` line in `~/.zshrc` as explicitly requested.
