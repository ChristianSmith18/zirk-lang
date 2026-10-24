# universal-string-rendering Specification

## Purpose

Owns the render table for every Zirk value and the shared `to_string()` contract used by printing and interpolation.

## Requirements

### Requirement: Universal `to_string()` contract

Every value-producing type SHALL answer `to_string(): String`, through a compiler default or a declared method, and SHALL satisfy the standard `Stringifiable` trait of `std.io` automatically, without declaring it. The trait supplies a concrete default `to_string()`, so a class that declares `to_string()` replaces that body and SHALL mark the method `#override`. Only `Void` and `Never` have no text form. `print`, `println`, and interpolation SHALL accept every other type and route through this conversion.

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

#### Scenario: Class replaces the standard rendering

- **WHEN** a class declares `#override to_string(): String { return this.text; }` and an instance is printed
- **THEN** the printed text is the method's result

#### Scenario: Missing override marker

- **WHEN** a class declares `to_string(): String { ... }` without `#override`
- **THEN** compilation fails with a diagnostic that the method replaces a trait default and needs `#override`

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

### Requirement: Dynamic objects have bounded deterministic rendering
An `Object` without an explicit `to_string()` implementation SHALL render as a readable object form using its string keys in deterministic insertion order. Rendering SHALL be bounded, SHALL mark cycles and unsupported values without recursing indefinitely, and SHALL NOT be treated as JSON serialization. A contract or class implementation of `to_string()` SHALL take precedence through ordinary dispatch.

#### Scenario: Render an object with string keys
- **WHEN** an object with `status: "ok"` is printed
- **THEN** it produces a readable object form containing the key and value in deterministic order

#### Scenario: Render a cyclic object
- **WHEN** an object graph contains a cycle and is rendered
- **THEN** rendering terminates and marks the cycle rather than overflowing

#### Scenario: Explicit rendering wins
- **WHEN** a class or contract view provides a compatible `to_string()` implementation
- **THEN** ordinary dispatch uses that implementation

### Requirement: Non-object structured rendering remains stable
Classes, tuples, payload enum cases, `Result`, named and anonymous functions, runtime handles, weak references, and regular expressions SHALL retain their documented readable renderings and surface names. Dynamic `Object` rendering SHALL follow the separate bounded rule above and SHALL NOT be confused with JSON serialization.

#### Scenario: Result and enum rendering remain readable
- **WHEN** an `Ok` result or a payload enum case is converted to a string
- **THEN** it uses its documented variant-oriented form

#### Scenario: Runtime handles use stable type names
- **WHEN** a job, channel, pointer, dependent reference, pin, or weak reference is rendered
- **THEN** the output uses its documented handle form without exposing internal addresses
