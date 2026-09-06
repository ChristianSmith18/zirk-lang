# Delta spec: zirk-ir-lowering

## ADDED Requirements

### Requirement: Lowering of civil temporal types

`Date`, `Time`, and `DateTime` SHALL lower to fixed-size value representations (day-count `Int32`/`Int64` for `Date`, nanoseconds-since-midnight `Int64` for `Time`, the pair for `DateTime`) with runtime calls for validation, formatting, and host-clock reads. Constructor arguments SHALL be validated through a runtime call that throws a controlled `InvalidDate`/`InvalidTime` on failure, and `to_string()` SHALL call the runtime's ISO 8601 formatters.

#### Scenario: Validating constructor call
- **WHEN** `Date(y, m, d)` is lowered
- **THEN** the IR calls the runtime date-validation entrypoint which throws `InvalidDate` on an invalid calendar day

#### Scenario: Component access is a load
- **WHEN** `d.year` is evaluated on a `Date`
- **THEN** the IR projects the stored component without a call

#### Scenario: to_string lowers to the runtime formatter
- **WHEN** `d.to_string()` is lowered
- **THEN** the IR calls the runtime ISO formatter for the civil type
