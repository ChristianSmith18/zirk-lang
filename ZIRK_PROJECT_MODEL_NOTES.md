# Zirk project model and execution entry — design notes

**Status:** historical discussion checkpoint; the final contract is recorded below and in the linked OpenSpec/handbook sources.

**Date:** 2026-10-25

## Goals

- Keep project metadata declarative and useful to tooling/publication.
- Let the user choose the executed file from the CLI or let the project select it in `.zkinit`.
- Remove the mandatory source-level `fn main` convention. The selected file executes its top-level statements in order.
- Make imported modules' top-level initialization predictable and execute each module at most once per process.

## Project metadata

The `project` object describes package/application identity and optional publication metadata:

```text
project {
    name: "acme.api";
    version: "1.2.0";
    type: application;
    authors: ["Acme Engineering"];
    description: "HTTP API for Acme services.";
    license: "MIT";
    entry: "src/main.zrk";
}
```

Fields:

- `name`, `version`, and `type` identify the project. `type` is `application` or `package`.
- `authors`, `description`, and `license` are optional metadata. `authors` is a list to support multiple contributors; `license` should use an SPDX identifier/expression.
- Applications use `entry` to select the source file whose top-level body starts the application.
- Packages use `root` to select their importable package-root source module; a package is not itself an executable application.

Example package metadata:

```text
project {
    name: "acme.http";
    version: "2.1.0";
    type: package;
    authors: ["Acme Engineering"];
    description: "HTTP client and server primitives.";
    license: "Apache-2.0";
    root: "src/package.zrk";
}
```

The `entry`/`root` split avoids making a library appear to have a file that the runtime executes. Optional-field grammar, metadata validation, and whether the default paths remain `src/main.zrk` for applications and `src/package.zrk` for packages remain to be finalized. Those are the defaults in the current parser when `entry` is omitted.

## CLI entry selection

### Explicit source path

```sh
zirk run src/dev.zrk
```

The supplied file becomes the execution entry for this invocation. If it is inside a project, the nearest `.zkinit`, lockfile, dependencies, globals, and requirements still provide project context; the explicit file overrides only the application's configured `entry`. If no project manifest applies, it runs as a standalone source entry subject to the standalone import/runtime rules.

### No source path

```sh
zirk run
```

The CLI discovers the project `.zkinit`, requires its type to be `application`, and executes its `entry` file. Running without an explicit path from a package project is an error with a diagnostic explaining that a package has no application entry.

## Source execution model

`.zrk` files may contain declarations and executable top-level statements. The selected application entry file evaluates its top-level statements sequentially. Function declarations define callable functions; they are not invoked merely because they appear in the file. `fn main` is an ordinary function name, not a language-required hook.

The compiler/runtime still has an internal native process entry (such as the platform ABI `main`) to initialize the runtime, execute Zirk, and shut down. That implementation detail is not a source-language entrypoint requirement.

## Import and module evaluation

### Current import behavior

Imports are currently static declarations. Quoted local imports resolve relative to the importing file and append `.zrk`; they must remain within the authorized project/package root after canonical and symlink checks. Unquoted logical package imports resolve through `.zkinit`, `zirk.lock`, and bundled standard-package roots. `share` controls visibility, and `share mod` publishes package-root module namespaces.

The loader currently reads only the graph reachable from the selected file, visits each file once, and permits mutual imports. The semantic checker gathers declarations from the whole loaded graph before resolving names. Current source files do not support executable top-level statements, and imported files have no runtime initialization behavior.

### Proposed behavior

1. The compiler discovers the selected entry, loads and parses the complete static import graph, validates all imports and types, and produces diagnostics before any program code executes.
2. At runtime, Zirk initializes the runtime and application globals.
3. Zirk evaluates each imported module's top-level executable statements before continuing with the importing module. Dependencies initialize before their importer; imports are considered in source order.
4. A module is evaluated once per process, keyed by canonical package/module identity. A diamond-shaped graph does not repeat initialization.
5. After dependencies are evaluated, the application entry's top-level statements run in order. The process completes after that body and its structured work/resources finish according to the runtime lifecycle.

`import` remains a static declaration: the graph is known and checked before execution. Its runtime effect is to ensure the imported module has been initialized before the importer continues. `share`, imports, and logical package identity retain their existing roles.

This model intentionally means importing a module can cause top-level side effects. Library authors should keep module initialization small and put reusable behavior in functions that consumers call explicitly.

## Cycles and initialization failures

The current compiler supports declaration cycles because it collects signatures before checking bodies. Top-level execution introduces a separate initialization-order problem. Recommended first-version rule: keep declaration-only import cycles if supported, but diagnose and reject any strongly connected import group whose cycle would require running executable initialization in a partially initialized module. Do not silently observe partially initialized globals or skip a module body.

A module initialization failure prevents its importer and the application entry from continuing. Errors explicitly handled as values inside top-level code can continue normally; an uncaught failure produces a startup diagnostic and nonzero process status. Exact diagnostics and exit-code mapping remain to be specified.

## Recommended source organization

For deterministic evaluation, require static imports to appear before executable top-level statements in each file. Declarations may remain available to the compiler across the full graph, while execution order is easy to read:

```zirk
import { configure } from "./config";
import { start } from acme.server;

inmut settings = configure();
start(settings);
```

The entry file needs no `fn main`; imported module initialization occurs once before this body.

## Decisions still open before a normative proposal

- Final `.zkinit` grammar, including the package `root` field and optional metadata syntax.
- Default entry/root paths and whether an application can omit `entry`.
- Whether imports must be textually before top-level statements or whether all static imports are hoisted for initialization.
- Exact algorithm for source-order depth-first evaluation and permitted cycle shapes.
- Whether a standalone file may import only standard/local modules or may discover project dependencies from an ancestor manifest.
- Return/exit-code behavior for normal completion and uncaught top-level errors.
- Whether command-line arguments are exposed to the entry body and through which API.
- Runtime behavior for partial module initialization and diagnostics when an initializer fails.

## Current repository status

The current `ProjectManifest` stores a file-path `entry` for both applications and packages. Its parser accepts only part of the project metadata; the current working-tree parser change validates a `version` field but does not store it. Author/description/license metadata and a distinct package `root` are not implemented. The language server/compiler loader treats imports as a static declaration graph. Semantic analysis and LLVM code generation require the source function `fn main`, and the native wrapper uses it to run the runtime lifecycle. Sequential top-level execution and once-only imported-module initialization are not implemented.

Relevant current references: `crates/zirk-cli/src/project.rs`, `crates/zirk-cli/src/modules.rs`, `crates/zirk-parser/src/parser.rs`, `crates/zirk-sema/src/checker.rs`, `crates/zirk-codegen-llvm/src/emit.rs`, and `docs/ZIRK_RUNTIME_SPEC.md`.

## Finalized model

This discussion checkpoint is superseded by `openspec/changes/complete-zkinit-project-runtime/specs/zirk-project-model/spec.md` and the [project handbook](docs/handbook/03-projects/02-zkinit.md). `.zkinit` uses a typed `project` object with name/version/type and optional authors/description/license. Applications select `entry` (default `src/main.zrk`); packages select importable `root` (default `src/package.zrk`). Source roots default to `src`; only applications declare build targets. `zirk run <path>` selects an invocation-specific file while retaining project context; pathless `zirk run` discovers the application entry. The selected file executes file-scope statements; `fn main` is an ordinary function. Imports are static, precede executable statements, and initialize once depth-first in source order. `zirk init` prompts through project fields; `-y` creates canonical defaults without a TTY or source files.
