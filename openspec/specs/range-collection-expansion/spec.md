# range-collection-expansion Specification

## Purpose

Defines expansion of finite ranges while constructing arrays and lists, contextual collection literal inference, and fixed-size allocation declarations.

## Requirements

### Requirement: Range expansion in collection construction

Array and List literals and their constructor calls SHALL expand each range operand into its generated sequence in source order. Expansion SHALL be available to both collection families and SHALL not require a separate spread operator.

#### Scenario: Array literal expands an exclusive range

- **WHEN** `inmut values: Array<Int32> = [0..3]` is constructed
- **THEN** the array contains `[0, 1, 2]` and has length `3`

#### Scenario: List constructor expands an inclusive range

- **WHEN** `inmut values: List<Int32> = List(0..=3)` is constructed
- **THEN** the list contains `[0, 1, 2, 3]` and has length `4`

#### Scenario: Mixed values and ranges expand in place

- **WHEN** `Array(9, 0..3, 10)` is constructed
- **THEN** the result contains `[9, 0, 1, 2, 10]`

### Requirement: Contextual collection literal selection

The `[...]` collection literal SHALL default to `Array<T>` when no expected collection type is available and SHALL produce `List<T>` when the destination context is `List<T>`. The same element and range expansion rules SHALL apply to both outcomes.

#### Scenario: Unannotated literal defaults to Array

- **WHEN** `inmut values = [0..100]` is written
- **THEN** the inferred type is `Array<Int32>` with values `0` through `99`

#### Scenario: List context selects List

- **WHEN** `inmut values: List<Int32> = [0..100]` is written
- **THEN** the value is a `List<Int32>` with values `0` through `99`

### Requirement: Fixed-size array allocation

An allocation-only declaration `T[n]` SHALL reserve exactly `n` fixed array slots without deriving its length from an initializer. The array SHALL remain non-resizable. Every slot SHALL be zero-initialized to `T`'s zero/default representation. A `T` with no zero/default representation SHALL be a compile-time error naming the first field or reason. Reading an unwritten slot SHALL be well-defined; there is no read-before-write rejection.

#### Scenario: Fixed array reserves declared capacity

- **WHEN** `inmut buffer: Int32[6];` is declared
- **THEN** `buffer.length` is `6`, every slot reads `0`, and indexed writes cannot change that length

#### Scenario: Fixed array of a type without a default

- **WHEN** `inmut rows: UserProfile[3];` is declared and `UserProfile` has a field with no default
- **THEN** a compile-time diagnostic names the field that prevents zero-initialization
