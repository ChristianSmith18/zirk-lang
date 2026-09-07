# zirk-data-types Delta Spec

## MODIFIED Requirements

### Requirement: Closed data-only enums
Traditional and algebraic enums SHALL be closed data declarations and SHALL NOT contain user-defined methods. Traditional cases SHALL expose native `.name`, `.value`, and `to_string()` behavior without implicit mapping conversion or declaration order; the enum type itself SHALL expose the built-in static members `count`, `keys()`, `values()`, `from_name()`, and `from_value()` (see `enum-static-members`). Algebraic payloads SHALL be extracted only through exhaustive match.

#### Scenario: Enum method rejected
- **WHEN** an enum body declares a method `to_celsius()`
- **THEN** compilation fails and domain behavior must be expressed by an external function with match

### Requirement: Generic contract satisfaction

A user-defined generic `class` or `record` that `implements` a generic contract (e.g., `Iterable<T>`) SHALL have its contract methods lowered with the type parameters substituted at each call site.

#### Scenario: Generic iterable class
- **WHEN** `class Box<T> implements Iterable<T>` declares `iterator(): Iterator<T>`
- **THEN** `for x in Box<Int32>()` compiles and yields `Int32` values

#### Scenario: Generic iterable record
- **WHEN** `record Pair<T, U> implements Iterable<T>` declares a custom iterator
- **THEN** `for x in Pair<Int32, String>()` compiles and yields the first `Int32` element
