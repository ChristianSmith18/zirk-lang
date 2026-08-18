# zirk-permissions Specification

## Purpose
TBD - created by archiving change document-errors-resources-and-permissions. Update Purpose after archive.
## Requirements
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

### Requirement: Network connect and listen authority remain distinct
Outbound authority SHALL use scoped `network.connect.origins`; inbound bind
authority SHALL use scoped `network.listen.addresses`. DNS answers SHALL NOT
grant authority: every resolved address SHALL be checked immediately before
each connect attempt. Redirects, reconnects, proxies, TLS server names,
broadcast, multicast, custom resolvers, local sockets, and interface inspection
SHALL be checked at their corresponding dynamic boundary.

#### Scenario: Authorized host resolves to a forbidden private address
- **WHEN** an authorized origin resolves or is rebound to an IP outside its effective grant
- **THEN** connection is denied before opening the socket
- **AND** the operation returns a typed `NetworkPermissionError`

#### Scenario: Connect permission is used to bind a listener
- **WHEN** code with outbound origin authority but no matching listen address attempts to bind
- **THEN** the bind is denied before the operating-system socket begins listening

### Requirement: HTTP authority is revalidated across protocol transitions
HTTP clients SHALL validate every redirect, DNS result, reconnect, proxy hop,
TLS server name, and WebSocket destination against the effective outbound
grant. HTTP servers SHALL validate their bind address against inbound authority.
Credentials, cookies, authorization fields, and pool entries SHALL NOT cross an
origin, proxy, or TLS identity boundary unless the applicable protocol policy
and permission explicitly allow it.

#### Scenario: Redirect changes origin
- **WHEN** an authorized request receives a redirect to another origin
- **THEN** sensitive headers and credentials are removed as required
- **AND** the new origin, resolved addresses, and TLS identity are checked before connecting

### Requirement: Environment access is typed and secret-safe
The standard library SHALL provide `Environment` and exact preferred alias
`Env` with static getters, nullable/default/required access, compile-time
constrained typed parsing, secret access, containment, and name listing. Every
operation SHALL preserve permission failure through `Result`; unauthorized
lookup SHALL NOT disclose whether a name exists. Broad listing SHALL require
broad authority and SHALL return names without values. Secret reads SHALL
return `SecretString`, which SHALL have no general string conversion and SHALL
remain redacted unless passed directly to an authorized sink boundary.

#### Scenario: Unapproved environment variable
- **WHEN** `Env.get("DATABASE_URL")` executes without a matching runtime grant
- **THEN** it returns `Error(PermissionDeniedError)` and does not reveal the variable

#### Scenario: Typed variable has invalid content
- **WHEN** an authorized present variable is read through `Env.get<Int>` and is not a valid `Int`
- **THEN** the result is `Error(EnvironmentParseError)` distinct from absence and denial

### Requirement: Fingerprinting information is not ambient authority
The runtime MUST keep detailed CPU and memory identity, user and host names,
device serials, stable machine identifiers, and hardware inventories out of
freely readable `System` or `Platform` properties. Any API that needs such
information MUST require a narrow permission and return typed denial.

#### Scenario: Library attempts to identify the machine
- **WHEN** library code requests a stable machine identifier without an effective grant
- **THEN** the operation is denied without returning a substitute fingerprint

### Requirement: Permission approval is inspectable and revocable
The CLI SHALL provide show, diff, approve, revoke, and history operations. History SHALL identify project name/location, requester, scope, phase, approver, and timestamp without storing or displaying secrets. Revocation SHALL take effect before the next privileged execution.

#### Scenario: Approval is revoked
- **WHEN** the developer revokes a project's permission fingerprint
- **THEN** the next authority-bearing command requires fresh approval before executing privileged code
