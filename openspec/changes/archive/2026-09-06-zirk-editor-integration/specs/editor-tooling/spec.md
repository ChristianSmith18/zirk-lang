## MODIFIED Requirements

### Requirement: Scope without a language server
This capability SHALL NOT include autocomplete, hover, navigation, or renaming provided by a language server. Compiler diagnostics SHALL be shown through the `editor-cli-bridge` while a full LSP is not available.

#### Scenario: Diagnostics while editing
- **WHEN** code with a type error is written and saved
- **THEN** the editor flags it by invoking the CLI bridge
- **AND** the diagnostic is shown in the editor

#### Scenario: Features that still require an LSP
- **WHEN** the user requests autocomplete, hover, go-to-definition, or rename
- **THEN** the editor does not provide them
- **AND** it documents that they arrive with the LSP in Phase 9

## ADDED Requirements

### Requirement: Command palette integration
The editor SHALL provide commands to check, build, and run the current `.zrk` file through the `editor-cli-bridge`.

#### Scenario: Running Zirk commands from the editor
- **WHEN** the user invokes `Zirk: Check File`, `Zirk: Build File`, or `Zirk: Run File`
- **THEN** the editor invokes the appropriate CLI command
- **AND** it displays the result in an output channel or status bar
