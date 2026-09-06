# editor-cli-bridge

## Purpose

Define how editor support talks to the Zirk toolchain while there is no language server: it discovers a usable `zirk`/`zirk-check` executable, invokes the frontend-only check, maps the compiler's JSON diagnostics to editor positions, and exposes build/run commands plus their feedback surfaces (output channel, status bar, tasks).

The bridge is the transport between the editor and the existing CLI — it does not re-implement parsing or checking. When the incremental LSP arrives in Phase 9, this capability is superseded by it.

## Requirements

### Requirement: Executable discovery
The editor-cli-bridge SHALL discover a usable `zirk` or `zirk-check` executable before invoking any command.

#### Scenario: Workspace binary is present
- **WHEN** the workspace contains `target/debug/zirk-check` or `target/release/zirk-check`
- **THEN** the bridge uses that executable

#### Scenario: Executable path is configured
- **WHEN** the user sets `zirk.executablePath`
- **THEN** the bridge uses the configured path

#### Scenario: Fallback to PATH
- **WHEN** no workspace binary or configured path exists
- **THEN** the bridge looks for `zirk` or `zirk-check` on the system PATH

### Requirement: Check command
The editor-cli-bridge SHALL run a frontend-only check on the current `.zrk` file and return structured diagnostics.

#### Scenario: File passes the check
- **WHEN** the bridge invokes the check command on a valid file
- **THEN** it returns an empty diagnostic list and a success status

#### Scenario: File contains an error
- **WHEN** the bridge invokes the check command on an invalid file
- **THEN** it receives a JSON array with one or more diagnostics
- **AND** each diagnostic contains `severity`, `code`, `message`, and `location`

#### Scenario: Check is disabled
- **WHEN** `zirk.checkOnSave` is `false`
- **THEN** the bridge does not run the check on save
- **AND** existing diagnostics remain visible until the next explicit check

### Requirement: Diagnostic mapping
The editor-cli-bridge SHALL map compiler diagnostics to editor positions.

#### Scenario: Single error on a line
- **WHEN** a diagnostic reports `line` 2 and `column` 24
- **THEN** the editor shows a diagnostic starting at line 1, character 23
- **AND** the range spans at least one character

#### Scenario: Diagnostic without a location
- **WHEN** a diagnostic has `location` set to `null`
- **THEN** the editor shows the diagnostic at the top of the file with a zero-length range

### Requirement: Build and run commands
The editor-cli-bridge SHALL expose commands to build or run the current file using the full `zirk` CLI.

#### Scenario: Building a file
- **WHEN** the user invokes the build command
- **THEN** the bridge runs `zirk build <file>`
- **AND** it reports the generated executable path or compilation errors

#### Scenario: Running a file
- **WHEN** the user invokes the run command
- **THEN** the bridge runs `zirk run <file>`
- **AND** it streams stdout and stderr to the output channel

#### Scenario: Backend is unavailable
- **WHEN** the `zirk` binary was built without the backend feature
- **THEN** the bridge reports that `build` and `run` are not available
- **AND** it suggests building with the `backend` feature

### Requirement: Output channel and status bar
The editor-cli-bridge SHALL provide an output channel and a status bar item for compiler feedback.

#### Scenario: After a check
- **WHEN** a check completes
- **THEN** the status bar shows whether the file passed or failed
- **AND** the output channel contains the raw compiler output

#### Scenario: Build or run output
- **WHEN** a build or run command produces output
- **THEN** the output channel streams it to the user

### Requirement: Configuration
The editor-cli-bridge SHALL read configuration from the editor.

#### Scenario: Custom executable path
- **WHEN** the user sets `zirk.executablePath`
- **THEN** the bridge uses that path for all commands

#### Scenario: Toggle check on save
- **WHEN** the user sets `zirk.checkOnSave` to `false`
- **THEN** the bridge stops checking files automatically on save
