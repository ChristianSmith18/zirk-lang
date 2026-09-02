## ADDED Requirements

### Requirement: Minimal structure of a diagnostic

Every diagnostic SHALL include severity, a stable code, a location in the source, a cause, and, when a clear fix exists, actionable help. This corresponds to `ZIRK_COMPILER_SPEC.md` section 8.

#### Scenario: Complete diagnostic
- **WHEN** an error diagnostic with a known location is constructed
- **THEN** it exposes severity, code, file, line, column, cause, and help

#### Scenario: Missing help
- **WHEN** there is no clear fix for the error
- **THEN** the diagnostic SHALL be emitted without help
- **AND** SHALL NOT emit generic help with no actionable value

### Requirement: Presentation format

A rendered diagnostic SHALL follow the format from `ZIRK_COMPILER_SPEC.md` section 8: a header with severity and code, location, a source excerpt with a position marker, and cause and help lines.

#### Scenario: Rendering with a source excerpt
- **WHEN** a diagnostic whose location has source available is rendered
- **THEN** the output includes the source line and a marker under the indicated column

#### Scenario: Rendering without source available
- **WHEN** the source is not available
- **THEN** the output retains header, location, cause, and help, omitting the excerpt

### Requirement: Stability of diagnostic codes

Each diagnostic SHALL have a stable code. A published code SHALL NOT be reused for a semantically distinct error.

#### Scenario: Unique code per error class
- **WHEN** a new diagnostic is defined
- **THEN** it receives a previously unused code

### Requirement: Differentiated severities

The system SHALL distinguish at least error and warning. Warnings SHALL NOT alter the program's semantics.

#### Scenario: Warnings as errors
- **WHEN** `--warnings-as-errors` mode is enabled
- **THEN** warnings are elevated to errors and compilation fails

### Requirement: Structured output

The diagnostics system SHALL be able to emit both a human-readable format and a structured format for tooling.

#### Scenario: JSON mode
- **WHEN** structured output is requested
- **THEN** each diagnostic is serialized preserving severity, code, location, cause, and help
