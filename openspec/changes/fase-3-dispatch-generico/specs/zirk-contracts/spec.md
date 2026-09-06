# Delta spec: zirk-contracts

## ADDED Requirements

### Requirement: Generic contract conformance

A `class` or `record` SHALL be able to declare conformance to a generic
contract instantiation (`implements Contract<T, ...>`). Signature
compatibility SHALL be checked against the contract's methods after
substituting the contract's own type parameters with the arguments written
in the `implements` clause, and trait default methods SHALL apply under the
same substitution.

#### Scenario: Signature checked under substitution
- **WHEN** `class Box<T> implements Iterable<T>` declares `fn iterator(): Iterator<T>`
- **THEN** the signature is compared against `Iterable`'s `iterator` with the contract parameter replaced by the adopter's `T`

#### Scenario: Missing method under substitution
- **WHEN** a class declares `implements Comparable<Int32>` without the required method at the substituted signature
- **THEN** a `MISSING_IMPLEMENTATION` diagnostic is emitted
