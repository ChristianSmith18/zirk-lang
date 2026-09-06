## MODIFIED Requirements

### Requirement: User-defined type categories include the renamed scalar families

The user-defined type categories documentation and diagnostics SHALL refer to `Decimal` as the exact base-ten scalar, `Dec` as its abbreviation, and `FloatN` as the IEEE 754 binary family. `Float` SHALL be an alias for `Float64`. The documentation SHALL recommend `Decimal` as the default fractional type and `FloatN` for performance-sensitive or hardware-integrated binary floating-point.

#### Scenario: Handbook describes `Decimal` as default
- **WHEN** a developer consults the built-in types reference
- **THEN** `Decimal` is described as the default exact fractional type and `FloatN` as the binary family

#### Scenario: Migration note for `Float64`
- **WHEN** a developer consults the type categories or migration sections
- **THEN** the documentation explains that the previous `Float64` is now `Float64` of the binary family and the previous `Float` is now `Decimal`
