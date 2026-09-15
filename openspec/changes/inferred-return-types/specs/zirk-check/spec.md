## ADDED Requirements

### Requirement: Type-map output mode

`zirk check` SHALL accept a `--types` flag that, alongside diagnostics, emits a structured JSON type map for the checked crate: one entry per declaration name and per resolved expression, carrying the file, the span, the symbol kind (function, method, variable, parameter, field, expression), and the rendered type text. The type map SHALL include inferred types — an unannotated function reports the type the checker inferred — and SHALL distinguish inferred entries from declared ones.

The `--types` mode SHALL remain frontend-only: it produces no executable, object file, or backend artifact, and works on the `zirk-check` binary built without the `backend` feature.

#### Scenario: Type map for an inferred function

- **WHEN** `zirk check --types` runs over `fn hola() { stdout.println("x"); }`
- **THEN** the output contains an entry for `hola` with rendered type `fn(): Void`
- **AND** the entry is marked as inferred

#### Scenario: Type map for a declared variable

- **WHEN** the checked file contains `mut count: Int32 = 0;`
- **THEN** the output contains an entry for `count` with rendered type `Int32`
- **AND** the entry is marked as declared

#### Scenario: Type map on a file with errors

- **WHEN** the checked file contains errors
- **THEN** the type map still reports the entries the checker resolved
- **AND** the diagnostics are emitted as usual

#### Scenario: Type map is frontend-only

- **WHEN** `zirk check --types` runs on a binary built without the `backend` feature
- **THEN** the type map is emitted without error
