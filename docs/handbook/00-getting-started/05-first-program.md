# Your First Zirk Program

This chapter builds the smallest complete Zirk application and explains every file that affects the result.

## Create the project

The normative CLI creates a project directory with `zirk new`:

```console
zirk new hello-zirk
cd hello-zirk
```

A minimal application has this shape:

```text
hello-zirk/
├── init.zrk
├── zirk.lock
├── src/
│   └── main.zrk
└── test/
```

`init.zrk` describes the project. `zirk.lock` records an exact dependency resolution and hashes; it is produced and maintained by dependency commands rather than edited as application source. `src/main.zrk` contains the entry function.

## Describe the application

Open `init.zrk` and verify the project block:

```zirk
project {
    name: "hello-zirk";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

Only applications define the final entry point and grant runtime permissions. Libraries expose an API and declare requirements; they do not decide what the consuming application may access.

## Write the entry point

Use the standard output API in `src/main.zrk`:

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

The import is explicit: `stdout` is not automatically global. `main` takes no arguments and returns `Void`, which corresponds to a successful process exit. Programs that need command-line arguments obtain them from the process APIs rather than receiving a mandatory parameter.

## Check, run, and build

Check semantics without producing native code:

```console
zirk check
```

Compile incrementally and execute:

```console
zirk run
```

Produce build artifacts without running them:

```console
zirk build
```

The exact output directory belongs to the build-profile and CLI contract. Do not make scripts depend on an undocumented cache path; use documented artifacts or structured CLI output.

## Make the program accept a value

The next version moves formatting into a typed function:

```zirk
import { stdout } from std.io;

fn greeting(name: String): String {
    return "Hello, {name}";
}

fn main(): Void {
    inmut name = "Zirk programmer";
    stdout.println(greeting(name));
}
```

The compiler infers `name` as `String`, while the function's public contract states its parameter and return type. `inmut` prevents rebinding `name`; use `mut` only when later reassignment is part of the design.

## A representative mistake

This call is invalid because `greeting` requires a `String`:

```zirk
stdout.println(greeting(42));
```

The compiler should report the argument location, explain that an integer is not assignable to `String`, and suggest a deliberate conversion if the API provides one. Zirk does not silently apply a potentially surprising conversion.

## Normative sources

- [Zirk Master Specification](../../ZIRK_SPEC_FINAL.md), §5
- [Zirk Language Specification](../../ZIRK_LANGUAGE_SPEC.md), functions and modules
- [Zirk Compiler and Tooling Specification](../../ZIRK_COMPILER_SPEC.md), CLI and diagnostics

---

**Previous:** [← Installation](04-installation.md) · **Next:** [ How Zirk Compiles](06-how-zirk-compiles.md)
