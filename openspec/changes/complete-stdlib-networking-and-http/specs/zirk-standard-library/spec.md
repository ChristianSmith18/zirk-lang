## ADDED Requirements

### Requirement: Standard library source stays inspectable around host bindings

Official `.zrk` package sources SHALL contain the public APIs and all portable library policy, validation, composition, and error mapping that can reasonably execute in Zirk. Compiler-owned host bindings SHALL be narrow operations requiring operating-system access or an audited protocol engine, SHALL be documented with their input validation, permission boundary, limits, cancellation, and resource ownership, and SHALL resolve only by the official package identity. The toolchain SHALL NOT introduce source-level native or ABI annotations for these packages.

#### Scenario: Inspect portable HTTP policy
- **WHEN** a developer opens the bundled `std.http` sources
- **THEN** redirect policy, header/body validation, routing behavior, and typed error mapping are visible as Zirk code
- **AND** transport calls cross only the documented host binding boundary

#### Scenario: Inspect native networking APIs
- **WHEN** a developer opens the bundled `std.net` sources
- **THEN** public types, API signatures, and portable validation are visible as Zirk code
- **AND** operating-system socket and audited TLS/QUIC engine operations are explicit host boundaries

#### Scenario: External package imitates a host binding
- **WHEN** an external package declares the same short method name as a standard host-bound operation
- **THEN** it remains an ordinary Zirk implementation and receives no runtime association

#### Scenario: Review a new host binding
- **WHEN** a standard-library change introduces a new host binding
- **THEN** its official source identity, host necessity, limits, permissions, cancellation, and ownership contract are reviewed and tested

### Requirement: Expanded networking packages publish target-specific capabilities

The bundled `std.net`, `std.http`, and `std.environment` packages SHALL publish the documented second-delivery APIs through their stable logical imports. Native and browser targets SHALL expose only capabilities they can implement securely; unsupported target operations SHALL produce explicit capability errors. The package indexes, API references, examples, and status catalog SHALL identify delivered protocols, required permissions, resource limits, target support, and known exclusions consistently.

#### Scenario: Import a new native transport API
- **WHEN** an application imports an implemented UDP, local IPC, interface, TLS, or HTTP/3 API on a supported native target
- **THEN** it resolves through the bundled package without a separate standard-library installation

#### Scenario: Import a native-only API in a browser target
- **WHEN** browser code references an API that requires a native socket or listener
- **THEN** the target reports an explicit capability diagnostic rather than resolving to an unsafe or fake implementation

#### Scenario: Review the published capability matrix
- **WHEN** a user consults the standard-library index or feature status page
- **THEN** the same protocol, target, permission, and remaining-work status appears in the handbook and companion website
