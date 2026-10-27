## ADDED Requirements

### Requirement: Float128 execution and formatting preserve binary128 semantics
Every supported native target SHALL execute `Float128` operations with IEEE 754 binary128 precision and outcomes. Conversion, comparison, arithmetic, and formatting SHALL NOT silently narrow through `Float64`. `to_string()` SHALL use a representation that round-trips every finite `Float128`; `format(spec)` SHALL apply the documented format and precision directly to the binary128 value. A target unable to provide the required behavior SHALL fail compilation or linking with a diagnostic identifying `Float128` and the target.

#### Scenario: Windows executes binary128 values
- **WHEN** a `Float128` program is compiled and run on Windows x86_64
- **THEN** arithmetic, comparisons, conversions, and output execute without unresolved or crashing soft-float helper calls and match the specified binary128 results

#### Scenario: Formatting preserves precision beyond Float64
- **WHEN** a finite `Float128` value cannot be represented exactly as `Float64` and is converted with `to_string()`
- **THEN** the text retains enough significant digits to parse back to the same binary128 value

#### Scenario: Explicit Float128 format precision is honored
- **WHEN** `Float128.format(spec)` is called with a valid precision specification
- **THEN** rounding and emitted digits are computed from the binary128 value without an intermediate `Float64` conversion

#### Scenario: Binary128 edge values match across targets
- **WHEN** supported native targets execute the same `Float128` cases, including subnormal, signed zero, infinity, overflow, and underflow boundaries
- **THEN** they produce equivalent observable values and controlled failures under the Zirk scalar contract
