## REMOVED Requirements

### Requirement: Lowering of spread and rest operators
**Reason**: This requirement lowers object/record spread as field-by-field copies of statically shaped values.
**Migration**: Lower object spread and rest as operations on open dynamic objects; retain iterable spread lowering for collections and calls.

### Requirement: Deterministic object spread lowering
**Reason**: Its record-typed assumptions no longer describe the dynamic object model.
**Migration**: Preserve insertion order and explicit override behavior for dynamic object spread.

## ADDED Requirements

### Requirement: Lowering dynamic object operations
The typed IR SHALL represent dynamic object creation, tagged property reads and writes, key removal, safe lookup, spread, rest, and contract conformance as explicit operations. Runtime checks SHALL preserve source locations and produce catchable errors. Contract views SHALL retain the underlying object reference rather than copy the object. Existing call and collection spread SHALL continue to lower as iterable expansion.

#### Scenario: Lower interpolated object key
- **WHEN** an object literal contains an interpolated key expression
- **THEN** lowering evaluates the key once and stores its resulting string with the tagged value

#### Scenario: Lower runtime contract check
- **WHEN** a value with only dynamic `Object` type is assigned to a contract
- **THEN** the IR validates required properties and operations before exposing the contract view

#### Scenario: Lower safe property lookup
- **WHEN** a program performs explicit presence-safe lookup
- **THEN** the IR returns an absent result for a missing key without emitting a throwing access

### Requirement: Dynamic object spread creates a shallow copy
Lowered object spread SHALL create a distinct object identity, copy property values in insertion order, and then apply explicit overrides. Nested managed references SHALL preserve their identities. Rest lowering SHALL copy all keys not selected by the pattern.

#### Scenario: Spread source and result have separate identities
- **WHEN** a dynamic object is spread into a new object
- **THEN** the result has a new identity and copied nested references remain shared
