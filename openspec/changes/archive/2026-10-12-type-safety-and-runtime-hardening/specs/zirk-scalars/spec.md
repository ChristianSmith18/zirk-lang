## ADDED Requirements

### Requirement: Binary floating conversions are range-safe

Conversions from binary floating types to integer types SHALL reject non-finite values and values outside the destination range with a controlled language failure. In-range conversion SHALL follow one documented rounding rule across execution backends.

#### Scenario: NaN or infinity is converted to an integer
- **WHEN** a non-finite value reaches an integer conversion
- **THEN** conversion fails deterministically without backend-dependent integer bits

#### Scenario: Floating value exceeds integer bounds
- **WHEN** a finite value is below the minimum or above the maximum of the destination integer type
- **THEN** conversion fails with a controlled range error
