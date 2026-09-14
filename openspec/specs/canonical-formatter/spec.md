# canonical-formatter Specification

## Purpose

Defines the first canonical formatter slice for triple-quoted strings.

## Requirements

### Requirement: `zirk format` subcommand

The CLI SHALL expose `zirk format <file>`, rewriting a well-formed file in place; the `.zrk` extension is optional.

#### Scenario: Unparseable file

- **WHEN** `zirk format` receives syntax errors
- **THEN** it reports diagnostics and leaves the file unchanged

### Requirement: Triple-quoted-string layout rule

`zirk format` SHALL place an opening `'''` on the line after its introducing `=`, `(`, or `name:`, one indentation level deeper; the closing `'''` SHALL be aligned on its own line and content SHALL retain its position relative to the edge.

#### Scenario: Inline opening normalized

- **WHEN** `inmut t = '''\nhello\n    '''` is formatted
- **THEN** opening and closing delimiters are placed on aligned canonical lines

#### Scenario: Nested call argument

- **WHEN** a `'''` literal is an argument of `f(...)` or `f(text: ...)`
- **THEN** its delimiter is indented one level deeper than the call line

### Requirement: Idempotence and check mode

Formatting SHALL be a fixed point. `zirk format --check <file>` SHALL not write and SHALL exit non-zero when formatting would change the file.

#### Scenario: Second run is a no-op

- **WHEN** `zirk format` runs twice
- **THEN** the second run leaves identical bytes

#### Scenario: Other style left untouched

- **WHEN** a file has no `'''` literals
- **THEN** this formatter slice leaves it unchanged
