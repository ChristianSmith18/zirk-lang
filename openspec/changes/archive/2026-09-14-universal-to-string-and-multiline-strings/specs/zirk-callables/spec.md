# zirk-callables Delta

## ADDED Requirements

### Requirement: Callable rendering

Values of function type SHALL render `<Function name>` when referring to a
named function and `<Function>` when anonymous. This replaces the earlier
uniform `<Callable>` rendering.

#### Scenario: Named function reference

- **WHEN** `println(main)` or `main.to_string()` is evaluated
- **THEN** the output is `<Function main>`

#### Scenario: Anonymous callable

- **WHEN** a lambda or closure without a name is rendered
- **THEN** the output is `<Function>`
