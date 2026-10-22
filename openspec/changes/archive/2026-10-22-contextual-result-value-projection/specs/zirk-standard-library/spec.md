## MODIFIED Requirements

### Requirement: Privileged APIs preserve denial explicitly
Filesystem, network, process, environment, system, and secret APIs SHALL retain permission, decoding, platform, and cancellation failures in their declared typed `Result<T,E>` outcome. When a caller uses that outcome in an ordinary value-demanding context, the language's contextual Result projection SHALL map any `Error(E)`, including permission denial, to `null`; this loss of error detail SHALL be explicit in API documentation. APIs SHALL NOT add separate nullable/default convenience methods that duplicate projection and `??`. Callers that need to distinguish denial from absence SHALL preserve the Result with an explicit `Result<...>` type or direct `match`.

#### Scenario: Explicit inspection preserves a permission denial
- **WHEN** `match Env.get("APP_MODE")` receives a denied environment read
- **THEN** the caller can inspect the typed `EnvironmentPermissionError`

#### Scenario: Ordinary environment read projects a denial
- **WHEN** a denied `Env.get("APP_MODE")` is assigned to an unannotated value binding
- **THEN** the binding receives `null` and has type `String?`

#### Scenario: Fallback composes with projection
- **WHEN** `Env.get<Int32>("PORT") ?? 8080` receives any `Error`
- **THEN** the expression evaluates to `8080`

#### Scenario: Environment projection uses a fallback
- **WHEN** `Env.get<Int32>(name) ?? fallback` receives any `Error`
- **THEN** it evaluates to `fallback`
- **AND** a direct match is required to distinguish permission denial from absence or another host failure

#### Scenario: Removed environment fallback helper is rejected
- **WHEN** source calls `Env.get_or(name, fallback)` or `Env.get_or_null(name)`
- **THEN** compilation reports that callers use `Env.get`, explicit Result handling, or projection with `??`

## ADDED Requirements

### Requirement: Standard Result APIs avoid redundant extraction helpers
The standard library SHALL expose `Result<T,E>` variants and composition operations without separate status, extraction, fallback, or unwrap helpers. It SHALL remove `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, and `unwrap_error`. Environment reads SHALL use `Env.get<T>(name): Result<T, EnvironmentError>` as the single retrieval operation for present, absent, parse-error, permission-error, and host-error outcomes.

#### Scenario: Environment value is read without a helper method
- **WHEN** `inmut mode = Env.get("APP_MODE");` is evaluated
- **THEN** the inferred value is `String?`

#### Scenario: Detailed environment outcome remains available
- **WHEN** a caller declares `inmut result: Result<String, EnvironmentError> = Env.get("APP_MODE");`
- **THEN** the result's typed variants remain available for direct matching

#### Scenario: Result composition remains available
- **WHEN** a caller transforms a successful value or error using `map`, `map_error`, `and_then`, `or_else`, or `or_throw`
- **THEN** the operation composes the Result without projecting it
