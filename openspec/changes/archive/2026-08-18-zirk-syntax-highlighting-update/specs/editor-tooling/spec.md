## MODIFIED Requirements

### Requirement: Source file recognition
Editor support SHALL be associated with the `.zrk` extension and with `init.zrk` manifest files.

#### Scenario: Opening a Zirk file
- **WHEN** a file with extension `.zrk` is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening the project manifest
- **WHEN** the `init.zrk` file is opened
- **THEN** the editor recognizes it as the Zirk language

### Requirement: Complete lexicon coverage
Highlighting SHALL cover **all** keywords recognized by the Phase 3 lexer, not only those of the implemented subset. This includes structured concurrency keywords, resource pattern matching, decorator modifiers and manifest configuration.

#### Scenario: Keyword of the implemented subset
- **WHEN** the source contains `fn`, `mut`, `inmut`, `if`, `else` or `return`
- **THEN** they are highlighted as keywords

#### Scenario: Concurrency and resource-match keyword
- **WHEN** the source contains `task`, `await`, `select`, `parallel`, `thread`, `match`, `with`, `do`, `yield`, `throw`, `throws` or `commit`
- **THEN** they are highlighted as control or storage keywords

#### Scenario: init.zrk manifest keyword
- **WHEN** the source contains `project`, `build_targets`, `globals`, `permissions`, `requires` or `during`
- **THEN** they are highlighted as keywords or structural blocks

### Requirement: Distinction by role
Highlighting SHALL distinguish lexical categories from one another: control flow, modifiers, types (including floats and temporal types), literals (including Unicode characters and regular expressions), operators (including `is` and the pipe `|>`), format markers in strings, and comments.

#### Scenario: Format marker in string
- **WHEN** the source contains a string with `:variable|modifier`
- **THEN** the colon, the placeholder name and the format modifier are distinguished from the normal string text

#### Scenario: Native and user library imports
- **WHEN** the source contains `import { stdin } from std.io;` and `import { User } from "./user";`
- **THEN** `std.io` is highlighted as a native namespace
- **AND** `from` is highlighted as control
- **AND** `"./user"` is highlighted as a text literal

#### Scenario: Named arguments and shorthand
- **WHEN** the source contains `host: "localhost"` or the shorthand `timeout:`
- **THEN** the identifiers before the colon are highlighted as call arguments
