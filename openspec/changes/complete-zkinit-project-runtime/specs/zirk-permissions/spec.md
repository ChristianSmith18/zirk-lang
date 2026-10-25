## REMOVED Requirements

### Requirement: Libraries request and applications grant scoped authority
**Reason**: A separate application `permissions` block and build/runtime phase fields conflict with the agreed single finite `requires` model.
**Migration**: Declare capability needs through `requires` in `.zkinit`; consent is checked and recorded by the runtime for concrete resources.

### Requirement: New network transports use distinct least-authority scopes
**Reason**: The requirement encodes permission names and phase rules that are removed by the unified capability vocabulary.
**Migration**: Express UDP destination, UDP listen, local endpoint connect/listen, and interface inspection as distinct operations in `requires`, without phase fields.

### Requirement: Permission approval is inspectable and revocable
**Reason**: Inspection is replaced by the complete dependency requirement tree and exact-resource consent history, which no longer records build/runtime phases.
**Migration**: Use permission inspection to review each requester and path; revoke the saved exact resource consent.

### Requirement: Interactive approval is precise and fail-closed
**Reason**: The former preflight approval flow offers phases, grant sets, and broad scopes instead of asking yes/no at the concrete operation.
**Migration**: Ask once at the protected operation for the exact resource, persist only an explicit yes, and deny only that operation after a no.

## MODIFIED Requirements

### Requirement: Permission effects propagate automatically

The compiler SHALL infer capability needs from privileged APIs through functions, methods, lambdas, closures, generators, and higher-order callables. Effects SHALL remain in compiler and callable metadata and call-chain diagnostics without adding permission syntax to `Fn`. Each application and package SHALL use the same finite `requires` declaration model; the consuming application's effective requirement tree SHALL retain every requesting package and dependency path. No `permissions`, `compile_permissions`, `during`, or build/test/runtime permission subsection SHALL be required.

#### Scenario: File-reading callable is stored
- **WHEN** a file-reading function is assigned to a compatible `Fn` and invoked indirectly
- **THEN** filesystem-read capability remains required and traceable through the indirect call

#### Scenario: Transitive requirement retains provenance
- **WHEN** package `lib-b` requires network listening through `lib-a`
- **THEN** the application's internal requirement tree identifies `lib-b`, `lib-a`, and the complete dependency path

### Requirement: Declaration and developer consent are separate

Capability requirements SHALL describe what code may attempt and SHALL NOT by themselves count as user consent. A package SHALL NOT grant authority to itself or another package. Zirk SHALL persist explicit consent in a user-owned approval store outside the repository and bind it to project name, canonical project location, user/device identity, exact resource scopes, locked requester versions/integrity, and transitive requirement paths. A first-use approval MAY update the application's single `requires` block using the manifest parser and formatter and SHALL ensure that a project `.zkinit` exists, but a manifest edit without a matching approval record SHALL grant no authority. Moving or renaming the project, widening a scope, changing a requester, or changing the dependency path SHALL require fresh approval for the affected scope.

#### Scenario: Library declares a capability without consent
- **WHEN** a dependency adds a capability requirement to its manifest
- **THEN** the requirement appears in the application's provenance tree but is not treated as user consent

#### Scenario: First-use approval creates project configuration
- **WHEN** a user approves a concrete capability for a standalone entry that has no `.zkinit`
- **THEN** Zirk creates a minimal application manifest with a stable local project name, version `0.1.0`, the selected entry path, and source root `.`, then stores the explicit approval bound to that project

#### Scenario: Library edits .zkinit
- **WHEN** repository contents add a broad capability requirement without a matching user approval
- **THEN** the requirement is visible in the dependency tree but does not grant authority

#### Scenario: Project moves
- **WHEN** an unchanged project is executed from a different canonical location
- **THEN** its previous approval is not reused and the first protected operation requires a new decision

#### Scenario: Unapproved manifest edit
- **WHEN** repository contents add or widen a capability requirement without a matching approval record
- **THEN** the new scope remains unauthorized and is checked at the protected operation

### Requirement: Permission validation is secure and incremental

Zirk SHALL compare project identity, canonical manifest, lockfile, requester-subgraph, and approval fingerprints on authority-bearing commands. Unchanged inputs SHALL use a cached fast path; changed inputs SHALL recompute affected graph segments. Missing, corrupt, or unverifiable approval data SHALL grant nothing and SHALL require a fresh runtime decision or a non-interactive denial when a protected operation is reached. Missing runtime consent SHALL NOT block compilation or startup solely because a requirement is declared.

#### Scenario: No permission-relevant input changed
- **WHEN** a project repeats an authority-bearing command with matching project, manifest, lockfile, requester, and consent fingerprints
- **THEN** Zirk reuses the cached decision without a full dependency rescan or another prompt

#### Scenario: Dependency requester updated
- **WHEN** an approved dependency changes version or integrity
- **THEN** its old approval fingerprint grants nothing for the changed requester and the next protected operation requires a fresh decision

### Requirement: Privileged APIs enforce normalized operation scopes

Filesystem access SHALL validate canonical paths and symlink escape; network access SHALL validate protocol, host, port, redirects, DNS results, and reconnects; environment access SHALL distinguish secrets; process access SHALL validate canonical executable and allowed argument forms; shell execution SHALL remain a separate high-risk capability. A protected operation SHALL check its exact effective scope immediately before its side effect. Missing or rejected consent SHALL not prevent application startup. Permission denial SHALL remain a typed error through an explicitly preserved `Result`; in ordinary value contexts, the language's existing error projection SHALL yield `null`. The denied operation SHALL not perform its protected side effect, and the runtime SHALL emit a deduplicated warning identifying the attempted capability, resource, and requester without exposing secret values.

#### Scenario: Process argument is outside grant
- **WHEN** code authorized for `git status` attempts `git clean -fd`
- **THEN** execution is denied before the child process starts and the application can handle the resulting error

#### Scenario: Permission is absent in an ordinary value context
- **WHEN** a protected API result is used in an ordinary value context and the exact resource is not approved
- **THEN** the protected operation is denied, the expression projects to `null`, and a warning identifies the attempted resource

#### Scenario: Permission is inspected with `match`
- **WHEN** code directly matches the protected API's `Result` after denial
- **THEN** it receives a typed permission error that can be handled without terminating the process

### Requirement: Network connect and listen authority remain distinct

Outbound authority SHALL use scoped `network.connect` resources and inbound bind authority SHALL use scoped `network.listen` resources from the single `requires` vocabulary. DNS answers SHALL NOT grant authority: every resolved address SHALL be checked immediately before each connect attempt. Redirects, reconnects, proxies, TLS server names, broadcast, multicast, custom resolvers, local sockets, and interface inspection SHALL be checked at their corresponding dynamic boundary. Connecting SHALL NOT imply listening; listening SHALL NOT imply connecting. Dynamic bind requests SHALL be checked against the concrete address and port selected by the application, and a different endpoint SHALL require a separate decision.

#### Scenario: Authorized host resolves to a forbidden private address
- **WHEN** an authorized origin resolves or is rebound to an IP outside its effective scope
- **THEN** connection is denied before opening the socket and returns a typed network permission error

#### Scenario: Connect permission is used to bind a listener
- **WHEN** code has outbound connection consent but no matching listen consent
- **THEN** the bind is denied before the operating-system socket begins listening

#### Scenario: Dynamic port is configured by the application
- **WHEN** a server selects `127.0.0.1:8087` at runtime
- **THEN** the runtime requests or checks that exact endpoint rather than widening consent to every host or port

#### Scenario: Port zero requests an ephemeral bind
- **WHEN** a server requests port `0` on `127.0.0.1`
- **THEN** the runtime checks that exact address and ephemeral-bind request before the side effect, then reports the OS-selected port after binding
- **AND** the approval does not imply permission for unrelated addresses or an explicitly selected port range

## ADDED Requirements

### Requirement: Interactive consent is requested at the protected operation

When a protected operation is reached without a matching saved consent, an interactive CLI SHALL ask at that operation boundary and show the application identity, exact capability and concrete resource, requesting package, dependency path, and operation being attempted. The initial prompt SHALL offer a clear yes/no decision. Approval SHALL persist the exact scope and allow the current operation; rejection SHALL deny only that operation and SHALL NOT edit `.zkinit` or the approval store. A process without an interactive terminal SHALL deny the operation and emit a warning without hanging or assuming consent. No permission phase selection or broad `all` choice SHALL be inferred from a yes/no response.

#### Scenario: User approves a dynamic listener
- **WHEN** an HTTP library attempts to listen on `127.0.0.1:8087` without saved consent
- **THEN** the prompt identifies that endpoint and the dependency that requested it
- **AND** a yes response stores consent for that exact endpoint and permits the bind

#### Scenario: User approves an ephemeral listener
- **WHEN** a server requests an OS-assigned port on `127.0.0.1` without saved consent
- **THEN** the prompt identifies the bind address and that the operating system will select the port
- **AND** a yes response permits that bind request and the runtime reports the actual endpoint after binding

#### Scenario: User rejects a protected operation
- **WHEN** the user answers no to a request for a filesystem path
- **THEN** that operation is denied, no approval is persisted, and unrelated program execution continues

#### Scenario: No terminal is available
- **WHEN** an unapproved operation is reached in a non-interactive process
- **THEN** the operation is denied with a warning and the process does not block waiting for input

### Requirement: One finite capability vocabulary covers project requirements

The manifest SHALL expose one `requires` block for capability declarations and SHALL reject separate permission, compile-permission, environment, or phase-specific blocks. The initial finite capability vocabulary SHALL be exactly `filesystem.read`, `filesystem.write`, `network.connect`, `network.listen`, `network.udp.send`, `network.udp.listen`, `network.local.connect`, `network.local.listen`, `network.inspect_interfaces`, `environment.read`, `environment.list_names`, `secrets.read`, `process.execute`, and `process.shell`. Native/FFI capabilities are outside the initial set until a concrete API defines them. Concrete resource scopes SHALL be supplied and checked at the protected operation and SHALL NOT silently imply another operation or broader resource.

#### Scenario: Manifest uses the unified block
- **WHEN** an application or package declares a supported capability in `requires`
- **THEN** the parser records the capability identifier and source span in the common requirement tree

#### Scenario: Legacy permission block is present
- **WHEN** `.zkinit` contains `permissions`, `compile_permissions`, or `during`
- **THEN** validation reports the removed syntax and directs the user to the unified `requires` form

#### Scenario: Unknown capability is declared
- **WHEN** a manifest uses a capability identifier outside the initial finite vocabulary
- **THEN** validation rejects it and lists the supported identifiers

### Requirement: Missing consent is local to one operation

The runtime SHALL check consent when code reaches a protected resource operation, not as a condition for starting or compiling an application. A denied protected operation SHALL return through the API's ordinary error/result contract, emit a warning that it was attempted and denied, and leave the rest of the application running unless user code chooses to stop. Repeated identical denials within one run SHALL be deduplicated without suppressing distinct resources or requesters.

#### Scenario: Application starts without any saved consent
- **WHEN** an application has no approval records but has not reached a protected operation
- **THEN** it starts and executes ordinary unprotected code

#### Scenario: Repeated denial does not flood the console
- **WHEN** the same package repeatedly attempts the same unapproved resource during one run
- **THEN** the runtime reports a useful warning without printing an unbounded duplicate for every attempt

### Requirement: Permission inspection reports the complete dependency tree

Permission inspection SHALL distinguish direct application requirements from transitive package requirements and SHALL preserve each package identity, version/integrity, concrete scope, and dependency path even when equivalent scopes share one effective approval. Revoking an approval SHALL take effect before the next protected operation.

#### Scenario: Two dependencies request an equivalent scope
- **WHEN** two packages request the same normalized resource scope
- **THEN** the effective decision can be shared while inspection still lists both requesters and dependency paths

#### Scenario: Approval is revoked
- **WHEN** a user revokes an application's saved consent for one resource
- **THEN** the next operation using that resource is checked again and prompts or is denied according to the current execution mode
