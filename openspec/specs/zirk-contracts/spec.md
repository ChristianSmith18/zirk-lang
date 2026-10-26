# zirk-contracts Specification

## Purpose
Defines structural contracts, traits, abstract requirement classes, capability
contracts, conformance, conflict resolution, and dynamic contract dispatch.
## Requirements
### Requirement: Contract, trait, and abstract requirements

Contracts SHALL contain structural property requirements and operation
signatures without reusable bodies. Traits SHALL contain behavior requirements
and reusable method bodies but no attributes or constructors. Abstract classes
SHALL contain nominal attribute and abstract-method requirements but no bodies.
Classes SHALL be able to implement contracts, traits, and abstract classes; a
declaration SHALL satisfy every compatible requirement explicitly.

#### Scenario: Trait state rejected

- **WHEN** a trait declares an instance attribute
- **THEN** compilation fails and recommends a required getter/setter method or abstract-class requirement

### Requirement: Contract and trait composition and conflict resolution

Contracts MAY extend contracts; traits MAY implement contracts and compose with
traits; abstract classes MAY implement contracts and extend abstract classes.
Cycles and incompatible same-name signatures SHALL fail. Conflicting trait
defaults SHALL require the `#override` marker (since a trait default is an
inherited implementation) and MAY select an implementation with
`TraitName.super.method()`.

#### Scenario: Explicit trait selection

- **WHEN** two adopted traits define `print()` and the class overrides it using `JsonPrintable.super.print()` under a `#override` marker
- **THEN** the selected default is called without declaration-order precedence

#### Scenario: Contract requirement needs no marker

- **WHEN** a class implements a contract operation and writes no `#override`
- **THEN** the implementation is accepted, since no inherited implementation is replaced

### Requirement: Contracts describe structural properties and operations

A `contract` SHALL declare required property types and operation signatures.
Classes and open `Object` values SHALL be eligible to conform by shape. A
contract-typed view SHALL expose only declared members while preserving the
identity of its underlying object.

#### Scenario: Class implementing a contract

- **WHEN** `class User implements Serializable` defines every required property and operation
- **THEN** the check succeeds
- **AND** `User` is acceptable wherever `Serializable` is expected

#### Scenario: Unimplemented contract operation

- **WHEN** a class declares that it implements a contract and omits a required operation
- **THEN** a diagnostic is emitted naming the missing operation and its signature

#### Scenario: Object conforms structurally

- **WHEN** an open object has every property and operation required by a contract
- **THEN** it is accepted through that contract view without changing object identity

### Requirement: Operator contracts

Operator overloading SHALL occur only by implementing the contract the language defines for it, and SHALL NOT alter its precedence or arity, per `ZIRK_LANGUAGE_SPEC.md` section 4.

Contracts use reserved methods: `_add` (`+`), `_subtract` (`-`), `_multiply` (`*`), `_divide` (`/`), `_remainder` (`%`), `_equals` (`==`, and `!=` as its negation), `_less` (`<`), `_less_equal` (`<=`), `_greater` (`>`), and `_greater_equal` (`>=`). User-defined types MAY implement them in safe code; native types SHALL NOT be reopenable from application code.

`String` SHALL implement native concatenation and checked repetition contracts:
`String * Integer` and `Integer * String` return a new String, reject negative
counts, and diagnose unrepresentable allocation sizes.

#### Scenario: String concatenation

- **WHEN** `"a" + "b"` is evaluated
- **THEN** the result is `"ab"`
- **AND** it resolves via the contract that `String` implements, not via a special-cased operator

#### Scenario: Operator on a type that does not implement it

- **WHEN** `+` is applied to a type that does not implement the contract
- **THEN** a diagnostic is emitted naming the missing contract

#### Scenario: Custom type implementing the contract

- **WHEN** a class implements the addition contract and two of its instances are written with `+`
- **THEN** its implementation is invoked

#### Scenario: Comparison through a contract

- **WHEN** a class implements `_less` and two of its instances are written with `<`
- **THEN** its implementation is invoked and the result is `Boolean`

#### Scenario: Inequality from equality

- **WHEN** a class implements `_equals` and two of its instances are written with `!=`
- **THEN** the result is the negation of `_equals`, with no separate method required

### Requirement: Iteration via contract

`Iterable<T>` SHALL be the source contract for explicit spread in calls, collection literals, and rest destructuring. Spread SHALL use the iterable's documented order and ordinary iteration projection-copy, invalidation, transfer, and sharing rules.

#### Scenario: User iterable spread
- **WHEN** an object implementing `Iterable<Int32>` is used as `...source`
- **THEN** its iterator supplies the expanded `Int32` values in order

### Requirement: A contract cannot be partially satisfied

A declaration that claims to implement a contract SHALL implement it completely before being accepted.

#### Scenario: Partial implementation

- **WHEN** a class implements two of the three methods of a contract
- **THEN** a diagnostic is emitted for each missing method

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

### Requirement: Structural object types compose with type algebra
Structural object type expressions SHALL declare required and optional properties, property types, and read-only constraints. They SHALL compose with contracts, unions, intersections, aliases, and generic type operators. Structural assignability SHALL check all required fields and compatible property capabilities. Classes SHALL retain nominal identity while conforming to structural contracts by shape.

Reading an optional property SHALL produce its declared type joined with `Null`, because the property may be absent. An optional property accepts only values assignable to its declared type when explicitly supplied; absence is represented by omitting the property. Writes through a structural view SHALL honor its read-only declaration, while required or optional mutable properties remain writable with their declared value type.

#### Scenario: Object literal satisfies a structural type
- **WHEN** an object literal provides every required field with assignable values
- **THEN** it is assignable to that structural object type

#### Scenario: Optional property is absent
- **WHEN** an object omits an optional property
- **THEN** the object remains assignable and reading the property produces a nullable value

#### Scenario: Optional property read and write
- **WHEN** code reads an optional property or writes a declared mutable optional property
- **THEN** the read includes `Null` and the write accepts only the property's declared type

#### Scenario: Read-only property rejects mutation
- **WHEN** a value is viewed through a structural type with a read-only property
- **THEN** assignment through that view is rejected

#### Scenario: Structural intersection combines fields
- **WHEN** an object satisfies { id: String } & { active: Boolean }
- **THEN** both fields are available through the intersection view

### Requirement: Contextual object literals select union alternatives
When an object literal is checked against a union of structural types or contracts, the checker SHALL evaluate every viable alternative using required keys, property types, nested context, and singleton discriminants. Exactly one viable alternative SHALL determine the literal's selected union member. No viable alternative SHALL produce a diagnostic identifying the closest mismatches. More than one viable alternative SHALL produce an ambiguity diagnostic and SHALL NOT be resolved by declaration order. Dynamic keys and spreads SHALL contribute only facts the checker can prove statically.

#### Scenario: Boolean discriminant selects one alternative
- **WHEN** an object literal with active: true is assigned to Enabled | Disabled where the contracts require opposite Boolean literals
- **THEN** Enabled is selected and its remaining fields are checked

#### Scenario: Canonical value discriminants select one alternative
- **WHEN** an object literal supplies a numeric, String, Char, temporal, enum, or immutable aggregate value that matches one singleton property in a structural union
- **THEN** canonical constant equality selects only the matching alternative and its remaining properties receive that alternative's context

#### Scenario: Nested discriminant selects a generic alternative
- **WHEN** a nested object literal supplies a singleton field that uniquely identifies one generic union member
- **THEN** the checker propagates the selected type arguments and validates nested fields in that context

#### Scenario: Selected alternative contextualizes nested generic, tuple, and callable values
- **WHEN** a discriminant uniquely selects a structural alternative containing a generic nested contract, a tuple, and a callable whose lambda parameter omits its type
- **THEN** the checker substitutes the nested generic argument, checks tuple elements positionally, and infers the lambda parameter from the selected callable signature

#### Scenario: No alternative accepts the literal
- **WHEN** an object literal satisfies no member of its expected union
- **THEN** a diagnostic reports the rejected field or missing requirement for each relevant candidate

#### Scenario: Multiple alternatives remain viable
- **WHEN** an object literal satisfies more than one union member and no literal discriminant distinguishes them
- **THEN** the checker reports ambiguity and offers an explicit type annotation or construction as guidance

#### Scenario: Unknown spread cannot invent a discriminant
- **WHEN** an object literal relies on a runtime-unknown spread for its discriminant
- **THEN** the checker does not claim one alternative was proven from that field

#### Scenario: Known spread discriminants use their declared property type
- **WHEN** a spread contributes a discriminant through a type alias with a singleton property type
- **THEN** the checker may select the matching alternative, while a broad mutable-capable property type such as `Boolean` cannot select either singleton alternative

#### Scenario: Object-literal selection does not narrow its source binding
- **WHEN** a new object literal selects an alternative using an explicit discriminant while spreading a union-typed source
- **THEN** the new literal is checked against the selected alternative and the source binding retains its declared union type
