# universal-string-rendering Specification

## Purpose

Owns the render table for every Zirk value and the shared `to_string()` contract used by printing and interpolation.

## Requirements

### Requirement: Universal `to_string()` contract

Every value-producing type SHALL answer `to_string(): String`, through a compiler default or a declared method. Only `Void` and `Never` have no text form. `print`, `println`, and interpolation SHALL accept every other type and route through this conversion.

#### Scenario: Collection prints without declaration

- **WHEN** a `List<Int32>` holding `[1, 2, 3]` is printed or interpolated
- **THEN** it produces `[1, 2, 3]`

#### Scenario: Non-value types rejected

- **WHEN** `Void` or `Never` is printed, converted, or interpolated
- **THEN** a diagnostic states that it has no text form

### Requirement: Scalar and temporal renderings

Integers render base ten, except `IntN.to_string(radix: n)` which uses lowercase sign-magnitude base 2–36. Decimal and binary floats render decimal text, Boolean renders `true`/`false`, Char its grapheme, and String itself. Duration renders a normalized largest unit, Date/Time/DateTime use their ISO forms.

#### Scenario: Integer radix

- **WHEN** `255.to_string(radix: 16)` and `(-255).to_string(radix: 2)` run
- **THEN** they return `ff` and `-11111111`

### Requirement: Sequence rendering

Array and List SHALL render recursively as `[e0, e1, …]`, with `, ` separators and `[]` when empty; string elements are unquoted.

### Requirement: Deterministic `Map` and `Set` rendering

Map SHALL render `{k: v, …}` and Set `{e, …}`. Entries SHALL use natural key/element order where defined and rendered-text order otherwise; empty collections render `{}`.

#### Scenario: Map sorts by key

- **WHEN** a map contains keys `2`, `10`, and `1`
- **THEN** it renders keys in numeric order `1`, `2`, `10`

### Requirement: Range rendering mirrors its literal

Range SHALL preserve exclusive or inclusive bounds and append `:step` when present.

#### Scenario: Stepped inclusive range

- **WHEN** `(1..=10:2).to_string()` runs
- **THEN** it returns `1..=10:2`

### Requirement: Nullable rendering

A nullable receiver SHALL support direct `to_string()`: a present value renders its inner value and absence renders `<Null>`. `null` renders `<Null>`; `x?.to_string()` remains `String?`.

#### Scenario: Null in a list

- **WHEN** `List<Int32?>` contains `[1, null, 3]`
- **THEN** it renders `[1, <Null>, 3]`

### Requirement: Structured, enum, callable, and handle rendering

Records, classes, and tuples use readable structural forms and surface names, stripping `$local$N` while keeping `Outer.Inner` and excluding synthesized `outer` fields. Payload enum cases render `Variant(field: v)`; Result renders `Ok(5)` and `Error(boom)`. Named functions render `<Function name>`, anonymous functions `<Function>`. Job, Channel, Pointer, NativeSlice, NativeSliceMut, Dependent, Pin, and Weak use `<TypeName>` forms. Regex renders `re'pattern'`.

#### Scenario: Contract dispatch

- **WHEN** a value reached through a contract declaring `to_string()` is printed
- **THEN** its concrete implementation supplies the text
