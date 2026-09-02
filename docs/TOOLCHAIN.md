# Zirk — Development Toolchain

Requirements to **build the Zirk compiler** from source, on the three supported platforms.

> This is different from the targets that Zirk **produces**. See [ADR-004](./decisions/ADR-004-portabilidad.md).

## Components

| Component | Version | Reason |
|---|---|---|
| Rust | 1.94+ (stable edition) | compiler language |
| LLVM | **20.1.x** (exact major-version pin) | codegen backend |
| lld | 20.1.x | cross-platform linker (ELF / Mach-O / COFF) |
| inkwell | 0.10 with feature `llvm20-1` | safe bindings to LLVM |

The LLVM pin **is not negotiable for local convenience**: `llvm-sys` links against the C++ ABI of a specific major version. A different version will not compile. See [ADR-001](./decisions/ADR-001-pin-llvm.md).

### Frontend-only validation without LLVM

`zirk check` validates the frontend (lexing, parsing, name resolution, type and flow checking) and does not generate native code. It can be built without an LLVM installation:

```sh
cargo build -p zirk-cli --bin zirk-check --no-default-features
```

The `zirk check` subcommand of the full `zirk` binary is also available when `LLVM_SYS_201_PREFIX` is unset, because the frontend code does not depend on the LLVM backend. Only `zirk build`, `zirk run`, and `--list-targets` require the pinned LLVM toolchain.

## Required environment variable

`llvm-sys` locates LLVM via `llvm-config` on the `PATH` or, preferably, via:

```
LLVM_SYS_201_PREFIX=<prefix of the LLVM 20.1 installation>
```

The variable is used instead of the `PATH` because, in most installations, LLVM 20 coexists with other versions and is not linked globally.

---

## macOS

```sh
brew install llvm@20 lld@20
```

`llvm@20` is *keg-only* (not linked into `/opt/homebrew/bin`), so the variable is required:

```sh
# Apple Silicon
export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20

# Intel
export LLVM_SYS_201_PREFIX=/usr/local/opt/llvm@20
```

Requires Xcode Command Line Tools for the system SDKs.

## Linux (Debian / Ubuntu)

The official `apt.llvm.org` repository publishes the static libraries in `llvm-20-dev`:

```sh
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
sudo ./llvm.sh 20
sudo apt-get install -y llvm-20-dev libpolly-20-dev lld-20

export LLVM_SYS_201_PREFIX=/usr/lib/llvm-20
```

On other distros, the package you need is the one that contains `llvm-config` **and** the `.a` files (usually `llvm-devel` or `llvm-static`), not just the clang binaries.

## Windows

> **The official LLVM distribution does not work.** Neither the `.exe` installer — which does not ship static libraries — nor the `clang+llvm-*-pc-windows-msvc.tar.xz` development tarball, which does ship them but is built against the **static** CRT while Rust uses the dynamic one. Mixing them puts two heaps in the same process, and the compiler aborts with `STATUS_ACCESS_VIOLATION` on the first LLVM call that returns a string.
>
> All four possible combinations were tested and none of them work. Details are in issue [#2](https://github.com/ChristianSmith18/zirk-lang/issues/2).

A build maintained specifically so that `inkwell` works on Windows is used, compiled with the matching CRT:

```
https://github.com/TyrsDev/llvm-package-windows/releases/tag/v20.1.8

  LLVM-20.1.8-win64.7z
```

Extract to `C:\LLVM` and set the variable with **forward slashes**:

```powershell
setx LLVM_SYS_201_PREFIX C:/LLVM
```

Requires **Visual Studio 2022** with the C++ tools (or equivalent Build Tools), because `llvm-sys` links against the MSVC runtime.

> This dependency comes from a single maintainer and constitutes a consciously accepted supply-chain risk. The alternative is compiling LLVM from source with `LLVM_USE_CRT_RELEASE=MD` and `LLVM_ENABLE_LIBXML2=OFF`, which takes hours per build.

---

## Verification

```sh
rustc --version                 # 1.94+
$LLVM_SYS_201_PREFIX/bin/llvm-config --version   # 20.1.x
```

The real check is the workspace test suite, which includes the LLVM sanity check (generates IR, emits an object, links, and runs a native binary):

```sh
cargo test --workspace
```

If the LLVM pin is wrong, the build fails before compiling, with a diagnostic naming the version found and the version expected. There is no need to interpret linker errors.

## Platform notes

- **Windows:** the prefix is declared with forward slashes (`C:/LLVM`, not `C:\LLVM`). This avoids escaping issues when passing through bash-like shells, and Windows APIs accept both separators.
- **macOS:** `llvm@20` and `lld@20` have been separate formulae since LLVM 20; installing only `llvm@20` leaves the system without `lld`.
- **Linux:** the required package is `llvm-20-dev`, not `llvm-20`. The former includes the static libraries; the latter only the binaries.

Per-platform installation is automated in `.github/workflows/ci.yml`, which is the executable reference for this document.
</content>
