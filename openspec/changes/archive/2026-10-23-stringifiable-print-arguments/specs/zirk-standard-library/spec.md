## ADDED Requirements

### Requirement: Output functions accept heterogeneous Stringifiable arguments

`stdout.print`, `stdout.println`, `stderr.print` and `stderr.println` SHALL be variadic over `Stringifiable`. They SHALL render the arguments left to right, join them with one space, and write the result; `println` SHALL add one line break and `print` SHALL add none. A call with no arguments SHALL write only the line break for `println` and nothing for `print`. The standard-library source SHALL perform the joining and call `to_string()` itself, delegating only single-text writes to the runtime.

#### Scenario: Mixed argument types

- **WHEN** a program calls `stdout.println("count", 2, true, label)` where `label` is a class whose `to_string()` returns `items`
- **THEN** the output is `count 2 true items` followed by one line break

#### Scenario: Nullable argument

- **WHEN** a program calls `stdout.println("done", 3.5, null)`
- **THEN** the output is `done 3.5 <Null>`

#### Scenario: Argument with no text form

- **WHEN** a `Void` expression is passed to `stdout.println`
- **THEN** compilation fails with a diagnostic that there is no conversion from `Void` to `Stringifiable`
