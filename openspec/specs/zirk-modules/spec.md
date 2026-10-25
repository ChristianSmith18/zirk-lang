# zirk-modules

## Purpose

Defines how the files of one crate see each other: `share`, `import` and `use`.

Which files make up a crate is decided by walking `import` from the entry file, so a `.zrk` nobody imports is not part of the program. Visibility is binary here — shared or private to its file. The three levels of `ZIRK_LANGUAGE_SPEC.md` section 7 depend on classes and arrive with them, and per-module namespacing belongs with the project system.
## Requirements
### Requirement: Binary visibility between files of a crate

A top-level declaration SHALL be visible only within the file that defines it, unless it is marked `share`, in which case it SHALL be visible from any other file of the same crate that imports it.

This phase does not implement the three levels of `public`/`private`/`protected` from `ZIRK_LANGUAGE_SPEC.md` section 7 — that depends on classes and is Phase 3. Visibility here is binary: shared or private to the file.

#### Scenario: Declaration private to the file
- **WHEN** a function without `share` is referenced from another file of the same crate
- **THEN** a diagnostic indicating that the declaration is not accessible from outside its file is emitted

#### Scenario: Shared declaration
- **WHEN** a function marked `share` is imported from another file
- **THEN** the reference resolves to that declaration

### Requirement: `import` resolution with local paths

`import { names } from "path"` with a quoted path SHALL resolve to a file of the same crate, located relative to the importing file, without the `.zrk` extension in the written path.

#### Scenario: Relative path resolved
- **WHEN** `import { User } from "./domain/user";` is written in a given file
- **THEN** it resolves to the file `domain/user.zrk` relative to that file

#### Scenario: Nonexistent file
- **WHEN** the path of an `import` does not correspond to any file of the crate
- **THEN** a diagnostic naming the path not found is emitted
- **AND** it points to the `import` that requested it, not the start of the file

#### Scenario: Name not shared in the destination file
- **WHEN** a name that exists in the destination file but is not marked `share` is imported
- **THEN** the same visibility diagnostic as a direct reference is emitted

### Requirement: Import alias

`import` SHALL support renaming an imported name with the syntax `name -> alias`, and the resolved name SHALL be used under the alias within the importing file.

#### Scenario: Import with alias
- **WHEN** `import { Role -> DomainRolee } from "./domain/user";` is written
- **THEN** within the file, `DomainRolee` resolves to the `Role` declaration of the imported file
- **AND** the unaliased name `Role` remains unavailable in the importing file

### Requirement: Mutual imports

Static module loading SHALL support declaration cycles without reading a source file more than once. Runtime initialization SHALL use canonical package/module identity to evaluate each reachable imported module at most once per process. Imports SHALL be considered in source order; the loader SHALL evaluate each imported module and its dependencies before continuing with the importing module. A strongly connected import component that contains executable top-level initialization SHALL be rejected before program code runs; declaration-only cycles MAY remain valid.

#### Scenario: Direct cycle
- **WHEN** file A imports from file B and file B imports from file A
- **THEN** the crate compiles
- **AND** each file is read exactly once

#### Scenario: Cycle with executable initialization
- **WHEN** a strongly connected import component contains top-level executable statements
- **THEN** compilation reports the complete initialization cycle before any module body executes

### Requirement: Imported modules initialize once before their importers

After the complete static import graph has been loaded and checked, the runtime SHALL initialize each imported module before executing the importing module's remaining top-level statements. Initialization SHALL follow deterministic depth-first source order and SHALL be keyed by canonical package/module identity so re-exports and repeated imports do not duplicate execution. Static imports SHALL appear before executable top-level statements in a file; a violation SHALL produce a source diagnostic.

#### Scenario: Import initialization precedes importer statements
- **WHEN** module A imports module B and both contain top-level statements
- **THEN** B's top-level statements execute before A's statements

#### Scenario: Re-export does not repeat initialization
- **WHEN** two package-root paths re-export the same canonical source module
- **THEN** that module's top-level statements execute once for the process

#### Scenario: All static errors precede runtime effects
- **WHEN** the import graph contains a missing module, type error, or invalid permission requirement
- **THEN** the compiler reports it before executing any module or application top-level statement

### Requirement: Module initialization failures stop dependent startup

An uncaught failure during module initialization SHALL prevent its importer and the application entry from executing. A failure explicitly handled as a value or exception inside module top-level code MAY continue according to the language's existing error semantics. The CLI SHALL report the failing module and dependency path and return a non-zero process status for an unhandled startup failure.

#### Scenario: Imported module fails during initialization
- **WHEN** a module's top-level operation raises an uncaught error
- **THEN** its importer and application entry do not execute and the diagnostic identifies the module

#### Scenario: Module handles its own initialization error
- **WHEN** top-level module code explicitly handles an operation error
- **THEN** the module may complete initialization and importer execution may continue

### Requirement: Collision of shared names

Module resolution SHALL reject two `share` declarations with the same name within the same crate, with no implicit tiebreak by file order.

#### Scenario: Two files share the same name
- **WHEN** two distinct files of the crate declare `share fn help(): Void {}` with the same name
- **THEN** a diagnostic pointing to both declarations is emitted

### Requirement: `use` enables globals without a qualified name

`use` SHALL enable unqualified access to the globals of an already-imported module, without bringing new names into scope that were not first imported with `import`.

#### Scenario: `use` over an existing import
- **WHEN** a file imports `stdout` from `std.io` and then writes `use stdout;`
- **THEN** `println(...)` without the `stdout.` qualifier resolves to the same declaration

#### Scenario: `use` without a prior `import`
- **WHEN** `use` is written over a name the file did not import
- **THEN** a diagnostic indicating that the name is not available in this file is emitted

### Requirement: Standard modules without quotes

`import { names } from standard.module;` with an unquoted name SHALL resolve
against compiler-provided standard packages and their published modules. The
same package-path grammar SHALL be available to declared external packages;
standard-library ownership is determined by the resolver, not by quoted source
paths.

#### Scenario: Importing a standard package module
- **WHEN** `import { stdin, stdout } from std.io;` is written
- **THEN** the resolver loads the public `io` module from the compiler-provided `std` package
- **AND** both names resolve to its published modules

#### Scenario: Importing an external package module
- **WHEN** `import { Controller } from acme.web;` is written and `acme.web` is declared by the project
- **THEN** the resolver loads the package's public root without quotes

#### Scenario: Importing `std.io`
- **WHEN** `import { stdout, stderr } from std.io;` is written
- **THEN** both names resolve to the published standard package modules

#### Scenario: Unrecognized standard module
- **WHEN** something is imported from a standard package module that the toolchain does not provide
- **THEN** a diagnostic indicating that the module is unavailable is emitted

#### Scenario: Quoted package-like name is not a package import
- **WHEN** `import { Controller } from "acme.web";` is written
- **THEN** the resolver treats it as a relative source path and emits a path diagnostic if no such local file exists

### Requirement: Standard-library convenience member resolution

Importing a module SHALL make its published members callable without
qualification when each unqualified name is otherwise unambiguous. A caller
MAY always select the member explicitly with `module.member`. The same default
opening applies to imported external package modules; local modules imported by
relative path SHALL follow the same lexical rule once they are represented as
modules. A collision SHALL require qualification.

#### Scenario: Direct standard output call
- **WHEN** a file imports `{ stdout } from std.io` and has no competing `println`
- **THEN** `println("hello")` resolves to `stdout.println("hello")`

#### Scenario: Explicit module member
- **WHEN** a file imports `{ stdout } from std.io`
- **THEN** `stdout.println("hello")` resolves to the same published member without relying on unqualified opening

#### Scenario: Ambiguous convenience name
- **WHEN** the file also declares or imports another `println`
- **THEN** an unqualified call is diagnosed as ambiguous
- **AND** `stdout.println(...)` remains valid

#### Scenario: External module members open by default
- **WHEN** a file imports `{ terminal } from acme.cli` and `terminal` publishes `clear()`
- **THEN** `clear()` and `terminal.clear()` both resolve when no collision exists

#### Scenario: Explicit member import
- **WHEN** `import { println } from std.io::stdout;` is written
- **THEN** only the selected published member is bound under `println`
- **AND** its owning module remains available only when separately imported

#### Scenario: Package object import
- **WHEN** an object is imported from an external package module
- **THEN** its published methods are opened by default when unambiguous
- **AND** `module.member` remains available for explicit selection

### Requirement: Modules can be published from index files

`share mod name from "relative-path"` SHALL publish the target source module
under `name` from the containing package or file module. The target path SHALL
be relative, omit `.zrk`, and resolve deterministically.

#### Scenario: Module barrel export
- **WHEN** a package root contains `share mod stdin from "./stdin"`
- **THEN** `stdin` is importable from the package root
- **AND** the implementation file may remain otherwise unexported

### Requirement: Published module declarations retain private boundaries

Only declarations marked `share` inside a published module SHALL be importable
by another module. Publishing a module SHALL NOT publish every private helper
inside its source file.

#### Scenario: Private helper remains hidden
- **WHEN** a published module contains a private `helper()` and a shared `parse()`
- **THEN** consumers can import `parse()`
- **AND** importing `helper()` emits the existing visibility diagnostic

### Requirement: Module identity is stable across re-exports

Re-exporting the same source module from multiple package-root paths SHALL NOT
create duplicate declarations or duplicate initialization. Diagnostics and
reflection SHALL retain the canonical package/module identity and written path.

#### Scenario: Diamond re-export
- **WHEN** two package index modules re-export the same module
- **THEN** the loader reads the source module once
- **AND** both paths refer to the same canonical declarations

### Requirement: Local imports remain within an authorized source root

The loader SHALL resolve quoted imports and relative module re-exports only within the root of the current project, or within the entry file's directory when loading a standalone file or editor overlay. A declared dependency SHALL be confined to its resolved package root. Existing paths SHALL be checked after filesystem canonicalization so that symlinks cannot escape their authorized root. Paths that escape a root SHALL produce a dedicated diagnostic at the import or re-export that requested the path, without reading the target source.

#### Scenario: Parent traversal escapes the project
- **WHEN** a project source imports a relative path that normalizes outside the project root
- **THEN** the loader rejects the import with the root-containment diagnostic
- **AND** it does not read the target file

#### Scenario: Symlink escapes the package root
- **WHEN** an imported path resolves through a symlink to a file outside the current project or package root
- **THEN** the loader rejects the import with the root-containment diagnostic
- **AND** it does not read the target file

#### Scenario: Import stays inside its root
- **WHEN** an import resolves to a file within the current project's or package's root
- **THEN** the loader reads that file normally

#### Scenario: Declared dependency source
- **WHEN** a project imports a file within a dependency resolved from its manifest and lockfile
- **THEN** the loader accepts the file within that dependency's own root
- **AND** it rejects paths that escape the dependency root

#### Scenario: Editor overlay entry does not exist on disk
- **WHEN** an editor overlay supplies the entry source at a path whose parent is within the standalone source root
- **THEN** the loader accepts the overlay entry and resolves its imports under that root

#### Scenario: Missing file within root
- **WHEN** an imported path does not exist but remains lexically inside its authorized root
- **THEN** the loader emits the existing unreadable-file diagnostic at the requesting import
