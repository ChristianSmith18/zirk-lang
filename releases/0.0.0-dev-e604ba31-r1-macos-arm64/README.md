# Zirk 0.0.0-dev-e604ba31-r1 for macOS arm64

This is a local development release built from source revision `e604ba314c1e3961f0749c5477f38effb6a4d54f`. It is intended to exercise the per-user installation flow on Apple silicon. It is not a signed or notarized public release.

## Install

Run the installer from this directory:

```sh
./install.sh
```

The installer verifies the archive checksum, installs the version under `~/.local/share/zirk/versions/`, selects it through `~/.local/share/zirk/current`, and exposes `zirk` through `~/.local/bin/zirk`. It does not use `sudo` or modify shell startup files. Ensure `~/.local/bin` is on `PATH`; if needed, add this line to `~/.zshrc` and open a new terminal:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Verify with:

```sh
zirk --version
zirk --help
```

## What is installed

- `bin/zirk`: optimized arm64 compiler executable.
- `bin/libzirk_runtime.a`: static runtime matched to this exact compiler build. The compiler locates it beside itself.
- `lib/libzstd.1.dylib`: private copy of the compiler's non-system zstd dependency, loaded through an executable-relative Mach-O run path.
- `manifest.json`: release version, target, source revision, runtime pairing, and standard-library identity.
- `MANIFEST.sha256`: checksums of the compiler, runtime, and private dylib.

The currently implemented `std.io` package sources are embedded in the compiler when it is built; they are not loaded from the working tree or a separate system directory. The compiler's standard-library package resolver therefore remains available after installation.

## Native builds

Frontend-only commands such as `zirk check` do not need the Apple linker. Commands such as `zirk build` and `zirk run` need Apple's `clang` and macOS SDK, supplied by Xcode or the standalone Command Line Tools. Check the selected developer directory with `xcode-select -p`; install Apple's Command Line Tools with `xcode-select --install` if they are not available.

The installed compiler does not require Rust, Cargo, Homebrew, or a separate LLVM installation. The Apple SDK is needed only when linking native programs.

## Remove this version

First remove the stable command and active-version link, then remove the version directory:

```sh
rm "$HOME/.local/bin/zirk"
rm "$HOME/.local/share/zirk/current"
rm -rf "$HOME/.local/share/zirk/versions/0.0.0-dev-e604ba31-r1-macos-arm64"
```

These commands remove only this Zirk installation. If a later Zirk version is active, inspect the `current` link before removing shared Zirk paths.
