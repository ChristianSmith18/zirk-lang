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

`.zkinit` files SHALL be recognized as a Zirk-flavored editor language with
the same TextMate grammar (`source.zirk`) and language configuration
(comments, bracket pairs) as `.zrk` files.

#### Scenario: Highlighting inside a `.zkinit` file
- **WHEN** a `.zkinit` file is opened
- **THEN** Zirk syntax highlighting and comment/bracket configuration apply

#### Scenario: No compiler commands on init files
- **WHEN** the active document is a `.zkinit` file
- **THEN** `Zirk: Check File`, `Zirk: Build File`, and `Zirk: Run File` do
  not invoke the CLI for it
