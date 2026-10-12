## ADDED Requirements

### Requirement: Civil temporal arithmetic uses checked wide intermediates

Civil date and time operations SHALL detect unrepresentable intermediate or result values before narrowing. `Time ± Duration` SHALL apply modulo-day semantics using an intermediate wide enough to avoid overflow. Date projections SHALL reject years outside the public representation instead of truncating them.

#### Scenario: Time wraps with an extreme duration
- **WHEN** a representable time is combined with an extreme duration
- **THEN** the documented modulo-day result is computed without integer overflow

#### Scenario: Date arithmetic exceeds the public year range
- **WHEN** date arithmetic or projection produces a year outside the supported year representation
- **THEN** the operation returns a controlled range failure and does not wrap or truncate

#### Scenario: Ordinary temporal operation is representable
- **WHEN** a date/time operation remains within its supported range
- **THEN** native and interpreted execution return the same result
