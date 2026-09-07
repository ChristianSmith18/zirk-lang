# Delta spec: zirk-contracts

## MODIFIED Requirements

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
