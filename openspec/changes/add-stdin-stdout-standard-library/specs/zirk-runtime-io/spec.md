## MODIFIED Requirements

### Requirement: No dependency on the Zirk stdlib

The runtime SHALL not parse, discover, or load Zirk standard-library source.
The compiler MAY map only a fully-qualified declaration from the compiler-owned
`std.io` registry to a stable runtime operation. A user package declaration
with the same name or signature SHALL remain an ordinary Zirk declaration.

#### Scenario: Standard stream declaration
- **WHEN** a program imports and calls `stdout.println` from `std.io`
- **THEN** the compiler maps that identified declaration to its output runtime operation

#### Scenario: External same-name declaration
- **WHEN** an external package publishes `stdout.println` with the same signature
- **THEN** it does not acquire a standard runtime binding

#### Scenario: Program without imports
- **WHEN** a legacy program calls `stdout.println` without an import statement
- **THEN** it remains compatible with the standard output operation during the migration
