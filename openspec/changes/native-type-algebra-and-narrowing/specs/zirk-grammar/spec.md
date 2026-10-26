## MODIFIED Requirements

### Requirement: Remaining data type declarations parse
The parser SHALL continue to recognize enum variants with associated data, unions A | B, and aliases with type. It SHALL recognize recursive union/intersection type expressions, singleton value types, generic type expressions, and structural object type expressions in every type position. & SHALL bind more tightly than |; parentheses SHALL override precedence. T? SHALL remain equivalent to T | Null. The parser SHALL NOT recognize legacy record or interface declarations as active declaration syntax.

#### Scenario: Enum with associated data
- **WHEN** enum Shape { Circle(Int32), Rect(Int32, Int32) } is parsed
- **THEN** the variants retain their declared payload types

#### Scenario: Union type
- **WHEN** mut value: Int32 | String; is parsed
- **THEN** the declaration contains a two-member union type

#### Scenario: N-ary union and intersection
- **WHEN** type View = A & B & C | D | E; is parsed
- **THEN** the syntax tree preserves three intersection members in the first union branch and two additional union alternatives

#### Scenario: Parentheses control type precedence
- **WHEN** (A | B) & (C | D) is parsed
- **THEN** both parenthesized unions remain distinct operands of the outer intersection

#### Scenario: Nested type expressions in generic arguments
- **WHEN** type Values = List<A | B> & Iterable<C | D>; is parsed
- **THEN** each generic argument contains its complete nested union expression

#### Scenario: Boolean, numeric, and string singleton syntax
- **WHEN** type Status = 200 | 404; type Mode = "read" | "write"; type Flag = true | false; is parsed
- **THEN** each literal remains a singleton type expression rather than a value expression

#### Scenario: General compile-time value singleton syntax
- **WHEN** type Holiday = const(Date(2026, 12, 25)) | const(Date(2027, 1, 1)); is parsed
- **THEN** each const(...) expression is preserved for bounded compile-time evaluation

#### Scenario: Temporal value patterns
- **WHEN** a match uses a Duration literal or deterministic Date, Time, or DateTime constructor call as a pattern
- **THEN** the parser preserves the exact literal or constructor expression and its source span for constant checking

#### Scenario: Structural object type syntax
- **WHEN** a type alias declares required, optional, literal-valued, and read-only properties in an object type expression
- **THEN** each property modifier and property type is preserved in the syntax tree

#### Scenario: Legacy record declaration is rejected
- **WHEN** source declares record Point { x: Int32; }
- **THEN** a targeted migration diagnostic recommends contract and Object
