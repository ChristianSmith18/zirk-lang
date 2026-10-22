## MODIFIED Requirements

### Requirement: Absence of implicit conversions
The checker SHALL NOT insert implicit conversions between distinct non-numeric types or between numeric types that could lose information. Implicit conversions between numeric types are permitted only when the destination can represent every value of the source type exactly, or when the compiler can prove at compile time that the specific value fits. All other conversions between numeric types require an explicit `as` cast. The sole non-numeric conversion exception SHALL be contextual projection from `Result<T,E>` to `T?`: `Ok(value)` projects to `value`, and `Error(_)` projects to `null`, only when the resolved value-demanding context does not explicitly expect `Result<...>` and is not the operand of a direct `match`.

#### Scenario: Assignment of an incompatible type
- **WHEN** `mut total: Int32 = "cuarenta";` is declared
- **THEN** a diagnostic pointing to the initializer is emitted
- **AND** the cause indicates that there is no implicit conversion from `String` to `Int32`

#### Scenario: Operands of distinct non-numeric types
- **WHEN** `String` and `Int32` are used with `+`
- **THEN** compilation fails because there is no common type between them

#### Scenario: Operands of distinct numeric types with a common type
- **WHEN** `Int8` and `Int32` are used with `+`
- **THEN** the operation is accepted and the result type is `Int32`

#### Scenario: Operands of distinct numeric types with no safe common type
- **WHEN** `UInt64` and `Int64` are used with `+`
- **THEN** compilation fails because no type can represent every value of both exactly

#### Scenario: Narrowing conversion proven safe at compile time
- **WHEN** `mut a: Int8 = 5;` is written with a constant `Int32` literal
- **THEN** the assignment succeeds because `5` is known to fit in `Int8`

#### Scenario: Widening across signedness is safe
- **WHEN** `mut a: Int16 = (5 as UInt8);` is written
- **THEN** the assignment succeeds because every `UInt8` value fits in `Int16`

#### Scenario: Result projects in an ordinary value context
- **WHEN** a `Result<String,E>` expression initializes an unannotated binding
- **THEN** the binding is typed `String?` and receives the success value or `null`

#### Scenario: Explicit Result expectation suppresses projection
- **WHEN** a `Result<T,E>` expression initializes a binding, argument, attribute, or return whose resolved declared type is `Result<T,E>`
- **THEN** the original Result value is passed through without projection

#### Scenario: Direct match suppresses projection
- **WHEN** a `match` expression directly consumes a `Result<T,E>` expression
- **THEN** pattern checking and exhaustiveness operate on the original Result variants

#### Scenario: Generic inference observes the resolved parameter type
- **WHEN** a generic call has an explicit or otherwise resolved parameter type of `Result<T,E>`
- **THEN** the argument remains a Result; otherwise its value projection is considered before inferring a generic parameter from an ordinary value context

#### Scenario: Coalescing consumes projected nullability
- **WHEN** `Env.get<Int32>("PORT") ?? 8080` is type-checked
- **THEN** Result projection produces `Int32?` before `??` selects an `Int32`

## ADDED Requirements

### Requirement: Result projection is determined by the resolved value context
The checker SHALL resolve the surrounding declaration, callable signature, generic instantiation, or direct pattern-match form before deciding whether a Result expression is preserved or projected. Projection SHALL occur once at the expression boundary and SHALL NOT recursively convert unrelated nested values. Diagnostics SHALL identify when projection loses access to the error payload and point to the explicit Result or `match` form that preserves it.

#### Scenario: Explicitly typed Result parameter preserves the argument
- **WHEN** a function parameter is declared `Result<String,E>` and receives a Result expression
- **THEN** the full Result reaches the function

#### Scenario: Ordinary nullable parameter receives a projection
- **WHEN** a function parameter is declared `String?` and receives `Result<String,E>`
- **THEN** the call passes the success value or `null`

#### Scenario: Nested Result is not recursively projected
- **WHEN** a value context receives `Result<Result<T,E1>,E2>` and expects the outer Result's payload type
- **THEN** one projection yields `Result<T,E1>?` and does not project the nested Result

#### Scenario: Direct Result method access follows its explicit receiver
- **WHEN** a caller explicitly invokes a member on a variable declared as `Result<T,E>`
- **THEN** member lookup uses the Result type and does not silently rewrite the receiver
