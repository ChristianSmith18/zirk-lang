## ADDED Requirements

### Requirement: Runtime exposes pinning helpers
The runtime SHALL provide `zirk_rt_pin_object` and `zirk_rt_unpin_object` to add or remove an object from the per-thread pin list, and pinned objects SHALL be ignored during collection compaction.

#### Scenario: Unsafe block pins object
- **WHEN** an `unsafe` block exposes an interior pointer
- **THEN** the runtime calls `zirk_rt_pin_object` for the base object and `zirk_rt_unpin_object` on block exit

#### Scenario: Pinned object is not compacted
- **WHEN** a GC cycle runs while an object is pinned
- **THEN** the object is not moved by the compactor

### Requirement: Runtime exposes dependent-ref helpers
The runtime SHALL provide `zirk_rt_dependent_base` to read the base pointer from a `Dependent<T>` and the GC SHALL trace the base object through the dependent.

#### Scenario: GC marks dependent
- **WHEN** the GC reaches a `Dependent<T>` value
- **THEN** it marks the base object as strongly reachable via `zirk_rt_dependent_base`

#### Scenario: Dependent base is preserved across collection
- **WHEN** a `Dependent<T>` is the only live reference to its base
- **THEN** the base object survives the GC cycle
