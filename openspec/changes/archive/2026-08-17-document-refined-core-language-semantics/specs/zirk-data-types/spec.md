## ADDED Requirements

### Requirement: Tuple value semantics
Tuple types SHALL use `Tuple(T...)`, values SHALL use `(v...)`, and heterogeneous element access SHALL use a compile-time constant index `tuple[n]` including normalized negative indexes. Tuples SHALL be immutable values, SHALL NOT support slicing or one-element/empty forms, and SHALL derive capabilities component-wise when requested.

#### Scenario: Tuple index type
- **WHEN** `(200, "OK")[1]` is checked
- **THEN** the result type is String and the extracted String is independent

### Requirement: Record value semantics
Records SHALL be nominal immutable values with named-only construction, omitted type defaults, methods without mutation, no custom constructor/inheritance, structural field equality, and explicit component-wise derivation.

#### Scenario: Omitted record fields
- **WHEN** `Settings()` omits Int32, Boolean, and String attributes
- **THEN** they contain `0`, `false`, and `""` respectively

### Requirement: Closed data-only enums
Traditional and algebraic enums SHALL be closed data declarations and SHALL NOT contain user-defined methods. Traditional cases SHALL expose native `.name`, `.value`, `to_string()`, `from_name()`, and `from_value()` behavior without implicit mapping conversion or declaration order. Algebraic payloads SHALL be extracted only through exhaustive match.

#### Scenario: Enum method rejected
- **WHEN** an enum body declares `fn to_celsius()`
- **THEN** compilation fails and domain behavior must be expressed by an external function with match

### Requirement: Normalized union semantics
Union order and duplicates SHALL not affect identity; `T | Never` and `T | T` SHALL normalize to T, `T | Null` to `T?`, and a subtype alternative subsumed by its supertype SHALL be removed. Only common compatible capabilities SHALL be callable before narrowing.

#### Scenario: Subsumed union member
- **WHEN** Dog extends Animal and `Animal | Dog` is formed
- **THEN** the type normalizes to Animal
