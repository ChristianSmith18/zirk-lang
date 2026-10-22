## ADDED Requirements

### Requirement: New network transports use distinct least-authority scopes

Permission manifests SHALL distinguish UDP destination authority, UDP bind authority, local-endpoint connect authority, local-endpoint listen authority, and interface inspection authority. A UDP destination grant SHALL identify the normalized address and port or an explicit bounded destination set; UDP bind SHALL use `network.udp_listen.addresses`, separate from TCP/HTTP `network.listen.addresses`. Local endpoint grants SHALL identify canonical endpoint paths or platform endpoint names and SHALL keep connect and listen distinct. Interface inspection SHALL require `network.inspect.interfaces`. None of these grants SHALL imply TCP, HTTP, broadcast, multicast, route selection, or another transport's authority. Libraries SHALL declare requirements and applications SHALL grant them under the existing phase and consent rules.

#### Scenario: UDP destination is outside its grant
- **WHEN** code sends a datagram to an address or port outside its effective UDP destination scope
- **THEN** the runtime denies the send before the operating-system side effect

#### Scenario: UDP bind uses its own inbound authority
- **WHEN** code binds a UDP socket without matching `network.udp_listen.addresses` authority
- **THEN** the runtime denies the bind before creating an externally reachable socket

#### Scenario: Local connect does not authorize local listen
- **WHEN** code has permission for one local endpoint connection but attempts to listen on that endpoint
- **THEN** the runtime denies the bind before creating or replacing an endpoint

#### Scenario: Interface inspection is explicitly granted
- **WHEN** code requests interface metadata without `network.inspect.interfaces`
- **THEN** the runtime returns typed permission denial without revealing interface records

#### Scenario: Permission phases remain enforced
- **WHEN** a library declares one of these scopes for build or runtime
- **THEN** the existing approval fingerprint, requester path, and selected phase rules apply without broadening any other grant
