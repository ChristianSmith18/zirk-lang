# zirk-contracts Delta Spec

## MODIFIED Requirements

### Requirement: Contract composition and conflict resolution
Interfaces MAY implement interfaces; traits MAY implement interfaces and traits; abstract classes MAY implement abstract classes and interfaces. Cycles and incompatible same-name signatures SHALL fail. Conflicting trait defaults SHALL require the `#override` marker (since a trait default is an inherited implementation) and MAY select an implementation with `TraitName.super.method()`.

#### Scenario: Explicit trait selection
- **WHEN** two adopted traits define `print()` and the class overrides it using `JsonPrintable.super.print()` under a `#override` marker
- **THEN** the selected default is called without declaration-order precedence

#### Scenario: Interface requirement needs no marker
- **WHEN** a class implements an `interface` method and writes no `#override`
- **THEN** the implementation is accepted, since no inherited implementation is replaced

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
