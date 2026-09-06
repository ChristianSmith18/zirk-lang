# Delta spec: zirk-generics

## ADDED Requirements

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
