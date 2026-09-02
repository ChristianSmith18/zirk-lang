## ADDED Requirements

### Requirement: Binary visibility across a crate's files

A top-level declaration SHALL be visible only within the file that defines it, unless marked `share`, in which case it SHALL be visible from any other file of the same crate that imports it.

This phase does not implement the three levels of `public`/`private`/`protected` from `ZIRK_LANGUAGE_SPEC.md` section 7 -- that depends on classes and is Phase 3. Visibility here is binary: shared or private to the file.

#### Scenario: Declaration private to the file
- **WHEN** a function without `share` is referenced from another file of the same crate
- **THEN** a diagnostic stating that the declaration is not accessible from outside its file is emitted

#### Scenario: Shared declaration
- **WHEN** a function marked `share` is imported from another file
- **THEN** the reference resolves to that declaration

### Requirement: Resolution of `import` with local paths

`import { names } from "path"` with a quoted path SHALL resolve to a file of the same crate, located relative to the importing file, without the `.zrk` extension in the written path.

#### Scenario: Resolved relative path
- **WHEN** `import { Usuario } from "./dominio/usuario";` is written in a given file
- **THEN** it resolves to the file `dominio/usuario.zrk` relative to that file

#### Scenario: Nonexistent file
- **WHEN** an `import`'s path does not correspond to any file of the crate
- **THEN** a diagnostic naming the path that was not found is emitted
- **AND** it points at the `import` that requested it, not at the start of the file

#### Scenario: Non-shared name in the target file
- **WHEN** a name that exists in the target file but is not marked `share` is imported
- **THEN** the same visibility diagnostic as a direct reference is emitted

### Requirement: Import alias

`import` SHALL admit renaming an imported name with the `name -> alias` syntax, and the resolved name SHALL be used under the alias within the importing file.

#### Scenario: Import with an alias
- **WHEN** `import { Rol -> RolDeDominio } from "./dominio/usuario";` is written
- **THEN** within the file, `RolDeDominio` resolves to the `Rol` declaration of the imported file
- **AND** the unaliased name `Rol` is not available in the importing file

### Requirement: Mutual imports

Module resolution SHALL admit two files importing from each other, and SHALL read each file of the crate only once.

An earlier version of this requirement demanded rejecting cycles. It was corrected during implementation: `import` brings names into scope and nothing in this phase depends on the order in which files are read -- signatures are collected before checking any body -- so two files that reference each other mutually are a normal program. The real danger is walking the cycle indefinitely, and that is solved by reading each file once, not by rejecting the program.

#### Scenario: Direct cycle
- **WHEN** file A imports from file B and file B imports from file A
- **THEN** the crate compiles
- **AND** each file is read only once

#### Scenario: Diamond of imports
- **WHEN** A imports from B and from C, and both import from D
- **THEN** `D` is read only once

### Requirement: Collision of shared names

Module resolution SHALL reject two `share` declarations with the same name within the same crate, with no implicit tie-break by file order.

#### Scenario: Two files share the same name
- **WHEN** two different files of the crate declare `share fn ayuda(): Void {}` with the same name
- **THEN** a diagnostic pointing at both declarations is emitted

### Requirement: `use` enables globals without a qualified name

`use` SHALL enable unqualified access to the globals of an already-imported module, without bringing new names into scope that were not first imported with `import`.

#### Scenario: `use` over an existing import
- **WHEN** a file imports `stdout` from `std.io` and then writes `use stdout;`
- **THEN** `println(...)` without the `stdout.` qualifier resolves to the same declaration

#### Scenario: `use` without a prior `import`
- **WHEN** `use` is written over a name the file did not import
- **THEN** a diagnostic stating that the name is not available in this file is emitted

### Requirement: Unquoted standard modules

`import { names } from standard.module;` with an unquoted name SHALL resolve against the modules the compiler recognizes as part of the standard library, per `ZIRK_LANGUAGE_SPEC.md` section 10.

This phase only recognizes `std.io` with `stdout`, `stdin`, and `stderr`, already existing as an intrinsic since Phase 1. The rest of the standard library is Phase 7.

#### Scenario: Importing `std.io`
- **WHEN** `import { stdout, stderr } from std.io;` is written
- **THEN** both names resolve to the already-existing intrinsics

#### Scenario: Unrecognized standard module
- **WHEN** an import comes from a standard module other than `std.io`
- **THEN** a diagnostic stating that this module arrives in a later phase is emitted
