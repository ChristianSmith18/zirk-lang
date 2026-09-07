# zirk-generics Delta Spec

## MODIFIED Requirements

### Requirement: Generic contract method lowering

When a generic `class` or `record` declares `implements Contract<T, ...>`, the compiler SHALL lower the contract methods with the generic parameters substituted at the call site. The generated vtable entries SHALL support the substituted signatures.

#### Scenario: Generic iterable class
- **WHEN** `class Box<T> implements Iterable<T> { iterator(): Iterator<T> { ... } }` is declared
- **THEN** `for x in Box<Int32>()` compiles, the `iterator()` method is lowered with `T = Int32`, and `x` is `Int32`

#### Scenario: Generic iterable record
- **WHEN** `record Pair<T, U> implements Iterable<T> { ... }` is declared
- **THEN** `for x in Pair<Int32, String>()` yields `Int32` values
