# zirk-environment Specification

## Purpose

Defines permission-checked, read-only access to the process environment, with typed parsing, explicit absence and platform failures, and secret values that are redacted by default.

## Requirements

### Requirement: Environment access is available as a bundled Zirk package

`std.environment` SHALL publish `Environment` and its exact preferred alias `Env` with `get`, `get_or_null`, `get_or`, `require`, `contains`, `list_names`, and `get_secret`. The API SHALL read the current process environment and SHALL NOT mutate it.

#### Scenario: Import and read a configured value
- **WHEN** an application imports `Env` and reads an authorized environment variable
- **THEN** the bundled API returns its value through the documented typed result

#### Scenario: Attempt to mutate the host environment
- **WHEN** Zirk code attempts to change or remove a process environment variable through `std.environment`
- **THEN** no mutating API is available

### Requirement: Environment operations require exact effective permission

Every environment read, containment query, name listing, and secret read SHALL validate the application's effective runtime grant before inspecting host state. Libraries SHALL declare requirements but SHALL NOT grant authority. The application approval flow SHALL collect requirements from the application's complete locked dependency graph and request explicit consent for their union with application-declared scopes. Dependency manifests and lockfiles SHALL be included in the approval fingerprint so changing a transitive requirement invalidates prior consent. Unauthorized lookup SHALL NOT reveal whether a name exists, and permission failure SHALL remain an `Error` even for fallback and nullable operations.

#### Scenario: A variable is not authorized
- **WHEN** code calls `Env.get`, `Env.contains`, `Env.get_or`, or `Env.get_or_null` without matching authority
- **THEN** the operation returns a typed permission error regardless of whether the variable exists

#### Scenario: A fallback is requested without authority
- **WHEN** code calls `Env.get_or(name, fallback)` without permission for `name`
- **THEN** the operation returns permission denial and does not return the fallback as if access were allowed

### Requirement: Absence and typed parsing remain distinct

`get` and `require` SHALL distinguish an authorized but absent value from permission, platform, and decoding failures. `get_or_null` and `get_or` SHALL apply nullable/fallback behavior only to authorized absence. Typed getters SHALL use an explicit compile-time-constrained parsing contract and return a typed parse error for malformed values; they SHALL NOT use reflection or guess constructors.

#### Scenario: Authorized value is absent
- **WHEN** an authorized name is absent
- **THEN** `get` and `require` return their documented missing-value errors, `get_or_null` returns `Ok(null)`, and `get_or` returns `Ok(fallback)`

#### Scenario: Typed value has invalid content
- **WHEN** an authorized present value cannot be parsed as the requested type
- **THEN** the operation returns `EnvironmentError.Parse`, distinct from missing value and permission denial

#### Scenario: Parsing uses a static contract
- **WHEN** `Env.get<T>(name)` is checked
- **THEN** `T` is constrained by `EnvironmentParsable<T>` with static `parse(String): Result<T, String>`, with `String` as the default type
- **AND** built-in parser intrinsics and user-defined static implementations are resolved at compile time without reflection

### Requirement: Name listing is bounded by permission scope

`list_names` SHALL return names only, filtered to the effective environment grant, in deterministic lexicographic order. Unrestricted discovery SHALL require explicit broad authority and normal developer approval. The API SHALL NOT return values as a side effect of listing names.

#### Scenario: A caller has a narrow grant
- **WHEN** the caller lists environment names under a narrow grant
- **THEN** only authorized names are returned and no values are exposed

### Requirement: Secret values are non-printable by default

`get_secret` SHALL require secret-specific authority and return `SecretString`. Secret values SHALL NOT have general string conversion, interpolation, ordinary formatting, or unredacted output through any implemented diagnostic sink. Reveal SHALL occur only at an explicitly authorized sink boundary. Any future debugger, heap snapshot, test runner, or test reporter that can expose `SecretString` values SHALL redact them before presentation or persistence.

#### Scenario: A secret is rendered in an implemented diagnostic sink
- **WHEN** a `SecretString` is included in supported formatting, interpolation, an exception, or structured runtime diagnostic output
- **THEN** only the redacted marker is observable

#### Scenario: A future debugger or test reporter exposes a secret
- **WHEN** a debugger, heap snapshot, test runner, or test reporter is implemented and can display or persist a `SecretString`
- **THEN** it redacts the value before presentation or persistence

#### Scenario: A secret is passed to an authorized sink
- **WHEN** code passes a secret to a sink that explicitly accepts secret values and has matching authority
- **THEN** the sink receives the value without exposing it as an ordinary `String`

#### Scenario: A secret is sent through the HTTP credential sink
- **WHEN** code passes a `SecretString` to `HTTP.request_with_secret_header`
- **THEN** the request uses HTTPS and the exact approved origin grant
- **AND** the secret is sent only as the explicitly named request header
- **AND** redirects are not followed and the value is not returned to Zirk as a `String`

#### Scenario: A secret authenticates an explicitly configured proxy
- **WHEN** code passes a `SecretString` to `HTTPProxy.create_authenticated`
- **AND** configures the proxy on an `HTTPClient`
- **THEN** the proxy and destination grants are checked independently
- **AND** remote cleartext HTTP proxies reject credentials before networking
- **AND** HTTPS proxy credentials are sent only after validating the proxy TLS identity
- **AND** proxy authorization is never forwarded through the destination tunnel
- **AND** the secret is not returned to Zirk as an ordinary `String`

### Requirement: Environment encoding and platform failures are explicit

Environment name comparison SHALL follow host semantics for both permission
scope checks and process lookups: exact, case-sensitive comparison on Unix and
ordinal, case-insensitive comparison on Windows. Scope checks and host lookup
MUST use the same comparison rule.

The implementation SHALL convert host environment values to Zirk strings only when their encoding is valid under the documented platform rules. Invalid encoding, host API failure, and name validation failure SHALL produce typed errors and SHALL NOT be mistaken for absence. Observable name and value semantics SHALL be consistent across the supported native target set.

#### Scenario: Host value cannot be decoded
- **WHEN** an authorized host value cannot be represented as valid Zirk text
- **THEN** the read returns a typed decoding error rather than replacement text or `null`
