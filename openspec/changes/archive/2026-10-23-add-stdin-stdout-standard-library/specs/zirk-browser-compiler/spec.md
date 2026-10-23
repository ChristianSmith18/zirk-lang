## ADDED Requirements

### Requirement: Browser stream capability boundary is explicit

The browser compiler SHALL represent `std.io` output as its existing bounded
output events and SHALL identify standard input reads as unavailable browser
effects before execution. It SHALL not acquire DOM, prompt, storage, network,
or other ambient input authority to emulate stdin.

#### Scenario: Captured output remains bounded
- **WHEN** browser execution writes through stdout or stderr within output limits
- **THEN** ordered events are returned through the versioned response contract

#### Scenario: Stdin is unavailable
- **WHEN** checked source has a reachable stdin read
- **THEN** browser execution reports a capability failure without invoking an ambient API
