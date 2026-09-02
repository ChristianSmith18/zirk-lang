# diagnostics-format

## Purpose

Define the contract for compiler diagnostics: severity, stable code, location, cause, and help, per `ZIRK_COMPILER_SPEC.md` section 8.

The format is fixed from day one on purpose: migrating it after several layers already use it is much more expensive than starting off right.

## Requirements

### Requirement: Minimum structure of a diagnostic

Every diagnostic SHALL include severity, a stable code, a location in the source, a cause and, when a clear fix exists, actionable help. This corresponds to `ZIRK_COMPILER_SPEC.md` section 8.

#### Scenario: Complete diagnostic
- **WHEN** an error diagnostic with a known location is constructed
- **THEN** it exposes severity, code, file, line, column, cause, and help

#### Scenario: Missing help
- **WHEN** no clear fix exists for the error
- **THEN** the diagnostic SHALL be emitted without help
- **AND** it SHALL NOT emit generic help with no actionable value

### Requirement: Presentation format

A rendered diagnostic SHALL follow the format of `ZIRK_COMPILER_SPEC.md` section 8: a header with severity and code, location, a source snippet with a marker at the position, and cause and help lines.

#### Scenario: Rendering with a source snippet
- **WHEN** a diagnostic whose location has source available is rendered
- **THEN** the output includes the source line and a marker under the indicated column

#### Scenario: Rendering without source available
- **WHEN** the source is not available
- **THEN** the output retains the header, location, cause, and help, omitting the snippet

### Requirement: Stability of diagnostic codes

Each diagnostic SHALL have a stable code. A published code SHALL NOT be reused for a semantically different error.

#### Scenario: Unique code per error class
- **WHEN** a new diagnostic is defined
- **THEN** it receives a code not previously used

### Requirement: Differentiated severities

The system SHALL distinguish at least error and warning. Warnings SHALL NOT alter the semantics of the program.

#### Scenario: Warnings as errors
- **WHEN** `--warnings-as-errors` mode is enabled
- **THEN** warnings are escalated to errors and compilation fails

### Requirement: Structured output

The diagnostics system SHALL be able to emit output in human-readable format and in structured format for tools.

#### Scenario: JSON mode
- **WHEN** structured output is requested
- **THEN** each diagnostic is serialized preserving severity, code, location, cause, and help
