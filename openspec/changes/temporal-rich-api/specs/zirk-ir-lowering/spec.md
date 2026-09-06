# Delta spec: zirk-ir-lowering

## ADDED Requirements

### Requirement: Lowering of civil temporal members

The IR SHALL lower calendrical properties and `with_*`/`start_of`/
`end_of`/`format` member calls on `Date`/`Time`/`DateTime` to runtime
entrypoints over the existing integer representations. The IR SHALL
lower `parse` through the shared `(ok, value)` probe into a
`Result<_, ParseError>`, and SHALL lower the `is_*`/`is_between` queries
to pure IR comparisons. The IR SHALL widen the `Date` day count to epoch
nanoseconds for the `Date ± Duration` → `DateTime` promotion.

#### Scenario: Query lowers without a runtime call
- **WHEN** `d.is_between(a, b)` is lowered
- **THEN** the IR contains only integer comparisons and boolean logic — no `zirk_rt_*` call

#### Scenario: Parse lowers to a Result
- **WHEN** `Date.parse("2026-09-06")` is lowered
- **THEN** the IR probes validity through the runtime and builds `Result<Date, ParseError>` on the two branches

#### Scenario: Format lowers to the runtime formatter
- **WHEN** `d.format("DD/MM/YYYY")` is lowered
- **THEN** the IR calls the runtime pattern formatter with the interned pattern string
