# enum-static-members Specification

## ADDED Requirements

### Requirement: Enum type static members
Every declared enum type `E` SHALL expose the built-in static members `E.count`, `E.keys()`, `E.values()`, `E.from_name(name)`, and `E.from_value(value)`, dispatched on the type name itself rather than on a value.

#### Scenario: `count` is the variant count
- **WHEN** `enum Direction { North, South, East, West }` is declared
- **THEN** `Direction.count` is `Int32` and evaluates to `4`

#### Scenario: `keys()` lists variant names in order
- **WHEN** `Direction.keys()` is called
- **THEN** the result is `List<String>` `["North", "South", "East", "West"]` in declaration order

#### Scenario: `values()` lists enum values in order
- **WHEN** `Direction.values()` is called
- **THEN** the result is `List<Direction>` `[Direction.North, Direction.South, Direction.East, Direction.West]` in declaration order

### Requirement: `from_name` performs a controlled name lookup
`E.from_name(name)` SHALL return `Result<E, LookupError>`: `Ok(case)` when `name` is a declared variant name, `Err(LookupError)` otherwise.

#### Scenario: Known name resolves
- **WHEN** `Direction.from_name("North")` is evaluated
- **THEN** the result is `Ok(Direction.North)`

#### Scenario: Unknown name is a controlled failure
- **WHEN** `Direction.from_name("Nowhere")` is evaluated
- **THEN** the result is `Err(LookupError)`, not a crash or a trap

### Requirement: `from_value` performs a controlled mapped-value lookup
`E.from_value(value)` SHALL return `Result<E, LookupError>` resolving a case by its declared `->` mapping, and SHALL be rejected when `E` has no mappable values (algebraic variants carry payloads, not observable values).

#### Scenario: Mapped value resolves
- **WHEN** `enum ExitCode { Success -> 0, Failure -> 1 }` is declared and `ExitCode.from_value(0)` is evaluated
- **THEN** the result is `Ok(ExitCode.Success)`

#### Scenario: Algebraic enum rejects `from_value`
- **WHEN** `enum Shape { Circle(Int32) }` is declared and `Shape.from_value(3)` is written
- **THEN** compilation fails because algebraic payloads have no observable mapped value

### Requirement: `Enums` generic helpers
The `Enums` type SHALL expose `Enums.keys(E)`, `Enums.values(E)`, and `Enums.count(E)` equivalent to `E.keys()`, `E.values()`, and `E.count` when the enum type is not known statically at the call site.

#### Scenario: Generic helper forwards to the type
- **WHEN** `Enums.keys(Direction)` is evaluated
- **THEN** the result is identical to `Direction.keys()`

### Requirement: `LookupError` is a native error type
`LookupError` SHALL be registered in the native exception hierarchy as a sibling of `ParseError`/`RegexError`, so `Result<E, LookupError>` composes with `?` and `match` like every other `Result`.

#### Scenario: `LookupError` participates in `Result`
- **WHEN** `Direction.from_name("x")?` or a `match` over the result is written
- **THEN** it type-checks like any `Result<_, Error>` consumer
