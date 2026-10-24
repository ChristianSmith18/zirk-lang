## REMOVED Requirements

### Requirement: Contract syntax
**Reason**: The existing syntax gives `interface` and `trait` overlapping structural declaration roles.
**Migration**: Write structural declarations with `contract`; keep `trait` declarations for reusable method implementations.

### Requirement: Data type syntax
**Reason**: The `record` declaration syntax is removed.
**Migration**: Use object literals with `Object`, and declare structural requirements with `contract`.

### Requirement: Spread and rest operator syntax
**Reason**: This requirement includes record patterns and record literal expressions.
**Migration**: Use object patterns and object spread for dynamic keys; collection spread remains unchanged.

### Requirement: Object spread and rest grammar
**Reason**: This requirement constrains spread to record-typed contexts.
**Migration**: Use open object spread and rest syntax as defined by `spread-rest-operators`.

### Requirement: Match alternatives, regex, and nested destructuring
**Reason**: The requirement includes record-specific nested destructuring syntax.
**Migration**: Use object property patterns for dynamic values and enum payload patterns for closed data.

## ADDED Requirements

### Requirement: Contract declaration syntax
The parser SHALL recognize `contract` declarations containing required properties and operation signatures. Contract operations SHALL omit `fn` consistently with existing type-body method syntax. Contract members SHALL NOT contain implementation bodies; trait methods may contain bodies.

#### Scenario: Contract with a property and operation
- **WHEN** `contract Named { name: String; label(): String; }` is parsed
- **THEN** a contract declaration contains one property requirement and one operation signature

#### Scenario: Contract body is rejected
- **WHEN** a contract operation includes a method body
- **THEN** a diagnostic directs reusable implementation to a trait or class

### Requirement: Dynamic object literal keys and access
The parser SHALL accept identifier keys, quoted string keys, and quoted keys containing ordinary string interpolation in object literals. It SHALL parse `object[key]` as computed object-property access when the receiver is `Object`, while preserving collection and string indexing rules for other receiver types.

#### Scenario: Interpolated key
- **WHEN** `{ "{name}": value }` is parsed
- **THEN** the key is represented as a runtime string expression

#### Scenario: Computed property access
- **WHEN** `object[key]` is parsed with an `Object` receiver and a string key
- **THEN** it is represented as dynamic property access rather than a numeric collection index

### Requirement: Remaining data type declarations parse
The parser SHALL continue to recognize enum variants with associated data, unions `A | B`, and aliases with `type`. It SHALL NOT recognize `record` or `interface` declarations as active language syntax.

#### Scenario: Enum with associated data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is parsed
- **THEN** the variants retain their declared payload types

#### Scenario: Union type
- **WHEN** `mut value: Int32 | String;` is parsed
- **THEN** the declaration contains a two-member union type

#### Scenario: Legacy record declaration is rejected
- **WHEN** source declares `record Point { x: Int32; }`
- **THEN** a targeted migration diagnostic recommends `contract` and `Object`

### Requirement: Spread and rest syntax remains distinct by domain
The parser SHALL distinguish positional call and collection spread based on `Iterable<T>` from named object spread and rest. Object spread and rest SHALL accept open objects; collection rest remains an ordered suffix binding.

#### Scenario: Collection spread remains iterable syntax
- **WHEN** `[0, ...values]` or `sum(...values)` is parsed
- **THEN** the spread operand is represented as an iterable expansion

#### Scenario: Dynamic object spread parses
- **WHEN** `{ ...source, field: value }` is parsed in expression position
- **THEN** it produces a dynamic object spread expression
