## REMOVED Requirements

### Requirement: Record value semantics
**Reason**: Named records are replaced by reference-based dynamic objects constrained by contracts.
**Migration**: Use `Object` for mutable object data, `contract` for structural guarantees, and tuples or enums for value aggregates.

### Requirement: Records
**Reason**: A separate immutable named-record type would duplicate the object and contract model.
**Migration**: Replace record declarations with a `contract` plus an object value, or use a class when nominal identity and behavior are required.

### Requirement: Record spread and rest
**Reason**: Field spread and rest now operate on dynamic objects and are specified by `spread-rest-operators`.
**Migration**: Use object spread and object rest; assign the result to a contract when a fixed structural view is required.

### Requirement: Pointer.from accepts record lvalues
**Reason**: Records no longer have inline value layout.
**Migration**: Use supported class/object references for addressable managed data; FFI layout requirements remain explicit.

### Requirement: Universal rendering on structured values
**Reason**: Record-specific rendering is removed and dynamic object rendering is defined by `universal-string-rendering`.
**Migration**: Use the ordinary object rendering or implement an explicit `to_string()` operation.

### Requirement: Type categories updated
**Reason**: The category list still presents `record` as a user-defined type and `Object` as a universal type root.
**Migration**: Use `contract` for structural declarations and `Object` for dynamic object values; scalars keep their own types.

### Requirement: Clone derivation for record and enum
**Reason**: Record auto-cloning is removed along with record value semantics.
**Migration**: Enums retain component-wise cloning; dynamic objects use explicit deep `clone()` when their reachable values support it.

### Requirement: Generic contract satisfaction
**Reason**: This requirement includes generic record conformance, which is removed.
**Migration**: Use generic classes or dynamic objects with generic contracts.

### Requirement: JSON values and typed codecs are explicit
**Reason**: The current API exposes a second JSON value tree instead of materializing object nodes as ordinary Zirk objects.
**Migration**: Parse JSON into tagged dynamic Zirk values; use `contract`-based typed codecs when a fixed structural view is required.

### Requirement: Compound values preserve managed-reference paths
**Reason**: The tracing model must describe dynamic object references and their runtime-tagged property values, not inline records.
**Migration**: Store dynamic data in `Object`; keep tuples, enums, closures, nullable values, and collections as supported aggregates.

## ADDED Requirements

### Requirement: Object is the open dynamic object value
`Object` SHALL represent the common reference-object view for dynamic object literals and class instances. Its string-keyed dynamic properties SHALL preserve their Zirk runtime types. It SHALL NOT be the universal root type of scalars, enums, tuples, or other values. A dynamic property read whose type cannot be proven statically SHALL produce a tagged `Dynamic` value that requires safe narrowing before typed use.

#### Scenario: Object accepts a class reference but not a scalar
- **WHEN** a class instance and an `Int32` are each assigned to `Object`
- **THEN** the class instance preserves its identity through the object view and the scalar assignment is rejected

#### Scenario: Dynamic read is safely narrowed
- **WHEN** a property is read from an `Object` without a statically known property type
- **THEN** the result preserves its runtime tag and cannot be used as a concrete type until a checked narrowing succeeds

### Requirement: Object literals support static and dynamic string keys
An object literal SHALL accept identifier-shaped keys without quotes, quoted string keys, and interpolated string keys. An interpolated key SHALL be evaluated as a string using Zirk's ordinary interpolation conversion. Computed property access SHALL accept a string key. Literal property values SHALL preserve their own inferred or expected types.

#### Scenario: Identifier key creates a string property
- **WHEN** `{ name: "Ada" }` is evaluated
- **THEN** it creates an `Object` with the string key `name` and a `String` value

#### Scenario: Interpolated key is evaluated
- **WHEN** `key` contains `custom_status` and `{ "{key}": null }` is evaluated
- **THEN** the object contains the key `custom_status` with a `Null` value

### Requirement: User-defined types use contracts for structural shape
The user-defined type model SHALL include classes with reference identity, contracts with structural property and operation requirements, enums with closed variants, unions, aliases, and open dynamic `Object` values. Scalars SHALL retain their own types and SHALL NOT be widened to `Object`.

#### Scenario: Structural declarations use contract
- **WHEN** a developer defines a named shape for an object
- **THEN** the declaration is a `contract`, not a record or interface

### Requirement: Enums retain component-wise cloning
An enum SHALL implement `Clone` when all associated values implement `Clone`. A dynamic `Object` SHALL NOT receive implicit value-copy semantics from this rule.

#### Scenario: Clone an enum with cloneable payloads
- **WHEN** every associated type of an enum implements `Clone` and an enum value is cloned
- **THEN** the clone contains independently cloned payload values

### Requirement: JSON materializes ordinary Zirk values
The JSON API SHALL parse JSON objects into ordinary `Object` values, arrays into ordinary Zirk collections of dynamic values, and JSON scalars into their corresponding Zirk scalar or `Null` values. The public parse result SHALL be `Dynamic`, not a second public object/tree model. Stringification SHALL serialize those values back to JSON, preserve exact JSON number behavior, and raise a catchable error for cycles or values that JSON cannot represent. Typed JSON codecs SHALL use contracts for structural object validation and SHALL NOT require retained decorators or runtime field scanning.

#### Scenario: Parsed JSON object is a dynamic object
- **WHEN** JSON containing `{"status":"ok"}` is parsed
- **THEN** the result is an `Object` whose `status` property can be read and narrowed to `String`

#### Scenario: Interpolated object key serializes
- **WHEN** an object literal uses an interpolated key and is passed to JSON stringification
- **THEN** the resulting JSON contains the evaluated string key

#### Scenario: Non-JSON runtime value is rejected
- **WHEN** stringification encounters a callable, resource, or cyclic object graph that JSON cannot represent
- **THEN** it raises a catchable JSON conversion error without returning partial JSON
