## ADDED Requirements

### Requirement: The standard library uses the package and module model

Every public standard-library area SHALL have a stable logical package/module
identity and a published root surface compatible with ordinary package imports.
`std.io` SHALL expose `stdin`, `stdout`, and `stderr` as published modules.

#### Scenario: Standard I/O import
- **WHEN** a program imports `{ stdin, stdout, stderr } from std.io;`
- **THEN** all three names resolve through the standard package root
- **AND** the source does not depend on a compiler-only intrinsic spelling

### Requirement: Standard-library Rust bindings are compiler-owned

The public standard-library source SHALL express its API as ordinary Zirk
declarations without `native`, `intrinsic`, or ABI annotations. The compiler
MAY associate a fully qualified standard-library declaration with Rust runtime
behavior only when the declaration belongs to the official standard-library
sources. External packages SHALL NOT acquire that association by name alone.

#### Scenario: Official I/O implementation
- **WHEN** `std.io::stdout::write` is compiled for a native target
- **THEN** the compiler may lower that known standard declaration to the runtime's native output implementation
- **AND** the source signature remains an ordinary Zirk function

#### Scenario: External same-name function
- **WHEN** an external package declares `write(text: String)`
- **THEN** it remains an ordinary Zirk function
- **AND** the compiler does not bind it to the standard runtime implementation

### Requirement: Standard-library convenience opening is deterministic

Published convenience members of an imported standard module SHALL be available
unqualified by default when unambiguous and SHALL remain selectable through
`module::member`. The standard library SHALL document collisions and the
qualification form for every convenience member.

#### Scenario: Unqualified standard call
- **WHEN** `{ stdout } from std.io` is imported and no `println` conflicts
- **THEN** `println("ready")` is accepted
- **AND** it is equivalent to `stdout::println("ready")`
