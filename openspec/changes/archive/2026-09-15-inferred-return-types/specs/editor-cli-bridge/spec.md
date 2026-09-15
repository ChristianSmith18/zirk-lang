## ADDED Requirements

### Requirement: Type-map retrieval

The editor-cli-bridge SHALL obtain the compiler's type map for the current `.zrk` file by invoking `zirk check --types`, reusing the same executable discovery as the diagnostic check. The type map SHALL be refreshed when the file is checked (on save or on explicit command), and SHALL be cached between refreshes so hover requests do not re-invoke the compiler.

#### Scenario: Type map refreshed with the check

- **WHEN** a `.zrk` file is saved and the bridge runs the check
- **THEN** the bridge also retrieves and caches the type map

#### Scenario: Stale cache is not shown forever

- **WHEN** a hover is requested after a successful re-check
- **THEN** the bridge answers from the newly refreshed type map, not the previous one

#### Scenario: Type-map retrieval failure

- **WHEN** the type-map invocation fails or returns malformed output
- **THEN** the bridge leaves the previous cache in place and reports no hover rather than an incorrect one

### Requirement: Hover mapping

The editor-cli-bridge SHALL map an editor position (line, character) to the type-map entry whose span contains it, and SHALL return the rendered type text for that entry — `fn name(params): Return`, `name: Type`, or the expression's type — formatted as a code block, with inferred types marked as inferred.

#### Scenario: Hover over an inferred function name

- **WHEN** the user hovers over `hola` in `fn hola() { ... }`
- **THEN** the hover shows `fn hola(): Void` marked as inferred

#### Scenario: Hover over a declared variable

- **WHEN** the user hovers over `count` in `mut count: Int32 = 0;`
- **THEN** the hover shows `count: Int32`

#### Scenario: Hover outside any typed span

- **WHEN** the position maps to no type-map entry
- **THEN** no hover is produced
