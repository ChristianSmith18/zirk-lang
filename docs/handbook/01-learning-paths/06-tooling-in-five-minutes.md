# Tooling in Five Minutes

The Zirk toolchain is designed around a short feedback loop and one shared interpretation of source code.

## Create and inspect

```console
zirk new sample
cd sample
zirk check
```

`check` runs the incremental frontend—lexing, parsing, name resolution, typing, and flow analysis—without native code generation. Use it as the fastest semantic gate in editors and CI.

## Format and lint

```console
zirk format
zirk lint
```

The formatter is canonical and idempotent. It adds semicolons and normalizes layout without a large style-configuration surface. The linter shares compiler syntax and types; automatic fixes must be safe and reviewable.

## Run tests and benchmarks

```console
zirk test
zirk bench
```

Tests execute through the official runner and remain subject to declared test permissions. Benchmarks are measured separately so ordinary unit tests stay deterministic and quick.

## Build and run

```console
zirk run
zirk build
zirk build --target x86_64-linux
```

`run` compiles incrementally and starts an application. `build` produces artifacts for the selected profile and target. A CLI target overrides manifest `build_targets` for that invocation.

## Audit and package

```console
zirk prepare
zirk package
```

`prepare` audits permissions, targets, and publication concerns and may propose changes. `build` remains strict and never grants permissions interactively. `package` produces a `.zpkg` containing typed public API, portable IR, manifest data, documentation, and licensing material.

## Editor services

The language server uses incremental snapshots and supports diagnostics, completion, hover, navigation, references, rename, formatting, semantic tokens, signatures, and code actions. The debugger relies on source mappings that cover tasks and validated generated code as well as ordinary functions.

> **Implementation status:** This is the normative command surface. Check `zirk --help` for the commands available in your compiler revision.

---

**Previous:** [← Zirk for Java and C# Programmers](05-zirk-for-java-csharp-programmers.md) · **Next:** [ The Zirk Language Handbook](../02-handbook/README.md)
