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

Module resolution SHALL support two files importing each other, and SHALL read each file of the crate exactly once.

An earlier version of this requirement required rejecting cycles. This was corrected during implementation: `import` brings names into scope and nothing in this phase depends on the order in which files are read — signatures are collected before any body is checked — so two files that reference each other mutually are a normal program. The real danger is traversing the cycle indefinitely, and that is solved by reading each file once, not by rejecting the program.

#### Scenario: Direct cycle
- **WHEN** file A imports from file B and file B imports from file A
- **THEN** the crate compiles
- **AND** each file is read exactly once

#### Scenario: Diamond of imports
- **WHEN** A imports from B and from C, and both import from D
- **THEN** `D` is read exactly once

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

`import { names } from standard.module;` with an unquoted name SHALL resolve against the modules the compiler recognizes as part of the standard library, per `ZIRK_LANGUAGE_SPEC.md` section 10.

This phase only recognizes `std.io` with `stdout`, `stdin`, and `stderr`, already existing as intrinsics since Phase 1. The rest of the standard library is Phase 7.

#### Scenario: Importing `std.io`
- **WHEN** `import { stdout, stderr } from std.io;` is written
- **THEN** both names resolve to the existing intrinsics

#### Scenario: Unrecognized standard module
- **WHEN** something is imported from a standard module other than `std.io`
- **THEN** a diagnostic indicating that module arrives in a later phase is emitted

### Requirement: Standard-library convenience member resolution
Importing a compiler-known standard-library object SHALL make its declared convenience members callable without qualification when the name is otherwise unambiguous. A local or imported collision SHALL require qualification through the imported object. Objects from local files or packages SHALL NOT inject their methods into file scope.

#### Scenario: Direct standard output call
- **WHEN** a file imports `{ stdout } from std.io` and has no competing `println`
- **THEN** `println("hello")` resolves to `stdout.println("hello")`

#### Scenario: Ambiguous convenience name
- **WHEN** the file also declares or imports another `println`
- **THEN** an unqualified call is diagnosed and `stdout.println(...)` selects the standard operation

#### Scenario: Package object import
- **WHEN** an object is imported from a package rather than a standard module
- **THEN** its methods remain accessible only through the object unless explicitly exported as declarations
