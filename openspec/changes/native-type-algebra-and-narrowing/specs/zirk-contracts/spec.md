## ADDED Requirements

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
