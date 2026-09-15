## MODIFIED Requirements

### Requirement: Scope without a language server

This capability SHALL NOT itself implement autocomplete, navigation, or
renaming. Interactive features SHALL be served by the `zirk-lsp` language
server when its executable is available, and otherwise by the
`editor-cli-bridge`: diagnostics through the CLI check, and hover exclusively
from the `zirk check --types` type map — never from the extension's own
parsing — because the compiler is the only source that knows resolved and
inferred types. Autocomplete, go-to-definition, and rename remain Phase 9
work even when the MVP server is present.

#### Scenario: Diagnostics while editing

- **WHEN** code with a type error is written in a `.zrk` file
- **THEN** the editor flags it through the language server when available,
  or through the CLI bridge otherwise
- **AND** the diagnostic is shown in the editor

#### Scenario: Hover shows the resolved type

- **WHEN** the user hovers over a declaration name or an expression
- **THEN** the editor shows the type the compiler resolved for it, including
  inferred types marked as such

#### Scenario: Features that still require an LSP

- **WHEN** the user requests autocomplete, go-to-definition, or rename
- **THEN** the editor does not provide them
- **AND** it documents that they arrive with the full LSP in Phase 9
