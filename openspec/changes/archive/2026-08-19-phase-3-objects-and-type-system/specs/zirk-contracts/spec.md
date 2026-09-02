## ADDED Requirements

### Requirement: Interfaces

An `interface` SHALL declare signatures with no implementation, and a class SHALL be able to combine several, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Class implementing an interface
- **WHEN** `class User implements Serializable` defines every method of the interface
- **THEN** the check succeeds
- **AND** `User` is acceptable where `Serializable` is expected

#### Scenario: Unimplemented interface method
- **WHEN** a class declares that it implements an interface and omits a method
- **THEN** a diagnostic naming the missing method and its signature is emitted

#### Scenario: Interface with a body
- **WHEN** a method of an `interface` declares a body
- **THEN** a diagnostic stating that reusable implementation belongs to a `trait` is emitted

### Requirement: Traits with implementation

A `trait` SHALL be able to include methods with a body, which the class that adopts it receives unless it overrides them.

#### Scenario: Method inherited from the trait
- **WHEN** a class adopts a trait with an implemented method and does not override it
- **THEN** calling that method on the class runs the trait's body

#### Scenario: The class overrides the trait's method
- **WHEN** the class defines its own version
- **THEN** the class's version runs

#### Scenario: Two traits contribute the same method
- **WHEN** a class adopts two traits that implement a method with the same name and does not override it
- **THEN** a diagnostic naming both traits is emitted
- **AND** the hint states that overriding it in the class resolves the conflict

### Requirement: Operator contracts

Overloading an operator SHALL happen only by implementing the contract the language defines for it, and SHALL NOT alter its precedence or arity, per `ZIRK_LANGUAGE_SPEC.md` section 4.

Contracts use reserved methods such as `_add` and `_subtract`. User-defined types MAY implement them in safe code; native types SHALL NOT be reopenable from application code.

`String` SHALL implement native concatenation and checked repetition contracts:
`String * Integer` and `Integer * String` return a new String, reject negative
counts, and diagnose unrepresentable allocation sizes.

#### Scenario: String concatenation
- **WHEN** `"a" + "b"` is evaluated
- **THEN** the result is `"ab"`
- **AND** it resolves through the contract `String` implements, not through a special-cased operator

#### Scenario: Operator on a type that does not implement it
- **WHEN** `+` is applied to a type that does not implement the contract
- **THEN** a diagnostic naming the missing contract is emitted

#### Scenario: User type implementing the contract
- **WHEN** a class implements the addition contract and two of its instances are written with `+`
- **THEN** its implementation is invoked

### Requirement: Iteration by contract

`for ... in` SHALL require the iterated expression to implement `Iterable<T>`, and the element SHALL have the type `T` the contract declares.

This retires the closed protocol from the previous phase: ranges and `String` now implement the contract instead of being cases the compiler recognizes.

#### Scenario: Iterating a user type
- **WHEN** a class implements `Iterable<Int32>` and `for x in instance { }` is written
- **THEN** `x` has type `Int32`
- **AND** the loop walks whatever the iterator produces

#### Scenario: Ranges keep working
- **WHEN** `for i in 0..10 { }` is written
- **THEN** it compiles and iterates the same as before
- **AND** it does so because the range implements `Iterable<Int32>`

#### Scenario: Type that does not implement the contract
- **WHEN** a type that does not implement `Iterable<T>` is iterated
- **THEN** a diagnostic naming the missing contract is emitted

### Requirement: A contract is not satisfied halfway

A declaration that claims to implement a contract SHALL implement it fully before being accepted.

#### Scenario: Partial implementation
- **WHEN** a class implements two of a contract's three methods
- **THEN** a diagnostic is emitted for each missing method
