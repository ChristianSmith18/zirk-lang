## REMOVED Requirements

### Requirement: Typed runtime values preserve Zirk semantics
**Reason**: The current value list includes records and relies on structural equality for them.
**Migration**: Represent dynamic objects with reference identity and tagged properties; keep the remaining scalar, aggregate, callable, and collection semantics.

### Requirement: Object-oriented programs execute with native-equivalent behavior
**Reason**: The requirement names interfaces and records as delivered object-model constructs.
**Migration**: Execute contracts, dynamic objects, classes, abstract classes, and traits with matching native semantics.

## ADDED Requirements

### Requirement: Interpreter values preserve dynamic object semantics
The interpreter SHALL represent every supported scalar, nullable, callable, closure, dynamic object, contract view, enum, string, regex, range, and collection value without coercing it to host JavaScript semantics. Dynamic `Object` values SHALL have stable identity and tagged string-keyed properties. Property reads, writes, safe lookups, key sealing, contract checks, spread, and rest SHALL match the native implementation. `Dynamic` values SHALL preserve their runtime tags and require the same checked narrowing as native execution.

#### Scenario: Dynamic object mutation matches native execution
- **WHEN** an interpreted program adds a property and reads it back
- **THEN** its value and identity behavior match native execution

#### Scenario: Runtime contract mismatch is catchable
- **WHEN** an interpreted dynamic object fails a contract check inside `try`
- **THEN** the same typed contract error is caught as in native execution

### Requirement: Contract and object programs execute with native-equivalent behavior
The interpreter SHALL execute the supported object-oriented surface, including classes, constructors, fields, inheritance, overrides, abstract classes, contracts, traits, static and nested classes, generic class methods, callable object properties, open dynamic objects, class instances with dynamic properties, and alias-visible key sealing with the same output, completion, and failure category as the native pipeline. Browser-support claims SHALL continue to require shared conformance fixtures.

#### Scenario: Contract dispatch uses the underlying value
- **WHEN** an interpreted class or object is called through a compatible contract view
- **THEN** dispatch invokes the compatible class method or stored callable property

#### Scenario: Sealed object remains sealed through aliases
- **WHEN** a dynamic object is assigned to a contract view and later accessed through another alias
- **THEN** no alias can add or remove keys
