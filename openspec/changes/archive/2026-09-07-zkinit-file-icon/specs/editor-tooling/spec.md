## MODIFIED Requirements

### Requirement: Source file recognition

Editor support SHALL be associated with the `.zrk` extension, with `init.zrk` manifest files, and with `.zkinit` project-initialization files. `.zkinit` files SHALL be a distinct editor language surface (`zirk-init`) so they carry their own file icon, while reusing the Zirk grammar and language configuration.

#### Scenario: Opening a Zirk file
- **WHEN** a file with the `.zrk` extension is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening the project manifest
- **WHEN** the `init.zrk` file is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening a `.zkinit` file
- **WHEN** a file named `.zkinit` is opened
- **THEN** the editor recognizes it as the Zirk-init surface with the dedicated `zirk-init` icon and Zirk syntax highlighting
