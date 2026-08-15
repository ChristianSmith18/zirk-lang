## ADDED Requirements

### Requirement: Complete conditional and loop forms
The parser SHALL accept value-producing `if`, SHALL accept a single immediate statement without braces, SHALL accept `for mut i = 0; i < 10; i++`, and SHALL accept `do { ... } while condition;`. A ternary SHALL remain the preferred compact value form but SHALL NOT replace the `if` expression.

#### Scenario: Single-statement conditional
- **WHEN** source contains `if closed return;`
- **THEN** the return is the sole conditional statement

#### Scenario: Traditional for
- **WHEN** source contains `for mut i = 0; i < 10; i++ { work(i); }`
- **THEN** the parser records initializer, condition, update, and body

#### Scenario: Do while
- **WHEN** source contains `do { poll(); } while pending;`
- **THEN** the parser records a post-condition loop whose body runs before its condition

### Requirement: Complete range and slice forms
The parser SHALL accept `start..end`, `start..=end`, descending bounds, `.step(distance)`, `.reverse()`, interpolated bounds such as `0..{number}`, and slices `[start:end:step]` with omitted or negative components.

#### Scenario: Descending stepped range
- **WHEN** source contains `10..=0.step(2)`
- **THEN** it represents an inclusive descending range with distance two

#### Scenario: Reverse slice
- **WHEN** source contains `values[::-1]`
- **THEN** it represents a slice with omitted bounds and negative step

### Requirement: Match alternatives, regex, and nested destructuring
The parser SHALL group alternative patterns with commas followed by one `=>` body, SHALL accept regex literals as patterns, and SHALL compose enum payload and record destructuring as `UserCreated({ id, name })`.

#### Scenario: Multiline alternatives
- **WHEN** `200`, `201`, and `204` appear on separate lines before one `=>` body
- **THEN** all three patterns select that body

#### Scenario: Nested enum payload pattern
- **WHEN** source contains `UserCreated({ id, name }) => handle(id, name)`
- **THEN** the enum payload is destructured and both names bind in the branch

### Requirement: Function and constructor surface
The parser SHALL accept lambdas with or without leading `fn`, SHALL require a type on every optional parameter, SHALL accept multiple `construct` declarations, SHALL accept reordered named construction arguments, and SHALL parse `fn gen` with `yield`.

#### Scenario: Optional fn lambda
- **WHEN** source contains `fn(a: Int32): Int32 => a + 1`
- **THEN** it produces the same lambda form as `(a: Int32): Int32 => a + 1`

#### Scenario: Multiple constructors
- **WHEN** a class contains two `construct` declarations with different parameter lists
- **THEN** both constructor signatures are retained for semantic resolution

#### Scenario: Generator declaration
- **WHEN** source contains `fn gen numbers(): Int32 { yield 1; }`
- **THEN** it produces a generator function whose declared type is its yielded element type

### Requirement: Traditional enum mappings
The parser SHALL accept traditional enum cases without mappings and cases mapped with `->` to compatible string or numeric values.

#### Scenario: String-mapped case
- **WHEN** source contains `North -> "N";`
- **THEN** the enum case retains `"N"` as its explicit observable mapping
