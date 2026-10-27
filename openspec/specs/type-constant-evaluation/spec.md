# type-constant-evaluation Specification

## Purpose

Provides deterministic, bounded compile-time values for singleton types and other type computations without granting compile-time code access to runtime effects.

## Requirements

### Requirement: Constant expressions evaluate deterministically under a budget
The compiler SHALL evaluate supported constant values used by compiler computations deterministically and within explicit resource budgets. Singleton type syntax SHALL admit type names, supported literal atoms, and only the fixed built-in temporal constructor forms `Date(integer, integer, integer)`, `Time(integer, integer[, integer[, integer]])`, and `DateTime(Date(...), Time(...))`. These constructor forms SHALL be parsed and canonicalized as temporal literal atoms, without invoking the general constant-expression evaluator. Singleton syntax SHALL NOT expose that evaluator through `const(...)`, arbitrary calls, arithmetic, or other expression syntax. Runtime effects, current time, environment reads, randomness, mutable globals, tasks, and I/O SHALL never contribute to a singleton type. Any internal constant evaluation that remains SHALL be deterministic, bounded to 100,000 fuel steps, 32 call frames, aggregate nesting depth 64, and 1 MiB per canonical value. Budget exhaustion SHALL emit source-located `E0482` with the operation category and configured bound. Evaluation SHALL never hang, overflow the compiler stack, truncate a value, or return a partial result.

#### Scenario: Direct literal singleton needs no expression evaluation
- **WHEN** `type Status = 200 | 404;` is type checked
- **THEN** both literal values become exact singleton alternatives without evaluating a source expression

#### Scenario: Pure constant arithmetic evaluates
- **WHEN** an internal compiler constant computation contains supported constant arithmetic
- **THEN** it resolves to the exact canonical value at compile time

#### Scenario: Temporal constructor literal canonicalizes directly
- **WHEN** `type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);` is type checked
- **THEN** both valid Date constructor literals become canonical Date singleton values

#### Scenario: Temporal constructor literals canonicalize
- **WHEN** `Date(2026, 12, 25)`, `Time(9, 30)`, or `DateTime(Date(2026, 12, 25), Time(9, 30))` appears in type position
- **THEN** each yields a canonical singleton identity without evaluating a source expression

#### Scenario: Call-shaped singleton wrapper is rejected
- **WHEN** `type Holiday = const(Date(2026, 12, 25));` or `type Holiday = Date.today();` is type checked
- **THEN** compilation reports that only direct types, literals, and fixed temporal constructor literals are accepted

#### Scenario: Runtime effect is rejected
- **WHEN** a singleton type is written using a clock, environment read, I/O operation, or mutable state
- **THEN** compilation rejects the non-literal expression before constant evaluation

#### Scenario: Constant function terminates within its budget
- **WHEN** an internal constant computation invokes a declared pure constant function within the configured budget
- **THEN** its result is available to the compiler computation and cannot be used to spell a singleton type atom

#### Scenario: Constant function exceeds its budget
- **WHEN** internal constant evaluation exceeds its fuel, recursion, or value-size limit
- **THEN** compilation emits a deterministic budget diagnostic rather than hanging or overflowing the compiler stack

#### Scenario: Invalid constant arithmetic is source located
- **WHEN** an internal constant expression overflows its resolved integer or Decimal type
- **THEN** compilation emits `E0483` at the operation that overflowed

#### Scenario: Internal constant computation exceeds its budget
- **WHEN** an internal constant computation exceeds its fuel, recursion, or value-size limit
- **THEN** compilation emits a deterministic budget diagnostic rather than hanging or overflowing the compiler stack

#### Scenario: Invalid literal construction is source located
- **WHEN** a temporal literal contains an invalid calendar date or clock time
- **THEN** compilation reports the invalid literal at its source span

### Requirement: Singleton constants use owning-type canonical identity
Every type SHALL be able to participate in singleton types when a supported literal directly denotes a value of that type and the owning type defines stable value identity. Date, Time, and DateTime singleton values SHALL use the fixed constructor forms defined above when their components are direct integer literals; bare ISO spellings SHALL NOT be accepted in type position. Built-in support SHALL include Boolean, all integer and floating/decimal families, String, Char, Null, Duration, Date, Time, DateTime, and every other deterministic built-in literal form. Direct enum cases without payloads SHALL preserve their enum identity. Tuple and object types SHALL use their normal type-expression semantics, with literal-valued object properties available as discriminants. Singleton syntax SHALL NOT invoke arbitrary constructors or functions; the fixed temporal forms SHALL be reduced to checked temporal values without executing code. Canonicalization SHALL follow each supported literal's documented equality and representation rules, including numeric exactness and temporal semantics. Mutable reference identity SHALL NOT be silently converted into value identity.

Canonical singleton identity SHALL include the resolved value type. Integer width and signedness remain distinct. Decimal values use normalized exact coefficient/scale form. String and Char use exact UTF-8 value equality without implicit normalization. Duration uses nanoseconds; civil temporal values use their checked scalar representation. Enum values include declared enum and variant identity; tuples include ordered element types and values; immutable object values use the documented direct-literal value identity.

Binary Float singleton equality SHALL match Zirk's ordered floating equality: positive and negative zero are equal, infinity preserves sign and width, and NaN is rejected because it is not reflexively equal and safe arithmetic reports NaN results. Zone-aware temporal singleton values SHALL include pinned rule-data identity when direct literal syntax exists. `Instant` identity is its timeline value; `TimeZone` identity is its canonical IANA identity or fixed-offset value; `ZonedDateTime` identity includes its instant, canonical zone, and rule-data version; `Period` identity is its exact calendar component tuple and SHALL NOT be equated with a Duration by elapsed-time conversion.

#### Scenario: Date and Time constructor values canonicalize
- **WHEN** two fixed Date or Time constructors denote equal values
- **THEN** they resolve to the same canonical singleton identity

#### Scenario: Date and Time constants canonicalize
- **WHEN** two supported compiler constant values denote equal Date or Time values
- **THEN** they resolve to the same canonical singleton identity

#### Scenario: Exact Duration literals normalize
- **WHEN** equivalent duration suffixes express the same exact nanosecond quantity
- **THEN** the singleton type normalizer treats them as the same value

#### Scenario: Exact Duration constants normalize
- **WHEN** equivalent direct duration literals express the same exact nanosecond quantity
- **THEN** the singleton type normalizer treats them as the same value

#### Scenario: Enum case literal preserves owning enum identity
- **WHEN** a qualified enum case is used directly as a singleton alternative
- **THEN** its identity includes the declared enum and case

#### Scenario: User-defined immutable value participates
- **WHEN** an immutable user-defined value type has no direct literal syntax
- **THEN** it cannot be constructed as a singleton through a function or constructor call

#### Scenario: Structural properties retain literal values
- **WHEN** an object type uses direct singleton literals for discriminating properties
- **THEN** those literal property types retain their exact values inside the structural type

#### Scenario: Mutable identity is not structural value equality
- **WHEN** a mutable class instance is supplied without supported direct literal syntax
- **THEN** compilation rejects the singleton rather than relying on mutable fields or allocation address
