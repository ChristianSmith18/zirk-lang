## ADDED Requirements

### Requirement: `Never` is the bottom type

The checker SHALL treat `Never` as assignable to any type, and as contributing nothing to the shared type at a branch join (`??`, `match`, `if`/ternary as an expression): the join's type SHALL be the other branch's type, never a nullable widening of it. No expression SHALL ever produce a runtime value of type `Never`; the only construct typed `Never` is a call to the compiler-known `fatalError(message: String): Never`, which never returns.

#### Scenario: `Never` is assignable anywhere
- **WHEN** `fatalError("unreachable")` is used where `Int32`, `String`, or `Boolean` is expected
- **THEN** the checker accepts it

#### Scenario: `Never` contributes nothing at a ternary join
- **WHEN** `cond ? 5 : fatalError("unreachable")` is checked
- **THEN** its type is `Int32`, not `Int32?` and not `Never`

#### Scenario: An enum with no variants is rejected
- **WHEN** `enum Impossible { }` is declared
- **THEN** the checker rejects it and names `Never` as the type that already means "no value"
