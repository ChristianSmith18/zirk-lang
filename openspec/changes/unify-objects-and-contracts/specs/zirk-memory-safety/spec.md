## REMOVED Requirements

### Requirement: inmut::strict applies to field declarations
**Reason**: The existing example uses a record declaration, which is removed.
**Migration**: Apply strict-field rules to class fields and dynamic object properties.

## ADDED Requirements

### Requirement: Strict field immutability applies to object properties
A property declared `inmut::strict` by a class or contract SHALL NOT be writable through any projection, regardless of whether its containing reference is `mut` or `inmut`.

#### Scenario: Strict contract property written through mutable object
- **WHEN** a mutable object conforms to a contract with an `inmut::strict` property and code writes that property
- **THEN** compilation fails with a strict-property write diagnostic

### Requirement: Dynamic object mutation follows reference mutability
Adding, replacing, or removing a dynamic property SHALL be treated as mutation of the referenced object and SHALL follow existing alias rules: `inmut` prevents rebinding but permits referent mutation, while `inmut::strict` prevents reachable mutation. A successful contract conformance SHALL seal the object's key set without changing its identity or bypassing strict-reference restrictions on property values.

#### Scenario: Strict alias prevents dynamic mutation
- **WHEN** a program adds a dynamic property while an `inmut::strict` alias to the same object remains accessible
- **THEN** the checker rejects the write under the existing mutable-alias rules

#### Scenario: Contract sealing is visible through aliases
- **WHEN** one alias successfully applies a contract to an object
- **THEN** every alias observes the sealed key set and cannot add or remove properties

#### Scenario: Nested references remain managed
- **WHEN** a dynamic property stores a managed object reference
- **THEN** ownership, lifetime, transfer, and collection rules apply to that reference as they do to other managed object fields
