## ADDED Requirements

### Requirement: Compiler-owned IDE semantic data

Editor-facing semantic data SHALL be derived from the compiler's parsed and checked project snapshot. The data SHALL include source-mapped declarations, resolved identifier uses, types, and visibility needed by editor queries, and SHALL not reimplement name or type resolution in the VS Code extension.

#### Scenario: Shared compiler semantics
- **WHEN** an IDE query requests the target of a name or its resolved type
- **THEN** the result agrees with the compiler frontend's resolution and type checking for the same project snapshot

#### Scenario: Editor query dependency direction
- **WHEN** compiler and editor-facing crate dependencies are inspected
- **THEN** IDE query and protocol crates consume frontend analysis without introducing a dependency from lexer, parser, or semantic analysis back into VS Code or LSP transport code

#### Scenario: Source mapping across modules
- **WHEN** a semantic result points to a declaration outside the entry file
- **THEN** its source identity maps to the correct project document and UTF-16 range expected by LSP clients
