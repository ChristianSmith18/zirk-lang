## REMOVED Requirements

### Requirement: Public type taxonomy
**Reason**: The current taxonomy makes `Object` the conceptual root and distinguishes records as a value type.
**Migration**: Treat `Object` as the dynamic object type, keep scalar and aggregate types distinct, and use contracts for structural views.

### Requirement: Object reference strictness
**Reason**: This requirement covers only class references and omits open dynamic object properties and contract sealing.
**Migration**: Apply the same binding mutability rules to `Object`; use contracts to limit visible members and seal the shared key set.

### Requirement: Object spread and rest typing
**Reason**: The current rule requires a known nominal record shape, contrary to the open-object model.
**Migration**: Spread open objects as defined by `spread-rest-operators` and assign to a contract when fixed fields are required.

### Requirement: Runtime type identity is safe and representation-independent
**Reason**: Its interface scenario exposes a distinct runtime interface type identity, while contracts are structural views.
**Migration**: Preserve concrete runtime type identity; contract conformance does not create a new object identity or runtime class.

### Requirement: Value, enum, array, iteration, and generator semantics
**Reason**: This requirement makes immutable records and structural record equality normative.
**Migration**: Use `Object` for reference-based mutable object data; tuples and enums remain value aggregates, and collections retain their current behavior.

### Requirement: Default initialization and immutable data
**Reason**: The rule declares records immutable values, which is removed.
**Migration**: Use immutable tuples and closed enums for values; ordinary objects follow reference mutability and contracts.

### Requirement: Nominal types and subtyping
**Reason**: The current rule includes records among nominal user-defined types and does not distinguish structural contracts.
**Migration**: Classes and enums remain nominal; contracts are structural and object-shaped values may conform by shape.

## ADDED Requirements

### Requirement: Object and Dynamic have distinct typing roles
`Object` SHALL type reference-based dynamic objects only and SHALL NOT accept scalar values through implicit widening. `Dynamic` SHALL represent a runtime-tagged value obtained from an object property whose static type is unknown. A `Dynamic` value SHALL require a checked type test, pattern match, or checked cast before use as a concrete type.

#### Scenario: Scalar does not widen to Object
- **WHEN** an `Int32` is assigned to `Object`
- **THEN** the checker rejects the assignment

#### Scenario: Dynamic access requires narrowing
- **WHEN** a `Dynamic` value is passed to a function requiring `String` without a check
- **THEN** the checker reports that narrowing is required

### Requirement: Runtime type identity remains concrete under contract views
Runtime type identity SHALL identify the concrete class or `Object` allocation. Viewing a value through a contract SHALL NOT create a distinct runtime type identity or alter the underlying object's identity. Contract compatibility SHALL remain a structural type-checking operation.

#### Scenario: Contract view preserves the concrete runtime type
- **WHEN** a class instance or dynamic object is held through a contract-typed binding
- **THEN** runtime type inspection identifies the underlying class or `Object`, not a new nominal contract type

### Requirement: Nominal classes and structural contracts are distinct
Classes and enums SHALL retain nominal type identity. Class references SHALL be assignable to the `Object` reference view without changing identity; scalars, tuples, and enums SHALL NOT widen to `Object`. A value SHALL be accepted by a contract type when it structurally satisfies the contract, even if it does not declare a nominal inheritance relationship. Two class types with the same shape SHALL remain distinct.

#### Scenario: Dynamic object conforms structurally
- **WHEN** an object has all required contract properties and operations with compatible types
- **THEN** it is accepted through that contract view

#### Scenario: Same-shaped classes remain distinct
- **WHEN** a value of one class is assigned to another class with the same members
- **THEN** the checker rejects the assignment

#### Scenario: Class reference widens to Object
- **WHEN** an instance of a class is assigned to `Object`
- **THEN** the reference identity is preserved and dynamic property access becomes available

### Requirement: Reference mutability applies to classes and dynamic objects
For class and `Object` references, `mut` SHALL permit rebinding and referent mutation, `inmut` SHALL prevent rebinding while permitting referent mutation, and `inmut::strict` SHALL prevent reachable mutation. A strict reference SHALL NOT become a mutable alias or be acquired while a mutable alias remains accessible.

#### Scenario: Strict dynamic object prevents property mutation
- **WHEN** an object is reachable only through an `inmut::strict Object` binding and code writes one of its properties
- **THEN** the checker rejects the mutation

### Requirement: Collections and enum semantics remain unchanged
An unmapped traditional enum case SHALL expose its exact case name as its default string value and no implicit numeric index. Arrays SHALL have fixed length; contextual collection literals SHALL infer `Array<T>` by default or select `List<T>` from context; range elements SHALL expand before type unification and allocation; `String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and `Iterable<T>` while preserving locals between yields. Tuples and enums SHALL retain their documented value behavior. Dynamic objects SHALL use reference identity and SHALL NOT gain structural equality implicitly.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: Contextual list typing
- **WHEN** `inmut xs: List<Int32> = [0..3]` is written
- **THEN** the literal has type `List<Int32>` and length `3`

#### Scenario: String iteration
- **WHEN** a string is consumed by `for ... in`
- **THEN** iteration produces the string's public character units in order

### Requirement: Object property operations obey mutability and presence
Named and computed access on `Object` SHALL resolve dynamically unless a statically known contract view supplies a type. Adding, replacing, and removing properties SHALL follow existing reference mutability, including the distinction between `inmut` and `inmut::strict`. Reading a missing property directly SHALL raise a catchable missing-property error; an explicit presence-safe lookup SHALL return an optional result without throwing. Once any contract is successfully applied, no alias SHALL add or remove keys.

#### Scenario: Mutable object adds a property
- **WHEN** a property is assigned through a mutable `Object` binding before contract conformance
- **THEN** the property is added with its runtime value type preserved

#### Scenario: Strict binding cannot mutate properties
- **WHEN** a property is assigned through an `inmut::strict Object` binding
- **THEN** the checker rejects the mutation

#### Scenario: Direct read of a missing property
- **WHEN** code reads a missing key with direct property access
- **THEN** a catchable missing-property error is raised

#### Scenario: Presence-safe lookup
- **WHEN** code uses the explicit safe lookup for a missing key
- **THEN** it receives the absent result and no exception is raised

### Requirement: Contract views enforce structural types
Assignment to a contract type SHALL expose only the contract's declared properties and operations. Required members SHALL be checked statically when shape information is available and dynamically otherwise. Property writes through the contract view SHALL satisfy the declared property type and mutability. Successful dynamic conformance SHALL seal the complete existing object key set across aliases.

#### Scenario: Contract view permits a declared mutable property
- **WHEN** a mutable contract view writes a type-compatible mutable property
- **THEN** the write succeeds on the underlying object

#### Scenario: Contract view rejects an undeclared key
- **WHEN** code accesses an undeclared property through a contract-typed value
- **THEN** the checker reports that the property is not part of the contract
