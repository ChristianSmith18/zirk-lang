## ADDED Requirements

### Requirement: Package and module declarations have explicit grammar

The grammar SHALL recognize `package qualified.name;`, `mod name { ... }`, and
`share mod name from "relative-path";` at their permitted source scopes. The
parser SHALL retain declaration and path spans for diagnostics and tooling.

#### Scenario: Package index declaration
- **WHEN** source contains `package std.io; share mod stdout from "./stdout";`
- **THEN** the syntax tree records the package identity and one module re-export

#### Scenario: Inline module declaration
- **WHEN** source contains `mod stdout { share fn println(): Void { } }`
- **THEN** the syntax tree records a named module with its published function

### Requirement: Package paths and relative paths are lexically distinct

An import source beginning with `./` or `../` and enclosed in quotes SHALL be
parsed as a relative source path. An unquoted dotted identifier path SHALL be
parsed as a logical package/module path. The parser SHALL reject ambiguous or
mixed spellings with a targeted diagnostic.

#### Scenario: Relative import
- **WHEN** source contains `import { User } from "./models/user";`
- **THEN** the source is recorded as a relative path without `.zrk`

#### Scenario: Logical package import
- **WHEN** source contains `import { stdout } from std.io;`
- **THEN** the source is recorded as an unquoted logical path

### Requirement: Explicit module member qualification is parsed

The grammar SHALL recognize `module::member` in value, call, type, and import
member positions where a qualified name is legal. It SHALL preserve the full
qualification for resolution rather than interpreting `::` as punctuation.

#### Scenario: Qualified call
- **WHEN** source contains `stdout.println("hello");`
- **THEN** the parser records a two-segment qualified member access

#### Scenario: Qualified import
- **WHEN** source contains `import { println } from std.io::stdout;`
- **THEN** the parser records the package path `std.io` and module segment `stdout`
