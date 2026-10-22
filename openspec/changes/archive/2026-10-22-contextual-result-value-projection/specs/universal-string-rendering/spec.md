## MODIFIED Requirements

### Requirement: Universal `to_string()` contract
Every value-producing type SHALL answer `to_string(): String`, through a compiler default or a declared method. Only `Void` and `Never` have no text form. `print`, `println`, and interpolation SHALL accept every other type and route through this conversion. When a Result expression is consumed by printing or interpolation, it SHALL first use contextual Result projection; an `Ok(value)` renders the value and an `Error(_)` renders the nullable absence marker. An explicitly invoked `to_string()` on a receiver statically typed as `Result<T,E>` SHALL retain the Result variant representation.

#### Scenario: Collection prints without declaration
- **WHEN** a `List<Int32>` holding `[1, 2, 3]` is printed or interpolated
- **THEN** it produces `[1, 2, 3]`

#### Scenario: Non-value types rejected
- **WHEN** `Void` or `Never` is printed, converted, or interpolated
- **THEN** a diagnostic states that it has no text form

#### Scenario: Result success prints its value
- **WHEN** `println(Env.get("APP_MODE"))` receives `Ok("production")`
- **THEN** it prints `production`

#### Scenario: Result failure prints nullable absence
- **WHEN** `println(Env.get("APP_MODE"))` receives `Error(permission_error)`
- **THEN** it prints the established null marker `<Null>` without rendering or exposing the error payload

#### Scenario: Explicit Result stringification preserves the variant
- **WHEN** `inmut result: Result<Int32, E> = Ok(5); result.to_string()` is invoked
- **THEN** it renders `Ok(5)`
