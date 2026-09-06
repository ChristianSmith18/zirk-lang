# Delta spec: zirk-ir-lowering

## ADDED Requirements

### Requirement: Lowering of civil temporal types

`Date`, `Time`, and `DateTime` SHALL lower to fixed-size value representations — a day-count `Int64` since 1970-01-01 for `Date`, nanoseconds-since-midnight `Int64` for `Time`, and epoch nanoseconds in an `Int128` for `DateTime` — with runtime calls for validation, civil-component projection, formatting, and host-clock reads. Constructor arguments SHALL be validated through a runtime call whose `false` answer throws a controlled `InvalidDate`/`InvalidTime`, and `to_string()` SHALL call the runtime's ISO 8601 formatters.

#### Scenario: Validating constructor call
- **WHEN** `Date(y, m, d)` is lowered
- **THEN** the IR calls the runtime date-validation entrypoint and throws `InvalidDate` on an invalid calendar day

#### Scenario: Component access projects the representation
- **WHEN** `d.year` is evaluated on a `Date`
- **THEN** the IR calls the runtime civil-projection entrypoint over the stored day count

#### Scenario: to_string lowers to the runtime formatter
- **WHEN** `d.to_string()` is lowered
- **THEN** the IR calls the runtime ISO formatter for the civil type
