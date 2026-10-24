## REMOVED Requirements

### Requirement: Object layout
**Reason**: The layout includes inline record fields and does not account for dynamic properties.
**Migration**: Keep fixed class layouts and represent open object properties through the dynamic object runtime representation.

### Requirement: Native code generation supports value-type field pointers
**Reason**: This pointer rule exists for inline record storage, which is removed.
**Migration**: Preserve supported pointers for ABI-safe fields and class layouts; dynamic properties are accessed through checked object operations.

## ADDED Requirements

### Requirement: Native code generation supports dynamic objects and contract checks
The native backend SHALL emit operations for dynamic object creation, string-keyed property access, writes, safe lookup, spread, rest, and contract conformance. Contract checks and missing-property failures SHALL enter the ordinary catchable runtime error path. Dynamic values SHALL preserve runtime tags and managed-reference metadata.

#### Scenario: Read a typed contract property
- **WHEN** native code reads a property through a contract-typed object
- **THEN** it uses the checked contract member type and the underlying object identity

#### Scenario: Read a dynamic property
- **WHEN** native code reads a property from `Object` without a known key type
- **THEN** it returns a tagged `Dynamic` value with its runtime type preserved

#### Scenario: Contract validation failure is catchable
- **WHEN** a runtime contract check finds a missing or incompatible requirement
- **THEN** generated code raises the contract error through the normal exception mechanism
