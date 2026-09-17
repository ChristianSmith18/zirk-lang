## MODIFIED Requirements

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
MAY always select the member explicitly with `module::member`. The same default
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
- **THEN** `clear()` and `terminal::clear()` both resolve when no collision exists

#### Scenario: Explicit member import
- **WHEN** `import { println } from std.io::stdout;` is written
- **THEN** only the selected published member is bound under `println`
- **AND** its owning module remains available only when separately imported

#### Scenario: Package object import
- **WHEN** an object is imported from an external package module
- **THEN** its published methods are opened by default when unambiguous
- **AND** `module::member` remains available for explicit selection

## ADDED Requirements

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
