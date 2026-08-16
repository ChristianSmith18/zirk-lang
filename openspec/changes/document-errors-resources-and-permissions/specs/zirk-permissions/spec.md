## ADDED Requirements

### Requirement: Libraries request and applications grant scoped authority
Libraries SHALL declare requirements through `requires`; only applications SHALL grant authority through `permissions`. Each operation grant SHALL identify `during: build`, `runtime`, or `both`. Libraries SHALL NOT enlarge grants, and a separate public `compile_permissions` block SHALL NOT exist.

#### Scenario: Dependency requirement is not granted
- **WHEN** a dependency requires runtime network access absent from the application permissions
- **THEN** the build stops before executing dependent code and reports the exact requirement path

### Requirement: Permission effects propagate automatically
The compiler SHALL infer permission needs from privileged APIs through functions, methods, lambdas, closures, generators, and higher-order callables. Effects SHALL remain in compiler/callable metadata and call-chain diagnostics without adding permission syntax to `Fn`. The application manifest SHALL remain the grant location.

#### Scenario: File-reading callable is stored
- **WHEN** a file-reading function is assigned to a compatible `Fn` and invoked indirectly
- **THEN** filesystem-read authority remains required and traceable through the indirect call

### Requirement: Declaration and developer consent are separate
Zirk SHALL store signed approval outside the repository and bind it to project name, canonical project location, user, device, exact permission phases/scopes, requester package versions/integrity, and transitive requirement paths. Moving or renaming the project, widening authority, updating a requester, adding a requester, or changing its path/phase SHALL invalidate the applicable approval. Repository edits alone SHALL never grant authority.

#### Scenario: Library edits init.zrk
- **WHEN** repository contents add a broad permission without a matching signed approval
- **THEN** Zirk stops before executing build or runtime code and requests explicit developer consent

#### Scenario: Project moves
- **WHEN** an unchanged project is executed from a different canonical location
- **THEN** its previous approval is not reused and Zirk asks for authorization at the new location

### Requirement: Permission validation is secure and incremental
Zirk SHALL compare signed manifest, permission, lockfile, requester-subgraph, and project-identity fingerprints on each authority-bearing command. Unchanged state SHALL take a cached fast path; changed state SHALL recompute affected graph segments. Missing, corrupt, or unverifiable cache/approval data SHALL grant nothing and trigger reconstruction or reapproval.

#### Scenario: No permission-relevant input changed
- **WHEN** a previously approved project compiles again with matching fingerprints
- **THEN** Zirk continues without a full dependency rescan or another prompt

### Requirement: Interactive approval is precise and fail-closed
An interactive trusted CLI SHALL show permission scope, phase, call path, requester/dependency path, and manifest diff before offering allow-once, approve-exact-set, or deny. Accepted edits SHALL use the manifest parser and formatter. Dynamic scopes SHALL require manual choice; unrestricted `all` SHALL require typing the project name and SHALL NOT be accepted by generic confirmation. Noninteractive CI SHALL use protected explicit policy. Deployed programs SHALL never prompt or edit their manifest.

#### Scenario: Broad permission requested
- **WHEN** a project requests `filesystem.read: all`
- **THEN** the CLI displays a critical warning and requires the exact project name before recording consent

### Requirement: Privileged APIs enforce normalized operation scopes
Filesystem access SHALL validate canonical paths and symlink escape; network access SHALL validate protocol, host, port, redirects, DNS results, and reconnects; environment access SHALL distinguish secrets; process access SHALL validate canonical executable and allowed argument forms; shell execution SHALL require a separate high-risk grant. Runtime dynamic denial SHALL return typed `PermissionDeniedError` through the operation's `Result`.

#### Scenario: Process argument is outside grant
- **WHEN** a project authorized for `git status` attempts `git clean -fd`
- **THEN** execution is denied before the child process starts

### Requirement: Environment access is typed and secret-safe
The standard library SHALL provide `Environment` and exact alias `Env` with static getters, nullable/default/required access, secret access, containment, and name listing. Secret reads SHALL return `SecretString`, and secrets SHALL be redacted from diagnostics, logs, stack traces, and tooling output unless deliberately revealed under an authorized boundary.

#### Scenario: Unapproved environment variable
- **WHEN** `Env.get("DATABASE_URL")` executes without a matching runtime grant
- **THEN** it returns `Error(PermissionDeniedError)` and does not reveal the variable

### Requirement: Permission approval is inspectable and revocable
The CLI SHALL provide show, diff, approve, revoke, and history operations. History SHALL identify project name/location, requester, scope, phase, approver, and timestamp without storing or displaying secrets. Revocation SHALL take effect before the next privileged execution.

#### Scenario: Approval is revoked
- **WHEN** the developer revokes a project's permission fingerprint
- **THEN** the next authority-bearing command requires fresh approval before executing privileged code
