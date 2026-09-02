## MODIFIED Requirements

### Requirement: Subset types

The checker SHALL support the types `Void`, `Int32`, `Boolean`, `String`,
nullable types (`T?`), and `enum` without associated data, per
`ZIRK_LANGUAGE_SPEC.md` sections 3 and 4.

The `Int` and `Integer` aliases SHALL resolve to `Int32`, which is exactly
what they name. An alias for an already-implemented type is not a deferred
capability.

#### Scenario: Unknown type
- **WHEN** an annotation names a type outside the subset
- **THEN** a diagnostic naming the type is emitted
- **AND** if the type exists in the full language, the help notes that it is not implemented yet

#### Scenario: `T?` is distinct from `T`
- **WHEN** the type `String?` is compared with `String`
- **THEN** the checker treats them as distinct types, not interchangeable without coalescing or safe access

#### Scenario: Short alias for the default integer
- **WHEN** `mut count: Int = 0;` or `mut count: Integer = 0;` is declared
- **THEN** checking succeeds and the type is indistinguishable from `Int32`

#### Scenario: Alias of a not-yet-implemented type
- **WHEN** an annotation names `UInt`
- **THEN** the not-yet-implemented-type diagnostic is emitted with `UInt32`'s phase

### Requirement: `for ... in` over the minimal iteration protocol

The checker SHALL admit `for ... in` over ranges (`0..N`, `0..=N`) and over
`String`, which iterates by graphemes binding an element of type `Char`.
Over any other type, it SHALL reject it, stating that iteration over
user-defined types arrives with Phase 3's traits.

While `Char` is not implemented, iteration over `String` SHALL be deferred
with the phase diagnostic, without binding an element of another type.

#### Scenario: Iteration over a range
- **WHEN** `for i in 0..10 { }` is written
- **THEN** `i` has type `Int32` inside the body

#### Scenario: Iteration over an unsupported type
- **WHEN** `for x in valor { }` is written and `valor` is neither a range nor `String`
- **THEN** a diagnostic stating that this type is not iterable yet is emitted

#### Scenario: Iteration over `String`
- **WHEN** `for c in texto { }` is written with `texto: String`
- **THEN** the bound element is a grapheme of type `Char`
- **AND** while `Char` is not implemented, the phase diagnostic is emitted instead of binding a `String`

### Requirement: Float family replaces Decimal family
The binary floating family SHALL be `Float16`, `Float32`, `Float64`, and `Float128`, with `Float` aliasing `Float64` and ordinary fractional literals inferring `Float64`. `NaN` SHALL NOT be a valid Zirk value; indeterminate operations SHALL produce controlled errors.

`Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec` and `Decimal` SHALL NOT be recognized as types of the language, nor announced as types of a future phase. An exact base-ten type may later arrive as a standard-library type, and it would be a different thing from `Float`.

#### Scenario: Default fractional literal
- **WHEN** `1.5` has no contextual type
- **THEN** its inferred type is `Float64`

#### Scenario: Indeterminate infinity operation
- **WHEN** positive infinity is subtracted from positive infinity
- **THEN** a controlled arithmetic error is produced instead of `NaN`

#### Scenario: Withdrawn Decimal family
- **WHEN** an annotation names `Decimal64`
- **THEN** an unknown-type diagnostic is emitted
- **AND** no arrival phase is announced for that name

## ADDED Requirements

### Requirement: `Float` family and temporal types recognized as pending

The checker SHALL recognize `Float16`, `Float32`, `Float64`, `Float128`,
`Float`, the not-yet-implemented integer widths, `Char`, and the temporal
types `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`,
`Duration`, and `Period` as pending language types, each declaring the
phase that brings it.

#### Scenario: Annotation with a Float type
- **WHEN** `mut ratio: Float64 = 0;` is declared
- **THEN** the diagnostic names the type and states the phase in which it arrives
- **AND** it is NOT reported as a nonexistent type

#### Scenario: Annotation with a temporal type
- **WHEN** an annotation names `Instant` or `Duration`
- **THEN** the diagnostic states the temporal family's phase

### Requirement: Observable identity and equality of `String`

The checker and the runtime SHALL treat `String` as a reference with
observable identity: `is` SHALL compare referent identity and `==` SHALL
compare content.

Content comparison SHALL be indifferent to the Unicode normalization form
of the operands. A `String`'s hash SHALL be derived from its canonical
form, so that two strings equal by `==` never produce different hashes.

#### Scenario: Equality indifferent to normalization
- **WHEN** two strings with the same perceived content, one in NFC and the other in NFD, are compared with `==`
- **THEN** the result is `true`

#### Scenario: Identity versus content
- **WHEN** two distinct bindings alias the same `String` and a third has the same content in another referent
- **THEN** `is` is `true` only for the first two and `==` is `true` for all three

#### Scenario: Consistency between hash and equality
- **WHEN** two strings equal by `==` are used as map keys
- **THEN** they resolve to the same entry
