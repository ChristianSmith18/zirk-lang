## MODIFIED Requirements

### Requirement: Remaining data type declarations parse
The parser SHALL continue to recognize enum variants with associated data, unions `A | B`, and aliases with `type`. It SHALL recognize recursive union/intersection type expressions, generic type expressions, and structural object type expressions in every type position. `&` SHALL bind more tightly than `|`; parentheses SHALL override precedence. `T?` SHALL remain equivalent to `T | Null`. Singleton alternatives SHALL be written only as a type name or a supported literal in type position; the grammar SHALL NOT require `const(...)` or accept arbitrary function calls or expressions as singleton type atoms. It SHALL accept only the built-in `Date(year, month, day)`, `Time(hour, minute, second?, nanosecond?)`, and `DateTime(Date(...), Time(...))` forms when every component is a direct integer literal. Primitive, numeric, string, character, duration, null, and enum-case literals SHALL use direct literal syntax. Tuple and object types SHALL retain their existing type-expression grammar, including literal-valued object properties. Temporal singleton atoms SHALL require their owning `Date(...)`, `Time(...)`, or `DateTime(Date(...), Time(...))` constructor form; bare ISO spellings SHALL be rejected in type position. The parser SHALL NOT recognize legacy record or interface declarations as active declaration syntax.

#### Scenario: Enum with associated data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is parsed
- **THEN** the variants retain their declared payload types

#### Scenario: Union type
- **WHEN** `mut value: Int32 | String;` is parsed
- **THEN** the declaration contains a two-member union type

#### Scenario: N-ary union and intersection
- **WHEN** `type View = A & B & C | D | E;` is parsed
- **THEN** the syntax tree preserves three intersection members in the first union branch and two additional union alternatives

#### Scenario: Parentheses control type precedence
- **WHEN** `(A | B) & (C | D)` is parsed
- **THEN** both parenthesized unions remain distinct operands of the outer intersection

#### Scenario: Nested type expressions in generic arguments
- **WHEN** `type Values = List<A | B> & Iterable<C | D>;` is parsed
- **THEN** each generic argument contains its complete nested union expression

#### Scenario: Primitive literal singleton syntax
- **WHEN** `type Status = 200 | 404; type Mode = "read" | "write"; type Flag = true | false;` is parsed
- **THEN** each literal remains a singleton type expression

#### Scenario: Boolean, numeric, and string singleton syntax
- **WHEN** `type Status = 200 | 404; type Mode = "read" | "write"; type Flag = true | false;` is parsed
- **THEN** each literal remains a singleton type expression rather than a value expression

#### Scenario: Bare ISO temporal singleton syntax is rejected
- **WHEN** `type Holiday = 2026-12-25; type Opening = 09:30; type Meeting = 2026-12-25 09:30;` is parsed
- **THEN** the parser reports that temporal singleton values require their owning constructor

#### Scenario: Time singleton values use Time constructors
- **WHEN** `type OfficeHours = Time(9, 0) | Time(17, 30, 15, 250000000);` is parsed
- **THEN** each value is preserved as a Time literal atom with direct integer components

#### Scenario: Temporal constructors are restricted literal atoms
- **WHEN** `type Holiday = Date(2026, 12, 25); type Opening = Time(9, 30); type Meeting = DateTime(Date(2026, 12, 25), Time(9, 30));` is parsed
- **THEN** each constructor is preserved as a temporal literal atom with a source span and direct integer components

#### Scenario: Call-shaped singleton syntax is rejected
- **WHEN** `type Holiday = Date.today();` or `type Count = compute(1);` is parsed
- **THEN** the parser rejects the call because only fixed temporal constructors with direct integer components are admitted

#### Scenario: Temporal constructors are required in type position
- **WHEN** `type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);` is parsed
- **THEN** each constructor is preserved as a temporal literal atom without evaluating a general expression

#### Scenario: Arbitrary expression is rejected as a singleton atom
- **WHEN** `type Count = 1 + 2;` is parsed as a type alias
- **THEN** the parser rejects the expression and points to direct literal alternatives

#### Scenario: Bare ISO date is rejected in type position
- **WHEN** `type InvalidDay = 2026-02-29;` is parsed
- **THEN** the parser reports that a Date constructor is required

#### Scenario: Temporal value patterns
- **WHEN** a match uses a Duration literal or deterministic Date, Time, or DateTime constructor call as a pattern
- **THEN** the parser preserves the exact literal or constructor expression and its source span for constant checking

#### Scenario: Structural object type syntax
- **WHEN** a type alias declares required, optional, literal-valued, and read-only properties in an object type expression
- **THEN** each property modifier and property type is preserved in the syntax tree

#### Scenario: Legacy record declaration is rejected
- **WHEN** source declares `record Point { x: Int32; }`
- **THEN** a targeted migration diagnostic recommends contract and Object
