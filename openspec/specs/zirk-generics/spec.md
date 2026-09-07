# zirk-generics Specification

## Purpose
Defines generic parameters, constraints, defaults, inference, variance,
specialization, recursion, and runtime type identity.
## Requirements

### Requirement: Generic declarations, constraints, defaults, and inference

Generic parameters SHALL use `<T>`, combined constraints SHALL use `T from A & B`, and trailing parameters MAY declare defaults satisfying their constraints. Inference SHALL use arguments, receiver, expected result, callable context, and constraints and SHALL fail rather than choose an arbitrary solution.

#### Scenario: Multiple missing constraints

- **WHEN** a concrete type is supplied for `T from Clone & Serializable` and lacks Serializable
- **THEN** the call-site diagnostic identifies T and the missing Serializable contract

### Requirement: Declared generic variance

Generic parameters SHALL be invariant by default. `out T` SHALL be legal only in covariant output positions, `in T` only in contravariant input positions, and any mutable attribute use SHALL require invariance. `Fn` SHALL retain its intrinsic parameter/result variance.

#### Scenario: Mutable List invariance

- **WHEN** Dog extends Animal and `List<Dog>` is supplied as `List<Animal>`
- **THEN** compilation fails because List permits insertion

### Requirement: Recursive generics and managed indirection

Verifiable recursive constraints SHALL be accepted. Infinite inline recursive layout SHALL fail and MAY be broken with managed `Box<T>`. Associated types and higher-kinded parameters SHALL remain outside the initial language.

#### Scenario: Recursive record requires Box

- **WHEN** a record directly stores its own type without indirection
- **THEN** compilation fails with an infinite-size diagnostic and suggests `Box<T>`

### Requirement: Generic identity and monomorphization

Concrete instantiations SHALL retain distinct static and runtime type identity. Generic bodies SHALL be checked once against constraints; portable IR SHALL retain generic information and final builds MAY monomorphize/share code only when ABI and observable semantics remain unchanged.

#### Scenario: Distinct runtime instantiations

- **WHEN** runtime type identity is requested for List<String> and List<Int32>
- **THEN** the two complete instantiations remain distinguishable

### Requirement: Type parameters

Functions, classes, and data types SHALL support type parameters with the `<T>` syntax, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Generic function

- **WHEN** `fn identity<T>(value: T): T { return value; }` is declared
- **THEN** `identity(1)` produces an `Int32` and `identity("a")` a `String`

#### Scenario: Generic class

- **WHEN** `class Box<T>` is declared with a field of type `T`
- **THEN** `Box<Int32>` and `Box<String>` are distinct types

#### Scenario: Unused type parameter

- **WHEN** a type parameter that does not appear in the signature is declared
- **THEN** a diagnostic naming it is emitted

### Requirement: Constraints with `from`

A type parameter SHALL support constraints with `from`, and the checker SHALL verify them at the use site.

#### Scenario: Argument satisfying the constraint

- **WHEN** `serialize<T from Serializable>` is called with a type that implements `Serializable`
- **THEN** the check succeeds

#### Scenario: Argument that does not satisfy it

- **WHEN** it is called with a type that does not implement the contract
- **THEN** a diagnostic naming the concrete type and the missing contract is emitted

#### Scenario: The body only uses what the constraint guarantees

- **WHEN** the body of a generic function calls a method that the constraint does not declare
- **THEN** a diagnostic is emitted
- **AND** the help indicates that the constraint must declare it

### Requirement: The generic is checked once

The checker SHALL verify the body of a generic declaration exactly once against its constraints, not once per instantiation.

#### Scenario: Error in the body is reported once

- **WHEN** a generic function with a type error is instantiated with three distinct types
- **THEN** the error is reported exactly once, at the declaration

### Requirement: Specialization during lowering

Lowering SHALL produce one copy per combination of type arguments actually used.

This is what `ZIRK_LANGUAGE_SPEC.md` section 7 calls specializing where appropriate, and it is what allows storing records inline instead of behind a pointer.

#### Scenario: Two instantiations, two copies

- **WHEN** `identity` is used with `Int32` and with `String`
- **THEN** the IR contains one function for each

#### Scenario: Repeated instantiation

- **WHEN** the same combination of types is used multiple times
- **THEN** the IR contains a single copy

### Requirement: Scope of generics in this phase

The checker SHALL NOT support associated types or higher-kinded type
parameters. Declared variance (`in T` / `out T`) is part of the language and
SHALL be verified per "Declared generic variance".

None of the remaining exclusions are in `ZIRK_LANGUAGE_SPEC.md`, and
supporting them would fix semantics that the spec does not fix.

#### Scenario: Unsupported construct

- **WHEN** an associated type or a higher-kinded type parameter is written
- **THEN** a diagnostic indicating that it is not part of the language is emitted

### Requirement: Generic contract method lowering

When a generic `class` or `record` declares `implements Contract<T, ...>`, the compiler SHALL lower the contract methods with the generic parameters substituted at the call site. The generated vtable entries SHALL support the substituted signatures.

#### Scenario: Generic iterable class

- **WHEN** `class Box<T> implements Iterable<T> { iterator(): Iterator<T> { ... } }` is declared
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

### Requirement: Dispatch through a generic contract reference

A call through a receiver of generic contract type SHALL dispatch to the
concrete adopter's method lowered under that instantiation's substitution.
The observable result SHALL be identical to calling the adopter's method
directly.

#### Scenario: Call through generic contract reference

- **WHEN** a value of `Box<Int32>` (which implements `Iterable<T>`) is held through `Iterable<Int32>` and `iterator()` is invoked
- **THEN** the call dispatches to `Box<Int32>`'s own `iterator()` implementation

#### Scenario: Generic contract as a parameter type

- **WHEN** a function declares a parameter of type `Iterable<String>` and is called with a `Box<String>`
- **THEN** contract calls inside the function dispatch to `Box<String>`'s methods
