# zirk-data-types Specification Delta

## MODIFIED Requirements

### Requirement: Closed data-only enums
Traditional and algebraic enums SHALL be closed data declarations and SHALL NOT contain user-defined methods. Traditional cases SHALL expose native `.name`, `.value`, and `to_string()` behavior without implicit mapping conversion or declaration order; the enum type itself SHALL expose the built-in static members `count`, `keys()`, `values()`, `from_name()`, and `from_value()` (see `enum-static-members`). Algebraic payloads SHALL be extracted only through exhaustive match.

#### Scenario: Enum method rejected
- **WHEN** an enum body declares `fn to_celsius()`
- **THEN** compilation fails and domain behavior must be expressed by an external function with match
