## ADDED Requirements

### Requirement: Browser format operation

The browser package SHALL expose a versioned `format` operation that
accepts UTF-8 Zirk source and returns either the canonically formatted
source or the ordered diagnostics for unformattable input. The operation
SHALL reuse the existing source-byte and diagnostic limits, SHALL appear
in the manifest's operation list and generated TypeScript declarations,
and SHALL run inside the worker without DOM, filesystem, or network
access. Adding the operation SHALL remain backward-compatible for
consumers of `check` and `run`.

#### Scenario: Format valid source in the worker

- **WHEN** a caller invokes `format` with well-formed source within limits
- **THEN** the response returns the canonically formatted source text

#### Scenario: Format invalid source

- **WHEN** `format` receives source with syntax errors
- **THEN** the response returns the ordered compiler diagnostics and no
  formatted text

#### Scenario: Manifest advertises format

- **WHEN** a consumer reads the package manifest
- **THEN** `format` appears alongside `check` and `run` in the operations
  list

#### Scenario: Older consumer unaffected

- **WHEN** a consumer built against the pre-`format` contract invokes
  `check` or `run`
- **THEN** both operations behave exactly as before
