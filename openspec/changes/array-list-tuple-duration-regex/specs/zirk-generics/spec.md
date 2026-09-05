# zirk-generics Specification

## Purpose

Defines generic type parameter substitution, generic contract satisfaction, and the lowering of user-defined generic `implements` declarations.

## ADDED Requirements

### Requirement: Generic contract method lowering

When a generic `class` or `record` declares `implements Contract<T, ...>`, the compiler SHALL lower the contract methods with the generic parameters substituted at the call site. The generated vtable entries SHALL support the substituted signatures.

#### Scenario: Generic iterable class
- **WHEN** `class Box<T> implements Iterable<T> { fn iterator(): Iterator<T> { ... } }` is declared
- **THEN** `for x in Box<Int32>()` compiles, the `iterator()` method is lowered with `T = Int32`, and `x` is `Int32`

#### Scenario: Generic iterable record
- **WHEN** `record Pair<T, U> implements Iterable<T> { ... }` is declared
- **THEN** `for x in Pair<Int32, String>()` yields `Int32` values

### Requirement: Generic contract substitution does not corrupt the vtable

The vtable layout for a generic `class`/`record` SHALL remain stable across different instantiations. Contract method offsets SHALL be the same for `Box<Int32>` and `Box<String>`.

#### Scenario: Vtable layout stability
- **WHEN** `Box<Int32>` and `Box<String>` are both instantiated and implement `Iterable<T>`
- **THEN** the `iterator` method offset is identical in both vtables

### Requirement: Recursion cap on generic substitution

Generic substitution SHALL be guarded against infinite recursion. Mutually recursive generic `class`/`record` definitions and contract instantiations SHALL produce a controlled diagnostic instead of stack overflow.

#### Scenario: Recursive generic alias
- **WHEN** `type Nested<T> = Box<Nested<T>>;` is written
- **THEN** a diagnostic is emitted before the compiler recurses unboundedly
