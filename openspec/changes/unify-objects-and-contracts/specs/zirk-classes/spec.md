## REMOVED Requirements

### Requirement: Concrete inheritance and abstract implementation
**Reason**: The existing rule names interfaces as a separate class implementation target.
**Migration**: Classes implement structural contracts and retain nominal abstract-class inheritance.

### Requirement: Explicit overriding and super dispatch
**Reason**: The existing override rule names interface signatures as a separate requirement category.
**Migration**: `#override` remains required only when replacing an inherited implementation; implementing a contract signature requires no marker.

### Requirement: Static members
**Reason**: The requirement leaves static declarations on records unresolved even though records are removed.
**Migration**: Static declarations remain class-only unless a future change explicitly broadens them.

## ADDED Requirements

### Requirement: Classes implement structural contracts
Classes SHALL be able to implement multiple contracts by providing compatible required properties and operations. Contract implementation SHALL not alter class inheritance, abstract-class conformance, or trait default conflict rules. Implementing a contract operation SHALL NOT require `#override` unless it also replaces an inherited implementation.

#### Scenario: Class implements a contract
- **WHEN** a class implements a contract and declares every required member
- **THEN** the class is accepted through that contract type

#### Scenario: Contract implementation does not require override marker
- **WHEN** a class supplies a contract operation without replacing an inherited implementation
- **THEN** the implementation is accepted without `#override`

### Requirement: Static members remain class-only
Classes MAY declare static methods and fields. Static members SHALL be accessed through the class name, SHALL NOT dispatch virtually or appear in instance contract tables, and SHALL NOT use `this`. Visibility and mutability rules SHALL apply as for other class members.

#### Scenario: Static method resolves on the class
- **WHEN** `Counter.make()` calls a declared static method
- **THEN** dispatch is static and requires no instance

#### Scenario: Static body rejects this
- **WHEN** a static method references `this`
- **THEN** the checker emits a diagnostic

### Requirement: Abstract classes remain nominal requirements
An abstract class SHALL define nominal required attributes and abstract method signatures without constructor, method body, state allocation, or layout contribution. A concrete subclass SHALL implement those requirements and SHALL dispatch calls through an abstract-class view to its concrete implementation. Classes MAY extend one concrete class.

#### Scenario: Concrete subclass satisfies abstract requirements
- **WHEN** a subclass implements every abstract method and required attribute
- **THEN** it can be used through the abstract-class type

### Requirement: Override markers select inherited implementations
`#override` SHALL be required only when replacing an inherited class implementation or trait default. Implementing a contract signature without replacing inherited behavior SHALL NOT require the marker. Inherited calls through `super` and explicit trait-default selection SHALL retain their existing behavior.

#### Scenario: Contract method needs no override marker
- **WHEN** a class supplies a contract operation that has no inherited implementation
- **THEN** it compiles without `#override`

### Requirement: Class instances support dynamic properties
Class instances SHALL support dynamic properties in addition to declared class fields. Unknown named access SHALL resolve to a dynamic property, and computed string access SHALL read or write dynamic properties. Dynamic property addition, replacement, and removal SHALL follow the instance reference's mutability and SHALL be disallowed after contract conformance seals the instance's key set. Declared class fields SHALL retain their declared types and visibility rules.

#### Scenario: Mutable class instance adds a dynamic property
- **WHEN** a mutable class instance receives a key not declared as a class field
- **THEN** that key is stored as a dynamic property without changing the class's declared layout or members

#### Scenario: Declared field keeps its declared type
- **WHEN** a dynamic property and a declared class field share the same spelling
- **THEN** declared-member resolution takes precedence and enforces the class field type

#### Scenario: Unknown member reads a dynamic property
- **WHEN** a class instance has a dynamic property not declared in its class
- **THEN** unknown-member access reads that property as `Dynamic` rather than producing a compile-time member-not-found error

#### Scenario: Contract seals class instance keys
- **WHEN** a class instance is successfully checked against a contract
- **THEN** its existing dynamic key set cannot be changed through any alias
