# universal-string-rendering Specification

## Purpose

Owns the render table for every Zirk value and the shared `to_string()` contract used by printing and interpolation.

## Requirements

### Requirement: Universal `to_string()` contract

Every value-producing type SHALL answer `to_string(): String`, through a compiler default or a declared method. Only `Void` and `Never` have no text form. `print`, `println`, and interpolation SHALL accept every other type and route through this conversion. When a Result expression is consumed by printing or interpolation, it SHALL first use contextual Result projection; an `Ok(value)` renders the value and an `Error(_)` renders the nullable absence marker. An explicitly invoked `to_string()` on a receiver statically typed as `Result<T,E>` SHALL retain the Result variant representation.

#### Scenario: Collection prints without declaration

- **WHEN** a `List<Int32>` holding `[1, 2, 3]` is printed or interpolated
- **THEN** it produces `[1, 2, 3]`

#### Scenario: Non-value types rejected

- **WHEN** `Void` or `Never` is printed, converted, or interpolated
- **THEN** a diagnostic states that it has no text form

#### Scenario: Result success prints its value

- **WHEN** `println(Env.get("APP_MODE"))` receives `Ok("production")`
- **THEN** it prints `production`

#### Scenario: Result failure prints nullable absence

- **WHEN** `println(Env.get("APP_MODE"))` receives `Error(permission_error)`
- **THEN** it prints the established null marker `<Null>` without rendering or exposing the error payload

#### Scenario: Explicit Result stringification preserves the variant

- **WHEN** `inmut result: Result<Int32, E> = Ok(5); result.to_string()` is invoked
- **THEN** it renders `Ok(5)`

### Requirement: Scalar and temporal renderings

Integers SHALL render in base ten, except `IntN.to_string(radix: n)` which uses lowercase sign-magnitude base 2–36. Decimal and binary floats SHALL render decimal text, Boolean SHALL render `true`/`false`, Char SHALL render its grapheme, and String SHALL render itself. Duration SHALL render a normalized largest unit; Date/Time/DateTime SHALL use their ISO forms.

#### Scenario: Integer radix

- **WHEN** `255.to_string(radix: 16)` and `(-255).to_string(radix: 2)` run
- **THEN** they return `ff` and `-11111111`

### Requirement: Sequence rendering

Array and List SHALL render recursively as `[e0, e1, …]`, with `, ` separators and `[]` when empty; string elements are unquoted.

#### Scenario: Sequence renders its items in order

- **WHEN** a `List<Int32>` containing `[1, 2, 3]` is converted to text
- **THEN** it renders as `[1, 2, 3]`

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

Records, classes, and tuples SHALL use readable structural forms and surface names, stripping `$local$N` while keeping `Outer.Inner` and excluding synthesized `outer` fields. Payload enum cases SHALL render `Variant(field: v)`; Result SHALL render `Ok(5)` and `Error(boom)`. Named functions SHALL render `<Function name>`, anonymous functions `<Function>`. Job, Channel, Pointer, NativeSlice, NativeSliceMut, Dependent, Pin, and Weak SHALL use `<TypeName>` forms. Regex SHALL render `re'pattern'`.

#### Scenario: Contract dispatch

- **WHEN** a value reached through a contract declaring `to_string()` is printed
- **THEN** its concrete implementation supplies the text
