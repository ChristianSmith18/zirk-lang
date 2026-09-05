# zirk-data-types Specification

## Purpose

Defines tuples, records, enums, unions, aliases, and their value semantics. This change removes `value class` and updates type categories accordingly.

## REMOVED Requirements

### Requirement: Value class declarations

**Reason**: `value class` is semantically redundant with `record` and is currently half-finished. `record` already covers the same use cases with a complete feature set.

**Migration**: Replace every `value class Name(field: Type);` with `record Name { field: Type; }` or `class Name { ... }` as appropriate for the desired mutability and identity semantics.

#### Scenario: Old value class is rejected
- **WHEN** `value class UserId(value: UInt64);` is written
- **THEN** a diagnostic is emitted indicating that `value class` is not supported

#### Scenario: Migration to record works
- **WHEN** `record UserId { value: UInt64; }` is written instead
- **THEN** the declaration is accepted and behaves as a nominal value type

## ADDED Requirements

### Requirement: Type categories updated

The user-defined type categories SHALL be `class` (reference with identity), `record` (nominal immutable value), `enum` (closed nominal set), `union`, and `alias`. Documentation and diagnostics SHALL no longer mention `value class`.

#### Scenario: Type category documentation
- **WHEN** a developer consults the handbook for user-defined types
- **THEN** `value class` is absent and `record` is the recommended value type

### Requirement: Type alias lowering

A `type` alias declared at module scope SHALL resolve to its underlying type throughout the compiler pipeline and SHALL be usable in executable programs without a `NOT_LOWERED` error.

#### Scenario: Alias in a variable declaration
- **WHEN** `type UserId = Int32;` and `mut x: UserId = 5;` are written
- **THEN** the variable `x` has type `Int32` and the program compiles and runs

#### Scenario: Alias to a generic type
- **WHEN** `type IntList = List<Int32>;` and `mut l: IntList = List<Int32>();` are written
- **THEN** the alias resolves to `List<Int32>` and `l` behaves as a `List<Int32>`

### Requirement: Clone derivation for record and enum

A `record` or `enum` SHALL automatically implement `Clone` when every field/associated value implements `Clone`. A record or enum containing a non-`Clone` field SHALL be rejected with the same diagnostic used for `class`.

#### Scenario: Record clone
- **WHEN** `record Point { x: Int32; y: Int32; }` is declared and `p2 = p1.clone()` is called
- **THEN** `p2` is an independent copy of `p1`

#### Scenario: Enum clone
- **WHEN** `enum Color { Red, Green, Blue }` is declared and `c2 = c1.clone()` is called
- **THEN** `c2` is an independent copy of `c1`

#### Scenario: Non-Clone field rejected
- **WHEN** a `record` contains a `class` field that does not implement `Clone`
- **THEN** deriving `Clone` for the record is rejected

### Requirement: Generic contract satisfaction

A user-defined generic `class` or `record` that `implements` a generic contract (e.g., `Iterable<T>`) SHALL have its contract methods lowered with the type parameters substituted at each call site.

#### Scenario: Generic iterable class
- **WHEN** `class Box<T> implements Iterable<T>` declares `fn iterator(): Iterator<T>`
- **THEN** `for x in Box<Int32>()` compiles and yields `Int32` values

#### Scenario: Generic iterable record
- **WHEN** `record Pair<T, U> implements Iterable<T>` declares a custom iterator
- **THEN** `for x in Pair<Int32, String>()` compiles and yields the first `Int32` element
