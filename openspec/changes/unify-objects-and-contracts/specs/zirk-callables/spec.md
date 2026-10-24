## REMOVED Requirements

### Requirement: Escaping captured closures are boxed
**Reason**: The current requirement names record fields as the only non-class user storage location for a closure.
**Migration**: Store callable values in dynamic object properties; captured environments remain managed references.

### Requirement: Callable captures remain visible to managed-memory tracing
**Reason**: The current aggregate list includes the removed record type and omits dynamic object properties.
**Migration**: Store closures in `Object` properties or supported aggregates; all capture edges remain traceable.

## ADDED Requirements

### Requirement: Callable values may be stored in dynamic object properties
An `Object` SHALL store callable values as tagged property values. Escaping closures SHALL use a managed capture environment and remain invocable after their creation frame exits. Captured references SHALL remain visible to managed-memory tracing when a closure is stored in an object or supported aggregate. The callable SHALL satisfy a contract operation when its signature and effects are compatible.

#### Scenario: Escaping closure is stored in an object
- **WHEN** an object stores a closure returned from another function and the closure captures a managed value
- **THEN** the closure remains callable and the captured value remains reachable

#### Scenario: Stored callable satisfies a contract operation
- **WHEN** a dynamic object stores a callable whose signature matches a contract operation
- **THEN** runtime conformance succeeds and the operation dispatches to the stored callable

#### Scenario: Stored closure remains rooted in a collection
- **WHEN** a collection contains an object whose only path to a captured object is a callable property
- **THEN** the captured object remains reachable during collection
