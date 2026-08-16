## ADDED Requirements

### Requirement: Authority-bearing commands validate signed approval incrementally
Before build-time or runtime code executes, relevant CLI commands SHALL compare project name/location, manifest, lockfile, permission, requester, phase, and approval fingerprints. They SHALL take a fast path when unchanged and recompute only affected requester graph segments when changed.

#### Scenario: Dependency requester updated
- **WHEN** an approved dependency receiving filesystem authority changes version or integrity
- **THEN** the CLI stops and requests new approval even when textual permission scope is unchanged

### Requirement: Permission management commands are auditable
The CLI SHALL provide `zirk permissions show`, `diff`, `approve`, `revoke`, and `history`. Interactive approval SHALL show exact scope, phase, call/requester path, and manifest diff. CI SHALL consume a protected explicit policy and fail with a diff when authority widens.

#### Scenario: Noninteractive build lacks approval
- **WHEN** CI encounters a permission fingerprint absent from its protected policy
- **THEN** the command fails without prompting and prints a machine-readable authorization diff
