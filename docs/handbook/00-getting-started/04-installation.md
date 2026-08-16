# Installation

Zirk's official distribution is centered on one command-line executable, `zirk`. That command owns project creation, checking, builds, execution, formatting, linting, tests, dependency management, packaging, publishing, and documentation generation.

> **Implementation status:** The command surface is normative, but public binary distribution channels are still implementation-dependent. Prefer installation instructions published with the exact compiler revision you intend to use.

## What an installation needs

A usable development installation must provide the Zirk frontend and its command-line tools. Native builds additionally depend on a supported LLVM toolchain, linker, and—in platform-specific cases—an operating-system SDK. Cross-compilation requires the complete target toolchain, not merely a target name.

The compiler specification intentionally does not mandate an unsafe universal shell installer. Official releases should publish platform-specific archives or packages with versions and integrity information. Building the repository from source is appropriate for compiler contributors, but it is not equivalent to a stable release install.

## Verify the command

After following the instructions for your release, open a new terminal and ask the executable for help:

```console
zirk --help
```

Then check which compiler revision is active:

```console
zirk --version
```

The command must be discoverable through your shell's `PATH`. If the shell reports that `zirk` is not found, confirm where the executable was installed and whether a newly opened terminal inherits the updated environment.

## Verify native build prerequisites

Create a disposable project and run the checker before attempting a full native build:

```console
zirk new hello-zirk
cd hello-zirk
zirk check
zirk run
```

`zirk check` exercises parsing, name resolution, typing, and flow analysis without invoking LLVM. If checking succeeds but building fails, inspect the diagnostic for a missing linker, SDK, or native dependency. Zirk diagnostics should identify the blocked target and recommend a concrete correction.

## Selecting a target

Without a CLI target or `build_targets` entry, the toolchain detects the host. An explicit target overrides the manifest temporarily:

```console
zirk build --target x86_64-linux
```

Target support depends on the host, LLVM, linker, SDK, and every native dependency. Do not assume that installation of the compiler alone makes every cross-target combination available.

## Updating safely

Treat the compiler version as a build input. Teams should agree on a version and record it in development and CI setup. Dependency reproducibility comes from `zirk.lock`; it does not pin the compiler itself. Before updating, read compatibility notes and run `zirk check`, tests, and representative release builds.

## Troubleshooting checklist

- Confirm `zirk --version` resolves the expected executable.
- Run `zirk check` to separate frontend problems from backend or linker problems.
- Verify the selected target and required SDK.
- Inspect native dependencies for target restrictions.
- Avoid granting additional runtime or compile permissions merely to work around an installation error.

## Normative source

[Zirk Compiler and Tooling Specification](../../ZIRK_COMPILER_SPEC.md), especially targets and the official CLI.

---

**Previous:** [← Language and Implementation Status](03-language-status.md) · **Next:** [ Your First Zirk Program](05-first-program.md)
