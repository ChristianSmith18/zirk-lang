## ADDED Requirements

### Requirement: Filesystem consent is scoped to the effective path and operation
File APIs in this change SHALL check `filesystem.read` and `filesystem.write` independently at each protected host operation. Metadata inspection in the next stage SHALL use the narrower `filesystem.metadata` capability. A full read grant SHALL also permit metadata inspection for the same exact path, but metadata consent SHALL NOT permit reading file contents or writing. Every check SHALL use the effective canonical file path and SHALL be bound to the requesting application/package provenance. Consent for one file SHALL NOT authorize sibling paths; consent for reads SHALL NOT authorize writes. Directory-subtree authorization is not part of this change, and no scope SHALL be inferred from a string prefix. The runtime SHALL protect validation/use against symbolic-link escapes and check/use races, return a typed permission error before a denied effect, and preserve the existing first-use consent, warning, and non-interactive denial behavior.

#### Scenario: Exact read consent does not cover a neighboring file
- **WHEN** an application has consent to read one canonical file and attempts to read another file in the same directory
- **THEN** the second path is checked independently and prompts or is denied according to the current execution mode

#### Scenario: First-use approval updates the project manifest
- **WHEN** application code requests a file capability and the user approves it
- **THEN** Zirk adds the capability to the single `requires` declaration, creating a minimal `.zkinit` for a standalone entry when necessary
- **AND** Zirk separately persists consent for the exact path, operation, project, and requester

#### Scenario: First-use approval creates a minimal manifest
- **WHEN** a user approves a protected file operation from a dependency for a standalone entry that has no `.zkinit`
- **THEN** Zirk creates the minimal application manifest with stable project identity, version `0.1.0`, selected entry, and source root `.`
- **AND** the dependency's own capability declaration remains attributed to that dependency rather than duplicated as a direct application requirement
- **AND** saves the concrete resource approval outside the repository

#### Scenario: Read consent does not cover a write
- **WHEN** an application has `filesystem.read` consent for a file but attempts to truncate or replace it
- **THEN** the write is denied before changing the file unless matching `filesystem.write` consent exists

#### Scenario: Denied filesystem effect remains recoverable
- **WHEN** a filesystem operation is rejected or lacks consent
- **THEN** its API returns typed permission denial and emits the established warning
- **AND** the application continues unless its own code chooses to stop
