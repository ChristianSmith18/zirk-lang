## ADDED Requirements

### Requirement: Algebraic collection payloads remain traced through every mutation boundary

Every admitted tuple, nullable, enum, and union payload stored in a managed collection SHALL expose all transitive managed references to the collector. Insertion, replacement, growth, removal, iteration, snapshots, clone, scope exit, and collection destruction SHALL update roots and release ownership consistently. Supported narrowing and match operations SHALL retain the selected payload while it is in use.

#### Scenario: Force collection after insertion and replacement

- **WHEN** a managed object is reachable only through a nested algebraic value in a supported managed container and collection runs after insertion or replacement
- **THEN** the object remains live while reachable and the replaced payload becomes reclaimable when no other root remains

#### Scenario: Remove and iterate during collection stress

- **WHEN** a supported container is iterated, mutated through its permitted API, and subjected to repeated collection at allocation and cleanup boundaries
- **THEN** no live payload is reclaimed, no stale payload is read, and removed references are eventually released

#### Scenario: Match retains managed payload during unwinding

- **WHEN** a narrowed union payload contains managed references and its branch exits through a catchable error or cleanup path
- **THEN** references remain valid through the branch and cleanup runs exactly once before they become collectible
