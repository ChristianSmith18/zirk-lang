## 1. Release Bundle

- [x] 1.1 Build the optimized macOS arm64 compiler and matching runtime archive, and verify both artifacts are present
- [x] 1.2 Relocate the compiler's private dynamic dependency into the release bundle and verify `otool -L` contains no machine-specific Homebrew paths
- [x] 1.3 Create a versioned release archive and manifest with source revision, target, runtime pairing, stdlib identity, and SHA-256 hashes; verify the manifest against the built payload

## 2. Installer

- [x] 2.1 Implement an English macOS arm64 installer that validates platform and archive checksum, stages a per-user version, and activates it without modifying shell startup files; verify unsupported targets and invalid checksums fail before activation
- [x] 2.2 Expose the active version through `~/.local/bin/zirk` and `~/.local/share/zirk/current`; verify both links resolve to the same compiler/runtime bundle
- [x] 2.3 Document bundle contents, Apple native-build prerequisites, PATH setup, installation output, and precise removal commands in English
- [x] 2.4 Add `zirk uninstall` to safely remove managed per-user command/version links and all recognized installed versions; verify it refuses unrelated invocations and preserves release artifacts

## 3. Installed Toolchain Verification

- [x] 3.1 With the repository alias disabled, install the release and verify a fresh shell resolves `zirk` from `~/.local/bin` and prints the expected version
- [x] 3.2 From a temporary directory and sanitized environment, verify `zirk check` and a native `zirk run` using the installed standard library and adjacent runtime
- [x] 3.3 Inspect the installed Mach-O dependencies and report exactly what the offline installation and native-build checks prove
- [x] 3.4 Run `zirk uninstall` from the installed release, verify managed paths are gone and repository releases remain, then reinstall and repeat the installed compiler smoke check
