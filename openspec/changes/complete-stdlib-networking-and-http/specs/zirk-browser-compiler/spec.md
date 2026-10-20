## MODIFIED Requirements

### Requirement: Browser execution grants no ambient authority

The browser package SHALL neither request nor expose ambient filesystem, network, environment, secrets, process, shell, native library, clock, random, or DOM capabilities. User source SHALL be processed only in caller-provided memory and SHALL not be persisted or transmitted by the package. An embedding host MAY explicitly provide a per-execution browser-network capability with an origin allowlist; the compiler package SHALL pass it only to that execution and SHALL NOT persist it, widen it, or create a network request by itself.

#### Scenario: Compile source locally
- **WHEN** a caller checks or runs source without providing a network capability
- **THEN** the package performs no HTTP request, storage write, or process invocation

#### Scenario: Run with explicitly injected network authority
- **WHEN** an embedding host provides a bounded browser-network capability for one execution
- **THEN** only that execution may issue HTTP requests to its allowed origins through the browser transport
- **AND** the package does not gain filesystem, raw socket, server, environment, process, or DOM access

#### Scenario: Source requests an unavailable effect
- **WHEN** compilation produces a module requiring an unavailable host capability
- **THEN** the result contains a capability diagnostic
- **AND** the package does not simulate a successful effect

#### Scenario: A later execution follows an authorized execution
- **WHEN** another browser request runs after an execution that received network authority
- **THEN** it has no network authority unless the embedding host supplies a separate capability for that request
