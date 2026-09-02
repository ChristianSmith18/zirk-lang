## ADDED Requirements

### Requirement: Type parameters

Functions, classes, and data types SHALL admit type parameters with the `<T>` syntax, per `ZIRK_LANGUAGE_SPEC.md` section 7.

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

A type parameter SHALL admit constraints with `from`, and the checker SHALL verify them at the use site.

#### Scenario: Argument satisfying the constraint
- **WHEN** `serialize<T from Serializable>` is called with a type that implements `Serializable`
- **THEN** the check succeeds

#### Scenario: Argument not satisfying it
- **WHEN** it is called with a type that does not implement the contract
- **THEN** a diagnostic naming the concrete type and the missing contract is emitted

#### Scenario: The body only uses what the constraint guarantees
- **WHEN** a generic function's body calls a method the constraint does not declare
- **THEN** a diagnostic is emitted
- **AND** the hint states that the constraint must declare it

### Requirement: The generic is checked once

The checker SHALL verify a generic declaration's body only once against its constraints, not once per instantiation.

#### Scenario: An error in the body is reported once
- **WHEN** a generic function with a type error is instantiated with three different types
- **THEN** the error is reported only once, at the declaration

### Requirement: Specialization at lowering

Lowering SHALL produce a copy per combination of type arguments actually used.

This is what `ZIRK_LANGUAGE_SPEC.md` section 7 calls specializing where appropriate, and what allows storing value classes inline instead of behind a pointer.

#### Scenario: Two instantiations, two copies
- **WHEN** `identity` is used with `Int32` and with `String`
- **THEN** the IR contains one function for each

#### Scenario: Repeated instantiation
- **WHEN** the same type combination is used several times
- **THEN** the IR contains a single copy

### Requirement: Scope of generics in this phase

The checker SHALL NOT admit declared variance, associated types, or higher-order type parameters.

None of these are in `ZIRK_LANGUAGE_SPEC.md`, and admitting them would fix semantics the spec does not fix.

#### Scenario: Unsupported construct
- **WHEN** a variance annotation is written
- **THEN** a diagnostic stating that it is not part of the language is emitted
