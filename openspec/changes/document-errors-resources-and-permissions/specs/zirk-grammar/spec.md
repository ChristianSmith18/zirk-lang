## ADDED Requirements

### Requirement: Failure and resource syntax is unambiguous
The grammar SHALL accept `throws T | U` after a return type, `throw expression`, exact `throw;` inside a catch, `try` followed by pattern-shaped `catch Type(binding)` clauses and optional `finally`, and `match acquisition with binding` including grouped acquisitions. Historical `catch<Type> name` SHALL be rejected with migration guidance.

#### Scenario: Typed catch parses
- **WHEN** source contains `catch NetworkError.Timeout(duration) { retry(duration); }`
- **THEN** the parser produces a typed variant catch pattern without a guard

### Requirement: Permission manifests use requires and permissions
The manifest grammar SHALL accept library `requires`, application `permissions`, operation-specific scopes, and `during: build | runtime | both`. It SHALL reject a top-level `compile_permissions` block with guidance to move the phase into the relevant grant.

#### Scenario: Build-only filesystem grant
- **WHEN** `init.zrk` grants a filesystem read operation with `during: build`
- **THEN** the manifest AST preserves the operation, scope, and phase separately
