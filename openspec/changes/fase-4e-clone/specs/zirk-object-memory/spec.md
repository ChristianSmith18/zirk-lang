## ADDED Requirements

### Requirement: Deep-clone traversal state is collector-safe for its whole duration

A `clone()` call's memoization table (mapping each already-cloned source address to its new clone's address) SHALL be scoped to one top-level `clone()` invocation, and every partially-built clone allocation reachable from that call SHALL remain reachable to the collector for the call's entire duration, so a collection triggered by one of the call's own allocations cannot reclaim a partially-built clone or the objects it already points to.

#### Scenario: Collection triggered mid-clone
- **WHEN** allocating a node during a deep `clone()` call crosses the collector's threshold and triggers a collection
- **THEN** every clone allocation produced so far by that call remains reachable and is not collected
