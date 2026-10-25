# zkinit-editor-recognition

## Purpose

Editor recognition for Zirk project-initialization files named `.zkinit`:
a dedicated file icon and Zirk-based syntax support, distinct from regular
`.zrk` sources.

## Requirements

### Requirement: `.zkinit` files use the dedicated init icon

The VS Code extension SHALL associate files named `.zkinit` with the
`zirk-init.svg` icon (Zirk mark with configuration gear), for both light
and dark editor themes. The `.zrk` language icon SHALL remain `logo.svg`.

#### Scenario: Opening a `.zkinit` file
- **WHEN** a file named `.zkinit` exists in the workspace
- **THEN** the file explorer and editor tab show the `zirk-init.svg` icon

#### Scenario: `.zrk` files are unaffected
- **WHEN** a file with the `.zrk` extension is opened
- **THEN** it keeps the existing `logo.svg` language icon

### Requirement: `.zkinit` files keep Zirk syntax support

`.zkinit` files SHALL retain the dedicated `zirk-init` language ID, icon, comment rules, and bracket pairs, while using a configuration-aware TextMate grammar that scopes project sections, field keys, built-in/custom type names, option values, strings, numbers, booleans, punctuation, and comments. The grammar SHALL continue to follow the active editor theme and SHALL NOT apply `.zkinit` scopes to `.zrk` source files.

#### Scenario: Configuration roles are distinguished
- **WHEN** a `.zkinit` contains `project`, `name`, a custom type, `application`, and a path string
- **THEN** each token receives a scope that distinguishes its configuration role

#### Scenario: Highlighting inside a `.zkinit` file
- **WHEN** a `.zkinit` file is opened
- **THEN** configuration-aware Zirk syntax scopes, comment rules, and bracket pairs apply

#### Scenario: `.zrk` source highlighting is unchanged
- **WHEN** the same identifiers and strings appear in a `.zrk` source file
- **THEN** the existing Zirk source scopes apply without configuration-only highlighting

#### Scenario: Theme remains readable
- **WHEN** the configuration grammar is used with dark and light themes
- **THEN** colors remain theme-aware and the recommended palette roles remain distinguishable

#### Scenario: No compiler commands on init files
- **WHEN** the active document is a `.zkinit` file
- **THEN** `Zirk: Check File`, `Zirk: Build File`, and `Zirk: Run File` do
  not invoke the CLI for it

### Requirement: `.zkinit` highlighting follows the agreed role palette

The extension SHALL provide role scopes for the agreed configuration palette: section names use the Zirk violet role, field keys the warm yellow role, built-in types teal, user-defined types green, enum-like values orange, strings and versions pink, numbers blue, and comments muted gray italic. Theme adaptation SHALL preserve readable contrast and SHALL NOT hard-code semantic token colors into unrelated languages.

#### Scenario: Project and global manifest use the role palette
- **WHEN** a `.zkinit` contains project fields and a custom typed global
- **THEN** section, field, custom type, option, string, and boolean scopes remain visually distinguishable

#### Scenario: Brand palette is disabled
- **WHEN** the user disables Zirk brand colors
- **THEN** the extension retains theme-native readable scopes without affecting other languages
