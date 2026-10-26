## MODIFIED Requirements

### Requirement: Normalized union semantics
Union order and duplicates SHALL not affect identity; T | Never and T | T SHALL normalize to T, T | Null SHALL normalize to T?, and a subtype alternative subsumed by its supertype SHALL be removed. Nested unions SHALL flatten. Literal alternatives covered by a primitive or broader literal type SHALL be removed. Normalization SHALL preserve named aliases for diagnostics while comparing their canonical targets. The checker SHALL make only members present on every alternative available before narrowing, with read types joined across alternatives and writes permitted only when safe for every alternative.

#### Scenario: Subsumed union member
- **WHEN** Dog extends Animal and Animal | Dog is formed
- **THEN** the type normalizes to Animal

#### Scenario: N-ary unions normalize independent of order
- **WHEN** A | B | C, C | A | B, and (A | B) | C are resolved
- **THEN** all three refer to the same canonical union type

#### Scenario: Boolean literal union normalizes to Boolean
- **WHEN** true | false is formed
- **THEN** it is equivalent to Boolean

#### Scenario: Primitive type subsumes its singleton
- **WHEN** String | "ready" or Int32 | 200 is formed
- **THEN** the union normalizes to the primitive type that already admits the literal

#### Scenario: Common property reads join each alternative
- **WHEN** every member of a union has property active with literal types true or false
- **THEN** reading active before narrowing produces Boolean

#### Scenario: Common property writes respect every alternative
- **WHEN** a union exposes a writable property with different types on its alternatives
- **THEN** a write is accepted only when the value is assignable to every possible property type

### Requirement: Unions and aliases
A | B | ... SHALL define a union of any finite number of alternatives, and type SHALL define a transparent alias. A union SHALL accept a value assignable to at least one alternative, remain distinct from Dynamic, and be usable in executable programs after a sound narrowing or common-member check. An undifferentiated union SHALL NOT be implicitly converted to one of its alternatives. Generic aliases SHALL be governed by zirk-generics.

#### Scenario: Value of a union
- **WHEN** a variable is declared Int32 | String and receives either compatible value
- **THEN** the assignment is accepted and the value retains its selected alternative at runtime

#### Scenario: Use without discrimination
- **WHEN** a union value is used where an unavailable member-specific type is required
- **THEN** a diagnostic explains the alternatives and identifies valid narrowing operations

#### Scenario: Alias
- **WHEN** type Id = Int32; is declared
- **THEN** Id and Int32 are interchangeable while diagnostics may retain the alias spelling

#### Scenario: Union alias executes
- **WHEN** type Input = String | Int32; is passed to and returned from a compiled function
- **THEN** native and interpreted execution preserve which alternative was supplied

## ADDED Requirements

### Requirement: Intersections compose compatible types
A & B & ... SHALL describe values satisfying every constituent. Intersections SHALL normalize associatively and idempotently, preserve all compatible structural capabilities, reduce a constituent subtype relationship to its narrower type, and reduce proven incompatible value domains to Never. Incompatible declarations with conflicting members SHALL receive a focused diagnostic when their combined meaning cannot be represented soundly.

#### Scenario: N-ary intersection composes contracts
- **WHEN** a value conforms to Readable & Writable & Closeable
- **THEN** it can be used through every constituent contract view

#### Scenario: Intersection is independent of grouping
- **WHEN** A & (B & C) and (A & B) & C are resolved
- **THEN** both produce the same canonical intersection type

#### Scenario: Incompatible singleton intersection is Never
- **WHEN** "ready" & "closed" is formed
- **THEN** the type is recognized as uninhabited

#### Scenario: Conflicting contract members are diagnosed
- **WHEN** two contract constituents declare the same member with incompatible requirements
- **THEN** the compiler reports the conflict and names both declarations

### Requirement: Singleton types represent compile-time values across Zirk types
Any type SHALL be eligible for a singleton type when its value can be constructed deterministically at compile time and its owning type defines stable value identity. This SHALL include Boolean, all integer and floating/decimal families, String, Char, Null, Duration, Date, Time, DateTime, other deterministic temporal values, enums and payloads, tuples, and user-defined immutable value types that meet the constant-value contract. The language SHALL NOT restrict singleton types to a hard-coded primitive list. Mutable reference identity SHALL NOT be mistaken for structural value equality.

#### Scenario: Temporal singleton alternatives
- **WHEN** const(Date(2026, 12, 25)) | const(Date(2027, 1, 1)) is used as a type
- **THEN** only those two Date values are assignable to it

#### Scenario: Duration singleton alternatives
- **WHEN** a type is defined from two exact Duration constants
- **THEN** values equal to either exact duration are accepted and other durations are rejected

#### Scenario: User value type exposes a singleton
- **WHEN** an immutable user-defined value type has a deterministic constant constructor and stable value equality
- **THEN** a const(...) type expression can identify one value of that type

#### Scenario: Runtime-dependent value cannot form a singleton
- **WHEN** a singleton expression depends on the current clock, environment, random input, I/O, or mutable state
- **THEN** compilation reports that the expression is not a deterministic constant
