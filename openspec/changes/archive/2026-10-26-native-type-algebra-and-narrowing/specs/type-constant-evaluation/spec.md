## Purpose

Provides deterministic, bounded compile-time values for singleton types and other type computations without granting compile-time code access to runtime effects.

## ADDED Requirements

### Requirement: Constant expressions evaluate deterministically under a budget
The compiler SHALL evaluate supported constant literals, pure operators, immutable aggregate constructors, and explicitly constant functions without filesystem, network, environment, clock, randomness, mutable global, task, or other runtime effects. Evaluation SHALL be deterministic and bounded to 100,000 fuel steps, 32 call frames, aggregate nesting depth 64, and 1 MiB per canonical value. The call-frame limit SHALL be safe on the default Rust test-thread stack and SHALL NOT rely on the larger process-main stack. Budget exhaustion SHALL emit source-located `E0482` with the operation category and configured bound. Evaluation SHALL never hang, overflow the compiler stack, truncate a value, or return a partial result.

#### Scenario: Pure constant arithmetic evaluates
- **WHEN** a singleton expression contains supported constant arithmetic
- **THEN** it resolves to the exact canonical value at compile time

#### Scenario: Runtime effect is rejected
- **WHEN** a singleton expression calls Date.today(), reads an environment variable, or performs I/O
- **THEN** compilation rejects it as non-constant and identifies the effectful expression

#### Scenario: Constant function terminates within its budget
- **WHEN** a declared pure constant function evaluates within the configured budget
- **THEN** its result is available to type normalization

#### Scenario: Constant function exceeds its budget
- **WHEN** evaluation exceeds its fuel, recursion, or value-size limit
- **THEN** compilation emits a deterministic budget diagnostic rather than hanging or overflowing the compiler stack

#### Scenario: Invalid constant arithmetic is source located
- **WHEN** a constant expression overflows its resolved integer or Decimal type
- **THEN** compilation emits `E0483` at the operation that overflowed

### Requirement: Singleton constants use owning-type canonical identity
Every type SHALL be able to participate in singleton evaluation when it exposes deterministic constant construction and stable value equality. Built-in support SHALL include Boolean, all integer and floating/decimal families, String, Char, Null, Duration, Date, Time, DateTime, and every other deterministic built-in value form. User-defined immutable value types SHALL be eligible through a checked compile-time constructor and value-equality contract. Canonicalization SHALL follow each type's documented equality and representation rules, including numeric exactness and temporal semantics. Clock-dependent, environment-dependent, or mutable reference identity SHALL NOT be silently converted into value identity.

Canonical singleton identity SHALL include the resolved value type. Integer width and signedness remain distinct. Decimal values use normalized exact coefficient/scale form. String and Char use exact UTF-8 value equality without implicit normalization. Duration uses nanoseconds; civil temporal values use their checked scalar representation. Enum values include declared enum, variant, and recursively canonical payloads; tuples include ordered element types and values; immutable user-defined values use declared type identity and recursively immutable fields.

Binary Float singleton equality SHALL match Zirk's ordered floating equality: positive and negative zero are equal, infinity preserves sign and width, and NaN is rejected by constant evaluation because it is not reflexively equal and safe arithmetic reports NaN results. Zone-aware temporal singleton values SHALL include the pinned rule-data identity. `Instant` identity is its timeline value; `TimeZone` identity is its canonical IANA identity or fixed-offset value; `ZonedDateTime` identity includes its instant, canonical zone, and rule-data version; `Period` identity is its exact calendar component tuple and SHALL NOT be equated with a Duration by elapsed-time conversion.

#### Scenario: Date and Time constants canonicalize
- **WHEN** two const(...) expressions construct equal Date or Time values by distinct valid syntax
- **THEN** they resolve to the same canonical singleton identity

#### Scenario: Exact Duration constants normalize
- **WHEN** equivalent duration suffixes express the same exact nanosecond quantity
- **THEN** the singleton type normalizer treats them as the same value

#### Scenario: User-defined immutable value participates
- **WHEN** a user type meets the constant-construction and value-equality contract
- **THEN** its valid constant instances can be used as singleton type alternatives

#### Scenario: Mutable identity is not structural value equality
- **WHEN** a mutable class instance is supplied as a constant-type value without an immutable identity contract
- **THEN** compilation rejects the singleton rather than relying on mutable fields or allocation address
