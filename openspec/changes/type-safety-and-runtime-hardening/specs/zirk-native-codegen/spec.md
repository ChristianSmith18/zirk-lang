## ADDED Requirements

### Requirement: Floating-to-integer code generation is checked

Native code generation SHALL check finiteness and destination range before every floating-to-integer conversion. An out-of-range or non-finite operand SHALL use the language's controlled conversion failure path and SHALL never reach an LLVM conversion with undefined result semantics.

#### Scenario: Out-of-range conversion is compiled
- **WHEN** generated code converts a floating value outside the destination integer range
- **THEN** it branches to a controlled failure before executing the conversion instruction

#### Scenario: In-range conversion is compiled
- **WHEN** generated code converts a finite representable floating value
- **THEN** it applies the same rounding rule as the interpreter

### Requirement: Optimized code preserves checked language semantics

Any non-baseline LLVM optimization level enabled for release code SHALL preserve the observable values, diagnostics, and controlled failure categories of the unoptimized native build and the interpreter for the supported corpus and defined edge cases.

#### Scenario: Optimized overflow and conversion paths are compared
- **WHEN** optimized code is checked against baseline native and interpreted execution
- **THEN** all outputs and failure categories agree for checked arithmetic and numeric boundary fixtures
