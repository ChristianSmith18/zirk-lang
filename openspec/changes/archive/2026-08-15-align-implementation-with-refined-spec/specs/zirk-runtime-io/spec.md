## MODIFIED Requirements

### Requirement: Opaque representation of String

The runtime SHALL expose `String` as an opaque handle whose layout is
private, per `docs/decisions/ADR-005-representacion-string.md`.

The handle SHALL also be the `String`'s observable identity: it is what
`is` compares. This is what allows grapheme indexing, normalization, and
any internal cache to live entirely inside the runtime, without the
compiler needing to know about them.

#### Scenario: Opacity for the compiler
- **WHEN** codegen manipulates a `String` value
- **THEN** it treats it as an opaque handle
- **AND** it does NOT inspect or assume its internal representation

#### Scenario: Construction from a literal
- **WHEN** generated code materializes a string literal
- **THEN** it invokes the runtime function that builds a `String` from UTF-8 bytes and its length

#### Scenario: The handle is the identity
- **WHEN** two `String` values come from the same handle
- **THEN** they are identical for the `is` operator
- **AND** two distinct handles are NOT identical even if their content matches

## ADDED Requirements

### Requirement: Content equality indifferent to normalization

The runtime's equality function SHALL compare the content of two `String`
values, treating two canonically equivalent sequences as equal even if
their bytes differ.

The comparison SHALL resolve without allocating memory in the common case:
same handle, or identical bytes, decide the result immediately.

#### Scenario: Different normalization forms
- **WHEN** a string in NFC and another in NFD with the same perceived content are compared
- **THEN** equality returns true

#### Scenario: Different content
- **WHEN** two strings whose perceived content differs are compared
- **THEN** equality returns false

#### Scenario: Fast path
- **WHEN** two handles match, or their bytes are identical
- **THEN** the result is decided without normalizing or allocating memory

### Requirement: Literals normalized at compile time

The compiler SHALL emit string literals in canonical form, so that
comparison between literals is resolved by byte comparison.

Normalizing once at compile time is what keeps the runtime equality rule
cheap.

#### Scenario: Literal in decomposed form in the source
- **WHEN** a string literal appears in the source in decomposed form
- **THEN** the runtime receives it already in canonical form

#### Scenario: Comparison between literals
- **WHEN** two literals with the same perceived content are compared
- **THEN** the comparison is resolved by bytes, without normalizing at runtime

### Requirement: Hash consistent with equality

When the runtime exposes a `String`'s hash, it SHALL be derived from its
canonical form.

Two strings equal according to the equality function SHALL always produce
the same hash.

#### Scenario: Hash of equivalent forms
- **WHEN** the hash of a string in NFC and that of its NFD equivalent are computed
- **THEN** both hashes match
