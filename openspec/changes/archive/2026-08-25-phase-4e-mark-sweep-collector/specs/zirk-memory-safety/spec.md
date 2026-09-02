## MODIFIED Requirements

### Requirement: Strategy-neutral automatic memory
Zirk SHALL reclaim unreachable managed memory including cycles without exposing GC, ownership, regions, moves, or reference counting as mandatory source semantics, and representation changes MUST preserve identity and observable lifetime.

#### Scenario: Runtime moves an object
- **WHEN** the runtime relocates a live managed object
- **THEN** safe references remain valid and `is` observes the same identity

### Requirement: Deterministic cleanup belongs to resources
Managed objects MUST NOT expose general-purpose finalizers whose timing is observable, and deterministic cleanup SHALL use `Resource<E>` and `match with`.

#### Scenario: Managed object becomes unreachable
- **WHEN** an ordinary managed object becomes unreachable
- **THEN** the program cannot depend on a destructor running at that moment
