# Zirk 0.0.0-dev-8636ab7e-r5 for macOS arm64

This is a local development release built from source revision `8636ab7ef5519f71239c8154138be7499d50ac92`. It is intended to exercise the per-user installation flow on Apple silicon. It is not a signed or notarized public release.

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
- `bin/libzirk_runtime.a`: static runtime matched to this exact compiler build.
- `lib/libzstd.1.dylib`: private copy of the compiler's non-system zstd dependency, loaded through an executable-relative Mach-O run path.
- `manifest.json` and `MANIFEST.sha256`: release identity and payload checksums.

The standard library sources used by the compiler are embedded in its executable. Native `zirk run` and `zirk build` use Apple's `clang` and macOS SDK.

## Uninstall

Run `zirk uninstall` from the installed toolchain. It removes managed Zirk command/version links while leaving release archives and unrelated files untouched.
