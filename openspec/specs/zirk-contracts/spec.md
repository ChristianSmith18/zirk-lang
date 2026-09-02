# zirk-contracts Specification

## Purpose
Defines interfaces, traits, abstract requirement classes, capability contracts,
conformance, conflict resolution, and dynamic contract dispatch.
## Requirements
### Requirement: Interface, trait, and abstract requirements
Interfaces SHALL contain behavior signatures only. Traits SHALL contain behavior requirements and reusable method bodies but no attributes or constructors. Abstract classes SHALL contain nominal attribute and abstract-method requirements but no bodies. All three SHALL be adopted through `implements`; a declaration SHALL satisfy every compatible requirement explicitly.

#### Scenario: Trait state rejected
- **WHEN** a trait declares an instance attribute
- **THEN** compilation fails and recommends a required getter/setter method or abstract-class requirement

### Requirement: Contract composition and conflict resolution
Interfaces MAY implement interfaces; traits MAY implement interfaces and traits; abstract classes MAY implement abstract classes and interfaces. Cycles and incompatible same-name signatures SHALL fail. Conflicting trait defaults SHALL require `override fn` and MAY select an implementation with `TraitName.super.method()`.

#### Scenario: Explicit trait selection
- **WHEN** two adopted traits define `print()` and the class overrides it using `JsonPrintable.super.print()`
- **THEN** the selected default is called without declaration-order precedence

### Requirement: Explicit capability derivation
Classes SHALL NOT derive equality, hashing, or cloning silently. Records, value classes, tuples, and enums MAY request explicit derivation only when every component satisfies the required capability; derived cloning SHALL be deep.

#### Scenario: Uncloneable component
- **WHEN** Clone derivation is requested for a value containing an uncloneable resource
- **THEN** compilation fails and identifies the component and missing capability

### Requirement: Interfaces

An `interface` SHALL declare signatures without implementation, and a class SHALL be able to combine several, per `ZIRK_LANGUAGE_SPEC.md` section 7. A `record` that implements an interface SHALL dispatch correctly through a reference typed by that interface, without exposing observable identity or mutation through that reference.

#### Scenario: Class implementing an interface
- **WHEN** `class User implements Serializable` defines every method of the interface
- **THEN** the check succeeds
- **AND** `User` is acceptable wherever `Serializable` is expected

#### Scenario: Unimplemented interface method
- **WHEN** a class declares that it implements an interface and omits a method
- **THEN** a diagnostic is emitted naming the missing method and its signature

#### Scenario: Record dispatches through an implemented interface
- **WHEN** a `record` implements an interface and is held through a variable typed as that interface
- **THEN** a call through the interface-typed reference dispatches to the record's own method, without granting the value observable identity or mutability through that reference

#### Scenario: Interface with a body
- **WHEN** a method of an `interface` declares a body
- **THEN** a diagnostic is emitted indicating that reusable implementation belongs in a `trait`

### Requirement: Traits with implementation

A `trait` SHALL be able to include methods with a body, which the class that adopts it receives unless it overrides them.

#### Scenario: Method inherited from the trait
- **WHEN** a class adopts a trait with an implemented method and does not override it
- **THEN** calling that method on the class executes the trait's body

#### Scenario: The class overrides the trait method
- **WHEN** the class defines its own version
- **THEN** the class's version is executed

#### Scenario: Two traits provide the same method
- **WHEN** a class adopts two traits that implement a method with the same name and does not override it
- **THEN** a diagnostic is emitted naming both traits
- **AND** the help indicates that overriding it in the class resolves the conflict

### Requirement: Operator contracts

Operator overloading SHALL occur only by implementing the contract the language defines for it, and SHALL NOT alter its precedence or arity, per `ZIRK_LANGUAGE_SPEC.md` section 4.

Contracts use reserved methods such as `_add` and `_subtract`. User-defined types MAY implement them in safe code; native types SHALL NOT be reopenable from application code.

`String` SHALL implement native concatenation and checked repetition contracts:
`String * Integer` and `Integer * String` return a new String, reject negative
counts, and diagnose unrepresentable allocation sizes.

#### Scenario: String concatenation
- **WHEN** `"a" + "b"` is evaluated
- **THEN** the result is `"ab"`
- **AND** it resolves via the contract that `String` implements, not via a special-cased operator

#### Scenario: Operator on a type that does not implement it
- **WHEN** `+` is applied to a type that does not implement the contract
- **THEN** a diagnostic is emitted naming the missing contract

#### Scenario: Custom type implementing the contract
- **WHEN** a class implements the addition contract and two of its instances are written with `+`
- **THEN** its implementation is invoked

### Requirement: Iteration via contract

`for ... in` SHALL require that the iterated expression implement `Iterable<T>`, and the element SHALL have the type `T` that the contract declares.

This removes the closed protocol from the previous phase: ranges and `String` now implement the contract instead of being cases the compiler recognizes specially.

#### Scenario: Iterating a custom type
- **WHEN** a class implements `Iterable<Int32>` and `for x in instance { }` is written
- **THEN** `x` has type `Int32`
- **AND** the loop iterates over what the iterator produces

#### Scenario: Ranges keep working
- **WHEN** `for i in 0..10 { }` is written
- **THEN** it compiles and iterates the same as before
- **AND** it does so because the range implements `Iterable<Int32>`

#### Scenario: Type that does not implement the contract
- **WHEN** a type that does not implement `Iterable<T>` is iterated
- **THEN** a diagnostic is emitted naming the missing contract

### Requirement: A contract cannot be partially satisfied

A declaration that claims to implement a contract SHALL implement it completely before being accepted.

#### Scenario: Partial implementation
- **WHEN** a class implements two of the three methods of a contract
- **THEN** a diagnostic is emitted for each missing method
