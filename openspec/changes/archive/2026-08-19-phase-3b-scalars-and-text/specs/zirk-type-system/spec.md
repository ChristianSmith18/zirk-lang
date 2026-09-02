## ADDED Requirements

### Requirement: `to_string()` contract

The language SHALL define `to_string()` as a reserved contract, in the same family as operator contracts (`ZIRK_LANGUAGE_SPEC.md` section 4): every native scalar type implements it, and a user type — class, record, enum — can implement it to produce its own textual representation. `print`/`println` SHALL route through it instead of accepting only a closed list of types.

#### Scenario: Printable native type with nothing declared
- **WHEN** an `Int32`, `Float64`, `Boolean`, or `Char` is printed
- **THEN** the check succeeds without the program declaring `to_string()` for them

#### Scenario: User type implementing `to_string()`
- **WHEN** a class declares `to_string(): String` and an instance is passed to `println`
- **THEN** the check succeeds and the printed value is the one that method produces

#### Scenario: User type that does not implement it
- **WHEN** a class with no `to_string()` is passed to `println`
- **THEN** a diagnostic naming the type is emitted, distinct from a generic `TYPE_MISMATCH`

### Requirement: String interpolation

A `String` literal SHALL admit `{expr}` inside it, desugared into concatenating the literal text with `expr.to_string()` for each interpolated expression, in the order they appear.

#### Scenario: Interpolating a variable
- **WHEN** `"User: {user.name}"` is written
- **THEN** the result concatenates the literal text with `user.name.to_string()`

#### Scenario: Interpolating a non-printable type
- **WHEN** the interpolated expression has a type with no `to_string()`
- **THEN** the same diagnostic as passing that value directly to `println` is emitted

### Requirement: Fractional literal context and mixed arithmetic

A fractional literal with no annotation SHALL have type `Float64`, and an arithmetic operation between an integer type and a `Float` type SHALL produce `Float`. `ZIRK_LANGUAGE_SPEC.md` section 3 already documented both rules; this is the first phase in which `Float` exists and they become verifiable.

#### Scenario: Fractional literal with no annotation
- **WHEN** `mut x = 1.5;` is written with no type annotation
- **THEN** `x` has type `Float64`

#### Scenario: Mixed integer and `Float` arithmetic
- **WHEN** an `Int32` and a `Float64` are added
- **THEN** the result has type `Float64`
