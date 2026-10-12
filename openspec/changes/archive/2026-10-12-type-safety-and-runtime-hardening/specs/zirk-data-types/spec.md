## ADDED Requirements

### Requirement: Compound values preserve managed-reference paths

The representation of tuples, records, enums, closures, and nullable values SHALL expose every initialized managed-reference path to root enumeration and heap tracing. Inactive enum payloads SHALL never be interpreted as live references.

#### Scenario: Enum variant contains managed references
- **WHEN** a live enum selects one variant and another variant has reference-typed fields
- **THEN** collection preserves the selected variant's references and ignores inactive storage safely

#### Scenario: Inline value contains nested references
- **WHEN** a tuple or record containing a managed reference is copied into a supported collection element
- **THEN** the nested reference remains live while the collection element remains reachable

### Requirement: Numeric conversions have defined boundary outcomes

Conversions between binary floating values, exact-decimal values, integers, and durations SHALL either produce a representable result under the language's specified rounding rule or a controlled language failure. Native and interpreted execution SHALL agree on the value or failure category.

#### Scenario: Float cannot be represented by the destination integer
- **WHEN** a conversion receives NaN, infinity, or a value outside the destination integer range
- **THEN** it produces a controlled conversion failure and never invokes an undefined backend conversion

#### Scenario: Conversion is representable
- **WHEN** a finite floating value lies within the destination range
- **THEN** native and interpreted execution apply the same documented rounding rule
