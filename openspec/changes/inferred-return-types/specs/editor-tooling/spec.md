## MODIFIED Requirements

### Requirement: Scope without a language server

This capability SHALL NOT include autocomplete, navigation, or renaming provided by a language server. Compiler diagnostics SHALL be shown through the `editor-cli-bridge` while a full LSP is not available. **Hover is the single exception**: the editor SHALL provide hover information for declarations and expressions, sourced exclusively from the `zirk check --types` type map through the `editor-cli-bridge` — never from the extension's own parsing — because the compiler is the only source that knows resolved and inferred types.

#### Scenario: Diagnostics while editing
- **WHEN** code with a type error is written and saved
- **THEN** the editor flags it by invoking the CLI bridge
- **AND** the diagnostic is shown in the editor

#### Scenario: Hover shows the resolved type
- **WHEN** the user hovers over a declaration name or an expression
- **THEN** the editor shows the type the compiler resolved for it, including inferred types marked as such

#### Scenario: Features that still require an LSP
- **WHEN** the user requests autocomplete, go-to-definition, or rename
- **THEN** the editor does not provide them
- **AND** it documents that they arrive with the LSP in Phase 9
