## MODIFIED Requirements

### Requirement: Optional, named, variadic parameters and default values

The parser SHALL recognize optional parameters, default values, and a final variadic parameter prefixed with `...`. It SHALL additionally recognize `...expression` as an explicit spread argument in calls and `...expression` as a collection-literal element, while keeping declaration and expression contexts distinct.

#### Scenario: Variadic declaration
- **WHEN** `fn print(...values: Int32): Void { }` is parsed
- **THEN** a variadic parameter is produced

#### Scenario: Spread call argument
- **WHEN** `print(...values)` is parsed
- **THEN** one spread argument node is produced

#### Scenario: Rest destructuring pattern
- **WHEN** `[first, ...remaining]` is parsed in a destructuring declaration
- **THEN** a final rest binding is produced

## ADDED Requirements

### Requirement: Object spread and rest grammar

The parser SHALL accept field-based object/record spread in a record-typed expression context (`{ ...source, field: value }`) and a final object rest binding in record destructuring (`{ field, ...rest }`). It SHALL distinguish these forms from statement blocks and reject more than one rest binding or any field after the rest binding.

#### Scenario: Object spread expression
- **WHEN** `{ ...profile, active: false }` appears where a `UserProfile` is expected
- **THEN** the parser produces an object-spread expression with one override field

#### Scenario: Object rest pattern
- **WHEN** `{ id, ...details }` appears in a destructuring declaration
- **THEN** the parser produces a record pattern with a final rest binding
