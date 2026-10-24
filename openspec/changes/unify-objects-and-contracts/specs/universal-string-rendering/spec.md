## REMOVED Requirements

### Requirement: Structured, enum, callable, and handle rendering
**Reason**: This requirement includes record rendering and does not define rendering for open dynamic objects.
**Migration**: Use the new default object rendering or provide an explicit `to_string()` implementation.

## ADDED Requirements

### Requirement: Dynamic objects have bounded deterministic rendering
An `Object` without an explicit `to_string()` implementation SHALL render as a readable object form using its string keys in deterministic insertion order. Rendering SHALL be bounded, SHALL mark cycles and unsupported values without recursing indefinitely, and SHALL NOT be treated as JSON serialization. A contract or class implementation of `to_string()` SHALL take precedence through ordinary dispatch.

#### Scenario: Render an object with string keys
- **WHEN** an object with `status: "ok"` is printed
- **THEN** it produces a readable object form containing the key and value in deterministic order

#### Scenario: Render a cyclic object
- **WHEN** an object graph contains a cycle and is rendered
- **THEN** rendering terminates and marks the cycle rather than overflowing

#### Scenario: Explicit rendering wins
- **WHEN** a class or contract view provides a compatible `to_string()` implementation
- **THEN** ordinary dispatch uses that implementation

### Requirement: Non-object structured rendering remains stable
Classes, tuples, payload enum cases, `Result`, named and anonymous functions, runtime handles, weak references, and regular expressions SHALL retain their documented readable renderings and surface names. Dynamic `Object` rendering SHALL follow the separate bounded rule above and SHALL NOT be confused with JSON serialization.

#### Scenario: Result and enum rendering remain readable
- **WHEN** an `Ok` result or a payload enum case is converted to a string
- **THEN** it uses its documented variant-oriented form

#### Scenario: Runtime handles use stable type names
- **WHEN** a job, channel, pointer, dependent reference, pin, or weak reference is rendered
- **THEN** the output uses its documented handle form without exposing internal addresses
