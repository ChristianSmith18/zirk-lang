## MODIFIED Requirements

### Requirement: Universal `to_string()` contract

Every value-producing type SHALL answer `to_string(): String`, through a compiler default or a declared method, and SHALL satisfy the standard `Stringifiable` trait of `std.io` automatically, without declaring it. The trait supplies a concrete default `to_string()`, so a class that declares `to_string()` replaces that body and SHALL mark the method `#override`. Only `Void` and `Never` have no text form. `print`, `println`, and interpolation SHALL accept every other type and route through this conversion.

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

#### Scenario: Class replaces the standard rendering

- **WHEN** a class declares `#override to_string(): String { return this.text; }` and an instance is printed
- **THEN** the printed text is the method's result

#### Scenario: Missing override marker

- **WHEN** a class declares `to_string(): String { ... }` without `#override`
- **THEN** compilation fails with a diagnostic that the method replaces a trait default and needs `#override`
