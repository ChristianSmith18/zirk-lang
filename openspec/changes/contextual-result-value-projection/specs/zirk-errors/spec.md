## MODIFIED Requirements

### Requirement: Expected failure uses mandatory Result handling
The language SHALL define `Result<T,E>` as `Ok(T)` or `Error(E)`. A direct `match` over a Result expression and a value context whose resolved expected type is explicitly `Result<...>` SHALL preserve both variants. In every other value-demanding context, the language SHALL project `Ok(value)` to `value` and `Error(_)` to `null`, producing `T?`, when `T` has a runtime nullable representation. A Result with a payload that has no nullable representation SHALL require explicit matching in value contexts and SHALL produce a compile-time diagnostic otherwise. Projection SHALL discard the error payload; code that needs the error SHALL preserve the Result and inspect it with `match`. A Result used as a standalone expression statement SHALL remain a must-use error unless explicitly discarded with `_ = expression`.

The public Result API SHALL retain `map`, `map_error`, `and_then`, `or_else`, and `or_throw` as composition operations. It SHALL NOT expose status, extraction, fallback, or unwrap helpers (`is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, or `unwrap_error`); callers SHALL use direct `match` for inspection or contextual projection for value use.

#### Scenario: Direct match retains both variants
- **WHEN** `match Env.get("APP_MODE") { Ok(value) => value, Error(error) => error.message() }` is evaluated
- **THEN** the match receives `Result<String, EnvironmentError>` and can inspect either variant

#### Scenario: Expected failure is matched
- **WHEN** a function returns `Result<User,LoadError>` and the caller exhaustively matches `Ok` and `Error`
- **THEN** both outcomes are statically handled and the match retains the original variants

#### Scenario: Inferred binding stores the projected nullable value
- **WHEN** `inmut mode = Env.get("APP_MODE");` is evaluated
- **THEN** `mode` has type `String?`, with the successful string or `null`

#### Scenario: Explicit Result annotation preserves the error
- **WHEN** `inmut result: Result<String, EnvironmentError> = Env.get("APP_MODE");` is evaluated
- **THEN** `result` retains `Ok` or the original `Error`

#### Scenario: Failure projects to null and loses its error payload
- **WHEN** a value-demanding context consumes `Error(permission_error)` as `Result<T,E>`
- **THEN** that context receives `null` of type `T?` and cannot inspect `permission_error`

#### Scenario: Null coalescing supplies a concise fallback
- **WHEN** `inmut port = Env.get<Int32>("PORT") ?? 8080;` is evaluated
- **THEN** `port` is the parsed integer on success or `8080` when projection produces `null`

#### Scenario: A Result expression statement is not silently ignored
- **WHEN** `Env.get("APP_MODE");` appears as a standalone expression statement
- **THEN** compilation rejects it as an unhandled must-use result
- **AND** `_ = Env.get("APP_MODE");` is the explicit discard form

#### Scenario: Result is ignored
- **WHEN** a `Result`-producing expression appears as an unused standalone statement
- **THEN** compilation fails and suggests handling it or writing `_ = expression`

#### Scenario: Redundant Result helper calls are rejected
- **WHEN** source calls `result.ok_or_null()`, `result.unwrap()`, or `result.is_error()`
- **THEN** compilation reports that Result inspection uses `match` and value use relies on contextual projection
