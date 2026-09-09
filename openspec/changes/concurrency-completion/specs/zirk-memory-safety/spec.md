## MODIFIED Requirements

### Requirement: Closed unsafe operation set

Raw pointer creation, dereference, arithmetic and representation casts; unsafe native calls; unchecked native construction; untagged native-union access; weak atomic ordering (`AtomicOrder.relaxed` / `.acquire` / `.release`); and manual safety-contract implementation SHALL require an explicit unsafe boundary. An `Atomic<T>` operation outside `unsafe` SHALL use sequentially consistent ordering.

#### Scenario: Safe code dereferences a pointer

- **WHEN** code dereferences `Pointer<T>` outside an unsafe block
- **THEN** compilation fails and identifies the required unsafe operation

#### Scenario: Weak atomic ordering outside unsafe

- **WHEN** an atomic operation requests `AtomicOrder.relaxed` outside an unsafe block
- **THEN** compilation fails and identifies the required unsafe operation
