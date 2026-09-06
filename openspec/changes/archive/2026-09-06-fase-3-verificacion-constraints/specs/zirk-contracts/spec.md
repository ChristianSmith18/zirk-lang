# Delta spec: zirk-contracts

## MODIFIED Requirements

### Requirement: Explicit capability derivation
Classes SHALL NOT derive equality, hashing, or cloning silently. Records, tuples, and enums MAY request explicit derivation only when every component satisfies the required capability; derived cloning SHALL be deep.

#### Scenario: Uncloneable component
- **WHEN** Clone derivation is requested for a value containing an uncloneable resource
- **THEN** compilation fails and identifies the component and missing capability
