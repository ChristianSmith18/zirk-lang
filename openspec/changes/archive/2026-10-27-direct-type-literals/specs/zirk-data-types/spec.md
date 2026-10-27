## MODIFIED Requirements

### Requirement: Singleton types represent compile-time values across Zirk types
Any type SHALL be eligible for a singleton type when a supported literal directly denotes a value of that type and its owning type defines stable value identity. This SHALL include Boolean, all integer and floating/decimal families, String, Char, Null, Duration, Date, Time, DateTime, and enum cases without payloads. Tuple types and structural object types SHALL use their existing type-expression semantics, including exact literal-valued discriminants on object properties. The language SHALL NOT restrict singleton types to a hard-coded primitive list. Singleton alternatives SHALL be expressed as direct type names or literal values. Temporal singleton values SHALL use only their fixed built-in constructor forms with direct integer components; bare ISO temporal spellings, wrappers, arbitrary function calls, and expressions SHALL NOT be accepted. Mutable reference identity SHALL NOT be mistaken for structural value equality.

#### Scenario: Temporal singleton alternatives
- **WHEN** `type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);` is used as a type
- **THEN** only those two Date values are assignable to it

#### Scenario: Time and DateTime singleton alternatives
- **WHEN** `type OfficeTime = Time(9, 0) | Time(17, 30); type Meeting = DateTime(Date(2026, 12, 25), Time(9, 30)) | DateTime(Date(2026, 12, 25), Time(10, 0));` is used as types
- **THEN** the alternatives are canonical Time and local DateTime singleton values

#### Scenario: Duration singleton alternatives
- **WHEN** a type is defined from two direct duration literals
- **THEN** values equal to either exact duration are accepted and other durations are rejected

#### Scenario: Enum case singleton
- **WHEN** a qualified enum case is used directly as a singleton alternative
- **THEN** only that enum case is assignable to the alternative

#### Scenario: User value type exposes a singleton
- **WHEN** an immutable user-defined value type has no supported direct literal syntax
- **THEN** singleton syntax does not invoke its constructor or user-defined function

#### Scenario: Object property literal discriminates a structural alternative
- **WHEN** structural object alternatives use `active: true` and `active: false`
- **THEN** the checker retains those exact property literals to distinguish the alternatives

#### Scenario: Temporal constructor value is a singleton alternative
- **WHEN** `type Holiday = Date(2026, 12, 25); type Opening = Time(9, 30); type Meeting = DateTime(Date(2026, 12, 25), Time(9, 30));` is used as singleton types
- **THEN** each fixed constructor form denotes a canonical Date, Time, or local DateTime value

#### Scenario: Bare ISO temporal spelling is rejected
- **WHEN** `type Holiday = 2026-12-25;` is used as a singleton type
- **THEN** compilation requires `Date(...)` in type position

#### Scenario: Unsupported constructor call is rejected
- **WHEN** `type Holiday = Date.today();` is used as a singleton type
- **THEN** compilation rejects the call and requires one of the fixed component constructors

#### Scenario: Runtime-dependent value cannot form a singleton
- **WHEN** a singleton type depends on the current clock, environment, random input, I/O, mutable state, or a function call
- **THEN** compilation reports that only type names and literal atoms are accepted
