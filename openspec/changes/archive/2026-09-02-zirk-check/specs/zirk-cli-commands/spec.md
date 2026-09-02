## ADDED Requirements

### Requirement: Check subcommand in the CLI

The CLI SHALL expose `zirk check` as a first-class subcommand, alongside `zirk build` and `zirk run`.

#### Scenario: Help lists the check subcommand
- **WHEN** `zirk --help` or `zirk check --help` is requested
- **THEN** `check` appears in the list of subcommands
- **AND** its description states that it runs the frontend without producing a native binary

#### Scenario: Running `zirk check` with an entry file
- **WHEN** `zirk check <path>` is invoked
- **THEN** the `zirk check` binary runs the frontend on the given path
- **AND** the command returns the same exit code as if the `zirk-check` binary were invoked directly

### Requirement: Consistent diagnostic presentation for `zirk check`

The `zirk check` subcommand SHALL present diagnostics in the same format as `zirk build`, including human-readable and structured output, and SHALL route them to the same output stream.

#### Scenario: Human-readable error from `zirk check`
- **WHEN** `zirk check` emits a diagnostic without a structured-output flag
- **THEN** the rendered output contains severity, code, location, snippet, cause, and help in the same form as `zirk build`

#### Scenario: Structured output from `zirk check`
- **WHEN** `zirk check` is invoked with a structured-output flag
- **THEN** the output is the same machine-readable format as `zirk build`
