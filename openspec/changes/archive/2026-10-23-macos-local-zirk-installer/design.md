## Context

See `proposal.md` for motivation. The current CLI locates `libzirk_runtime.a` beside its executable and links generated programs through a host `clang`. The development binary also has a Homebrew zstd load command, so copying the executable alone is not relocatable. The std.io source is embedded in the compiler binary today. Native output on macOS still depends on Apple's linker and SDK.

## Goals / Non-Goals

**Goals:**

- Prove a release-shaped installation in the current user's `~/.local` hierarchy.
- Keep the compiler, static runtime, and private dylib dependency together in one relocatable version directory.
- Make the package contents, prerequisites, install steps, and removal steps understandable from English release documentation and installer output.
- Keep the first artifact explicitly local and development-only.

**Non-Goals:**

- Define a public release channel, signatures/notarization, automatic updates, or a general-purpose version manager.
- Build Intel or universal macOS packages.
- Change language-level import semantics or move the embedded standard library to dynamic loading.

## Decisions

### Install into a versioned user-local sysroot

Place immutable releases under `~/.local/share/zirk/versions/<version>-macos-arm64`, with a `current` symlink selecting the active version. Place the stable `~/.local/bin/zirk` symlink at the front door. This follows the user's preference for user-local installation and leaves room for explicit version switching later without requiring it now. A global `/usr/local` or `/opt` installer would require elevated privileges and broaden ownership concerns.

### Bundle the compiler, adjacent runtime, and private dylib

Package `bin/zirk`, `bin/libzirk_runtime.a`, and `lib/libzstd.1.dylib`. Rewrite the compiler's zstd load path to an `@rpath` reference and add an executable-relative runtime search path. Keep the runtime archive adjacent to the compiler because the current driver searches beside its own executable. The archive also includes a manifest and SHA-256 checksum. The standard library remains embedded in this compiler build; the manifest records its source identity rather than suggesting that the binary loads a separate stdlib directory.

### Use a transparent shell installer, not a system package

Provide a versioned `install.sh` alongside the archive in `releases/`. It checks macOS/arm64, validates the archive checksum, stages and activates the exact release, creates local symlinks, and runs a version check. It prints descriptive English progress and recovery messages. It does not mutate `.zshrc`, `.zprofile`, or `.bashrc`; it explains how to expose `~/.local/bin` if needed. No package manager, Rust, Cargo, Homebrew, or LLVM installation is required for end users of the bundle.

### Let the installed compiler remove its managed user installation

Implement `zirk uninstall` in the CLI. Before removing anything, it resolves its own executable path, confirms it runs from a version directory under `~/.local/share/zirk/versions`, confirms the active-version link targets a recognized Zirk manifest, and confirms `~/.local/bin/zirk` is the expected managed symlink. It then removes only the managed command/current links and version directories whose manifests identify them as Zirk bundles. This lets the tool uninstall all of its installed versions while preserving release archives in the source repository and refusing unrelated launch paths.

### Treat Apple tools as the native-output boundary

The bundle can run compiler frontend operations without LLVM installation because the LLVM backend is built into the binary. Linking generated native programs needs Apple's `clang` and macOS SDK. The compiler should use the host `clang` when no development-only LLVM override is configured, and diagnostics should identify missing Apple tools instead of failing with an opaque linker error.

## Risks / Trade-offs

- **Ad-hoc local bundle is not a distributable signed release** → label it as a development artifact; public distribution must add signing, notarization, and a reviewed dependency/license inventory.
- **`~/.local/bin` may not be on `PATH` in every shell** → do not edit startup files; print the exact manual setup and verify through a fresh shell after the requested alias is disabled.
- **Compiler and runtime ABI can drift** → install both from one archive under one version directory and switch only the `current` symlink.
- **Mach-O relocation can leave hidden absolute dependencies** → inspect the installed binary with `otool -L` and run it with a sanitized environment outside the checkout.
- **Full Xcode and standalone Command Line Tools differ in installation state** → check for an available Apple compiler and SDK at build time and report the concrete missing prerequisite.
