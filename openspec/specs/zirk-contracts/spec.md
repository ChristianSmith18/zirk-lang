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

Interfaces MAY implement interfaces; traits MAY implement interfaces and traits; abstract classes MAY implement abstract classes and interfaces. Cycles and incompatible same-name signatures SHALL fail. Conflicting trait defaults SHALL require the `#override` marker (since a trait default is an inherited implementation) and MAY select an implementation with `TraitName.super.method()`.

#### Scenario: Explicit trait selection

- **WHEN** two adopted traits define `print()` and the class overrides it using `JsonPrintable.super.print()` under a `#override` marker
- **THEN** the selected default is called without declaration-order precedence

#### Scenario: Interface requirement needs no marker

- **WHEN** a class implements an `interface` method and writes no `#override`
- **THEN** the implementation is accepted, since no inherited implementation is replaced

### Requirement: Capability derivation model

Classes SHALL NOT gain equality, hashing, or cloning silently: equality on
classes requires an explicit `_equals` implementation, and cloning requires
the type to satisfy the `Clone` capability. Records, tuples, and enums have
structural equality by definition, and `clone` SHALL be available for any
type whose complete reachable type graph is cloneable — no explicit
derivation syntax is required; derived cloning SHALL be deep.

#### Scenario: Uncloneable component

- **WHEN** cloning is attempted on a value containing an uncloneable resource
- **THEN** compilation fails and identifies the component and missing capability

#### Scenario: Record equality is automatic

- **WHEN** two `record` values of the same type are compared with `==`
- **THEN** structural field equality applies without any declared derivation

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

Contracts use reserved methods: `_add` (`+`), `_subtract` (`-`), `_multiply` (`*`), `_divide` (`/`), `_remainder` (`%`), `_equals` (`==`, and `!=` as its negation), `_less` (`<`), `_less_equal` (`<=`), `_greater` (`>`), and `_greater_equal` (`>=`). User-defined types MAY implement them in safe code; native types SHALL NOT be reopenable from application code.

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

#### Scenario: Comparison through a contract

- **WHEN** a class implements `_less` and two of its instances are written with `<`
- **THEN** its implementation is invoked and the result is `Boolean`

#### Scenario: Inequality from equality

- **WHEN** a class implements `_equals` and two of its instances are written with `!=`
- **THEN** the result is the negation of `_equals`, with no separate method required

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

### Requirement: Generic contract conformance

A `class` or `record` SHALL be able to declare conformance to a generic
contract instantiation (`implements Contract<T, ...>`). Signature
compatibility SHALL be checked against the contract's methods after
substituting the contract's own type parameters with the arguments written
in the `implements` clause, and trait default methods SHALL apply under the
same substitution.

#### Scenario: Signature checked under substitution

- **WHEN** `class Box<T> implements Iterable<T>` declares `iterator(): Iterator<T>`
- **THEN** the signature is compared against `Iterable`'s `iterator` with the contract parameter replaced by the adopter's `T`

#### Scenario: Missing method under substitution

- **WHEN** a class declares `implements Comparable<Int32>` without the required method at the substituted signature
- **THEN** a `MISSING_IMPLEMENTATION` diagnostic is emitted
