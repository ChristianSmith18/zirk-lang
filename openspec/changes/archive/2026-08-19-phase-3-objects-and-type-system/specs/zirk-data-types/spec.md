## ADDED Requirements

### Requirement: Algebraic enums

An `enum` SHALL admit variants with associated values, extending the previous phase's data-less enum.

A traditional case with no mapping SHALL expose its exact name as its observable value and SHALL NOT receive an implicit numeric index. A case MAY declare a compatible string or numeric mapping via `->`.

#### Scenario: Variant with data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is declared
- **THEN** `Shape.Circle(3)` constructs a value

#### Scenario: Data-less enum still works
- **WHEN** `enum Direction { North, South }` is declared
- **THEN** it compiles the same as in the previous phase

#### Scenario: Constructor with incorrect arity
- **WHEN** a variant is constructed with more or fewer values than declared
- **THEN** a diagnostic stating the expected arity is emitted

### Requirement: Destructuring in `match`

A pattern SHALL be able to extract a variant's associated values, binding them to names inside the arm.

#### Scenario: Extracting a variant's data
- **WHEN** `match s { Shape.Circle(r) => r, Shape.Rect(w, h) => w * h }` is written
- **THEN** `r`, `w`, and `h` are bound in their arm with the declared type

#### Scenario: Pattern with incorrect arity
- **WHEN** a variant pattern binds fewer names than the variant declares
- **THEN** a diagnostic stating the arity is emitted

#### Scenario: Exhaustiveness with associated data
- **WHEN** a `match` over an algebraic enum omits a variant and has no `_`
- **THEN** the exhaustiveness diagnostic naming the missing variant is emitted

### Requirement: Records

A `record` SHALL be immutable and have structural semantics, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Structural equality
- **WHEN** two records with the same field values are compared
- **THEN** `==` produces `true` even if they are distinct instances

#### Scenario: Immutability
- **WHEN** a record's field is assigned to
- **THEN** a diagnostic stating that a record is not mutable is emitted

### Requirement: Value classes

A value class SHALL NOT have observable identity and SHALL be storable inline.

#### Scenario: No identity
- **WHEN** the identity operator is applied to two value classes with the same content
- **THEN** a diagnostic stating that they have no observable identity is emitted

#### Scenario: Stored without indirection
- **WHEN** a value class is a field of another declaration
- **THEN** it occupies its space inside it, with no intermediate pointer

### Requirement: Unions and aliases

`A | B` SHALL declare a union, and `type` SHALL declare an alias.

#### Scenario: Value of a union
- **WHEN** a variable is declared `Int32 | String`
- **THEN** it admits values of either

#### Scenario: Use without discriminating
- **WHEN** a union value is used where one of its members is expected
- **THEN** a diagnostic is emitted
- **AND** the hint states to discriminate it with `match`

#### Scenario: Alias
- **WHEN** `type Id = Int32;` is declared
- **THEN** `Id` and `Int32` are interchangeable
