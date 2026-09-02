## MODIFIED Requirements

### Requirement: Interfaces

An `interface` SHALL declare signatures without implementation, and a class SHALL be able to combine several, per `ZIRK_LANGUAGE_SPEC.md` section 7. A `record` that implements an interface SHALL dispatch correctly through a reference typed by that interface, without exposing observable identity or mutation through that reference.

#### Scenario: Class implementing an interface
- **WHEN** `class User implements Serializable` defines all of the interface's methods
- **THEN** checking succeeds
- **AND** `User` is acceptable where `Serializable` is expected

#### Scenario: Interface method left unimplemented
- **WHEN** a class declares that it implements an interface and omits a method
- **THEN** a diagnostic is emitted naming the missing method and its signature

#### Scenario: Record dispatches through an implemented interface
- **WHEN** a `record` implements an interface and is held through a variable typed as that interface
- **THEN** a call through the interface-typed reference dispatches to the record's own method, without granting the value observable identity or mutability through that reference

NOTE (not applied to the main spec as a `value class` scenario): `value class`'s own compact declaration grammar has no `implements` clause and no method-body syntax at all today, independent of this change — so a `value class` cannot yet satisfy a contract regardless of this dispatch mechanism now working for `record`. Extending `value class`'s grammar is separate, future work.
