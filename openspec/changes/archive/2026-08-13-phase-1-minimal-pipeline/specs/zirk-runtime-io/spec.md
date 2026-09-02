## ADDED Requirements

### Requirement: Opaque representation of String

The runtime SHALL expose `String` as an opaque handle whose layout is private, per `docs/decisions/ADR-005-representacion-string.md`.

#### Scenario: Opacity for the compiler
- **WHEN** codegen manipulates a `String` value
- **THEN** it treats it as an opaque handle
- **AND** it does NOT inspect or assume its internal representation

#### Scenario: Construction from a literal
- **WHEN** the generated code materializes a string literal
- **THEN** it invokes the runtime function that builds a `String` from UTF-8 bytes and its length

### Requirement: Standard output

The runtime SHALL expose an `extern "C"` function that writes a `String` to standard output followed by a newline.

#### Scenario: Printing a string
- **WHEN** a program invokes the print function with a `String`
- **THEN** the content appears on standard output followed by a newline

#### Scenario: Non-ASCII content
- **WHEN** the string contains Unicode characters outside ASCII
- **THEN** they are written correctly encoded in UTF-8

#### Scenario: Flush before terminating
- **WHEN** the program terminates
- **THEN** standard output is flushed before the process ends

### Requirement: Stability of runtime symbols

Every runtime symbol intended for generated code SHALL be declared `extern "C"` with no mangling and with a stable name.

They are compatibility surface: changing them breaks already-compiled binaries.

#### Scenario: Symbols without mangling
- **WHEN** the produced static library is inspected
- **THEN** the symbols intended for generated code appear with their literal name

### Requirement: No dependency on Zirk's stdlib

The runtime at this phase SHALL NOT require `std.io` to exist as a Zirk module.

`stdout.println` is resolved as a compiler intrinsic. This is deliberate debt, retired in Phase 7.

#### Scenario: Program without imports
- **WHEN** a program uses `stdout.println` with no `import` statement
- **THEN** it compiles and runs correctly
