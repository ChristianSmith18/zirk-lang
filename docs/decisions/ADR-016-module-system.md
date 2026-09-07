# ADR-016 — Module, Import and Entrypoint System

- **Status:** accepted
- **Date:** September 7, 2026
- **Phase:** 6

## Context

Zirk currently compiles and runs a single `.zrk` file that must contain `fn main(): Void`. Phase 6 needs a real project system: multiple source files, imports, libraries, a project manifest, and a clear entrypoint model.

The language already has `share`, `import` and `use` in the grammar, and `ZIRK_LANGUAGE_SPEC.md` section 10 describes an `init.zrk` manifest. This ADR tightens those ideas, introduces `.zkinit` as the project manifest, and defines the script-vs-project execution modes.

## Decision

Every `.zrk` file is a **module** — a unit of compilation. Modules declare `class`, `record`, `enum`, `interface`, `trait`, `type` aliases and `fn`. Only symbols marked `share` are visible to importers. Imports are direct, non-transitive, and idempotent: a module loaded by several others is loaded exactly once.

Execution is split into two modes:

- **Project mode**: `zirk run` / `zirk build` read `.zkinit`, locate the entrypoint, and run its `fn main`.
- **Script mode**: `zirk run archivo.zrk` executes a single file. If it has `fn main`, that function runs; otherwise the top-level statements run in order.

## Detailed design

### 1. `.zkinit` — project manifest

`.zkinit` is a declarative DSL, not executable code. It replaces the previous `init.zrk` manifest. If `.zkinit` is missing and `init.zrk` exists, the CLI may read it with a deprecation warning.

Example:

```text
project "mi-app"
    entrypoint "src/main.zrk"
    build "release"
    paths {
        "@app" -> "./src",
        "@core" -> "./lib/core"
    }
    globals { }
    permissions { }
    requires { }
```

- `entrypoint` defaults to `main.zrk` if omitted.
- `paths` defines aliases for absolute-style imports. `import { User } from "@app/domain/user"` resolves through the alias.

### 2. Entrypoint and `main` signature

The entrypoint must declare a `main` function. The following signatures are valid:

```zirk
fn main(): Void
fn main(args: List<String>): Void
fn main(): Int32
fn main(args: List<String>): Int32
```

- `args` is optional. When present, the CLI passes command-line arguments as a `List<String>`.
- If the return type is `Int32`, the value becomes the process exit code. `Void` is equivalent to returning `0`.

### 3. Script mode and top-level statements

When `zirk run archivo.zrk` targets a single file:

- If the file declares `fn main`, `main` runs and top-level statements are ignored with a warning.
- If the file has no `fn main`, its top-level statements execute in textual order.

Top-level statements in modules imported by a project or script are **ignored**: they are parsed but not emitted. This lets the same file behave as a standalone script and as a reusable library. The warning can be suppressed with `--ignore-dead-code`.

### 4. Import forms and visibility

Three `import` forms are supported:

```zirk
// Open import: every `share` symbol enters the current scope.
import "./utils";

// Namespace import: every `share` symbol is available under `utils`.
import utils from "./utils";

// Selective import with per-symbol aliasing.
import { User, Role -> DomainRole } from "./domain/user";
```

- `share` behaves like JavaScript `export` and can prefix `class`, `record`, `enum`, `fn`, `type`, `interface` and `trait`.
- Un-imported symbols are private to the file.
- Open imports that cause name collisions are compile-time errors.
- Importing a file that exports nothing emits a warning.

### 5. Path resolution

Relative paths use double quotes and omit `.zrk`:

```zirk
import { Helper } from "./helpers/helper";
import { Service } from "../core";
```

Resolution order for `"./foo"`:

1. Use `./foo.zrk` if it exists.
2. Otherwise use `./foo/index.zrk` if it exists.
3. Otherwise emit an error.

Absolute-style paths like `"@app/..."` are resolved through aliases declared in `.zkinit`. Bare paths such as `std.io` are reserved for compiler-known standard library modules.

### 6. Module loading and cycles

A module imported by several files is loaded once. All importers refer to the same unit, so `static` fields and functions are not duplicated.

- **Import cycles of declarations are allowed**. Two modules can import each other as long as the dependency is only at the declaration/type level.
- **Self-import is an error**.
- **Static initialization cycles are errors**. If `A.value` is initialized from `B.value` and `B.value` from `A.value`, the compiler rejects the program. For cycles that cannot be detected statically, runtime raises `StaticInitializationError`.

`static` fields are initialized lazily on first use of the class, following the dependency graph.

### 7. Tree-shaking

Build emits only symbols reachable from the entrypoint through imports and code references. A class is included if:

- it is referenced directly, or
- one of its `static` fields is referenced by an included class.

`static` initializer side effects are preserved when the class is reachable; otherwise the class is omitted.

### 8. Standard library

Standard library modules are imported through bare identifiers:

```zirk
import { stdout } from std.io;
```

`std` is a compiler-known root. User modules cannot declare `std.*` paths.

## Consequences

- `init.zrk` is deprecated in favor of `.zkinit`.
- The parser and `zirk-cli` must distinguish project mode from script mode.
- `Span` already carries a `FileId` (ADR-010), so multi-file diagnostics are supported.
- `share` already parses in the grammar for top-level declarations; the checker only needs to enforce visibility across files.
- Static cycle detection and lazy initialization require new IR/runtime support.
- Tree-shaking changes how the linker decides what to include, but it is purely an optimization and must not alter observable behavior.
