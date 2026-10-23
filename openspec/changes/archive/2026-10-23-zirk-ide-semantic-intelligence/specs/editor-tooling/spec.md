## MODIFIED Requirements

### Requirement: Scope without a language server

The editor extension SHALL use `zirk-lsp` for compiler-backed semantic features when the server is available, including diagnostics, hover, symbols, formatting, navigation, references, rename, completion, signature help, semantic tokens, inlay hints, workspace symbols, and code actions as advertised by the server. When the server is unavailable, the `editor-cli-bridge` SHALL provide only the fallback features it can implement correctly: compiler diagnostics, type hover, formatting, and explicit check/build/run commands. The extension SHALL show whether the LSP is active or fallback mode is active and SHALL NOT imply that unsupported semantic features are available.

#### Scenario: Diagnostics while editing
- **WHEN** code with a type error is written in a `.zrk` file
- **THEN** the editor flags it through the language server when available, or through the CLI bridge otherwise
- **AND** the diagnostic is shown in the editor

#### Scenario: Hover shows the resolved type
- **WHEN** the user hovers over a declaration name or an expression
- **THEN** the editor shows the type the compiler resolved for it, including inferred types marked as such

#### Scenario: Features that still require an LSP
- **WHEN** the user requests autocomplete, go-to-definition, or rename
- **THEN** the editor provides them when the corresponding LSP capability is active
- **AND** it does not claim they are available through the CLI-only fallback

#### Scenario: Language server available
- **WHEN** `zirk-lsp` starts successfully for a `.zrk` workspace
- **THEN** the extension routes semantic requests through the server
- **AND** shows the active server state

#### Scenario: Language server unavailable
- **WHEN** `zirk-lsp` cannot be located or fails to start
- **THEN** the extension uses the CLI bridge for its supported fallback features
- **AND** identifies unavailable semantic navigation and completion features in the language status or output

#### Scenario: No duplicate feature providers
- **WHEN** the language server is active for a document
- **THEN** the CLI bridge does not publish competing diagnostics, hover, or formatting results for that document

#### Scenario: Semantic highlighting falls back
- **WHEN** semantic tokens are missing or temporarily unavailable
- **THEN** the Zirk syntax grammar continues to provide lexical highlighting

## ADDED Requirements

### Requirement: Debugger integration

The editor SHALL expose Zirk debugging through a DAP adapter when a supported execution target is configured. The debug UI SHALL report target capability limits and SHALL distinguish runtime values from compile-time type information.

#### Scenario: Debug a supported interpreter program
- **WHEN** a supported interpreter launch configuration starts a debug session
- **THEN** the editor exposes verified breakpoints, stepping, stack frames, scopes, and runtime variables supported by that target

#### Scenario: Unsupported debug target
- **WHEN** the selected target does not support a requested debug operation
- **THEN** the editor communicates the limitation and does not display fabricated values
