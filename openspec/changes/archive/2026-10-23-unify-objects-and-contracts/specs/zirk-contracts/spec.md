## REMOVED Requirements

### Requirement: Interface, trait, and abstract requirements
**Reason**: Structural property requirements move into `contract`; retaining a separate interface role would duplicate the new contract model.
**Migration**: Replace `interface` declarations with `contract`. Keep `trait` for reusable method bodies and keep abstract classes nominal.

### Requirement: Contract composition and conflict resolution
**Reason**: The existing composition rules name interfaces as a distinct structural abstraction.
**Migration**: Compose contracts with contracts and traits with traits; retain the existing explicit trait-default conflict rules.

### Requirement: Capability derivation model
**Reason**: Records and their implicit structural equality are removed; ordinary objects have reference identity.
**Migration**: Use tuples and enums for structural value equality. Use `is` for object identity and explicit equality capabilities where needed.

### Requirement: Interfaces
**Reason**: `contract` replaces the interface declaration and is also able to declare required properties.
**Migration**: Declare required properties and operation signatures in a `contract`, then assign or implement that contract.

### Requirement: Generic contract conformance
**Reason**: This requirement includes record adopters and omits runtime conformance of dynamic objects.
**Migration**: Use classes or dynamic `Object` values as contract adopters; generic type arguments continue to be substituted before checking conformance.

### Requirement: Object spread does not require Iterable
**Reason**: Object spread is specified over the single dynamic object model in `spread-rest-operators`.
**Migration**: Use object field spread; collection and call spread continue to require `Iterable<T>`.

### Requirement: Traits with implementation
**Reason**: Trait implementation remains valid, but the old requirement is grouped with the removed interface distinction and does not define the new contract boundary.
**Migration**: Keep trait method bodies and default dispatch unchanged; use contracts only for required properties and operation signatures.

## ADDED Requirements

### Requirement: Contract conformance validates and seals dynamic objects
The checker SHALL verify statically known conformance at compile time. When the source has only dynamic `Object` type, conformance SHALL validate required properties and operations at runtime and raise a catchable contract violation if any requirement is missing or incompatible. Successful conformance SHALL seal the object's complete existing key set across all aliases. Extra keys SHALL remain stored and sealed; contract-typed access SHALL expose only declared properties and operations. Writes SHALL follow the contract property's mutability and the binding's mutability rules.

#### Scenario: Statically known object satisfies a contract
- **WHEN** an object literal with compatible required members is assigned to a contract type
- **THEN** conformance is checked at compile time and the object key set becomes sealed

#### Scenario: Dynamic object misses a required property
- **WHEN** a dynamic `Object` without a required property is assigned to a contract type
- **THEN** a catchable contract-violation error identifies the missing requirement

#### Scenario: Extra property is sealed across aliases
- **WHEN** an object with `name` and `active` is viewed through a contract requiring only `name`
- **THEN** `active` remains on the same object, no alias can add or remove keys, and the contract view exposes only `name`

### Requirement: Contract operations accept compatible callable properties
A dynamic object's callable property SHALL satisfy a contract operation when its parameter, result, mutability, safety, and effect requirements are compatible. Calls through the contract view SHALL use the callable value stored on the same object.

#### Scenario: Callable property satisfies an operation
- **WHEN** an object stores `label` as a compatible callable and is checked against a contract requiring `label(): String`
- **THEN** conformance succeeds and calling `label()` through the contract view invokes that stored callable

### Requirement: Contract and trait composition remain distinct
Contracts MAY extend other contracts, and classes MAY implement multiple contracts and traits. Traits MAY compose with traits and MAY implement contracts. Abstract classes MAY extend abstract classes and implement contracts. Cycles and incompatible same-name signatures SHALL fail. Conflicting trait defaults SHALL require explicit `#override` selection.

#### Scenario: Class implements multiple contracts
- **WHEN** a class implements two contracts and supplies every required member
- **THEN** both structural conformance checks succeed

#### Scenario: Conflicting trait defaults require selection
- **WHEN** two adopted traits provide the same method and the class does not resolve the conflict
- **THEN** a diagnostic identifies both trait defaults

### Requirement: Object identity does not imply structural equality
Dynamic objects SHALL use reference identity. Assignment and contract views SHALL preserve the same object identity; equality SHALL NOT recursively compare object properties unless an explicit equality capability is implemented.

#### Scenario: Two equal-looking objects have distinct identity
- **WHEN** two separately created objects have the same properties
- **THEN** structural equality is not inferred and `is` reports distinct identities

### Requirement: Value capabilities remain explicit
Classes and dynamic objects SHALL NOT gain equality or cloning silently. Equality SHALL require an explicit equality capability or method. Tuples and enums SHALL retain their documented component-wise value semantics; dynamic objects SHALL use identity unless an explicit equality implementation is selected. Deep cloning SHALL be available only when the reachable values satisfy `Clone`.

#### Scenario: Object clone requires cloneable reachable values
- **WHEN** a dynamic object graph contains a value that does not implement `Clone`
- **THEN** deep cloning is rejected with a diagnostic naming the unsupported value
