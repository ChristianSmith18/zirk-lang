## ADDED Requirements

### Requirement: Runtime exposes overflow/cast error helpers
The runtime SHALL provide `zirk_rt_throw_overflow` and `zirk_rt_throw_invalid_cast` helpers callable from generated code when a native safety check fails.

#### Scenario: Overflow check in generated code
- **WHEN** the generated code detects an integer overflow
- **THEN** it calls `zirk_rt_throw_overflow` and the function returns a `RuntimeError` object

#### Scenario: Invalid cast in generated code
- **WHEN** the generated code detects an invalid `as` cast
- **THEN** it calls `zirk_rt_throw_invalid_cast`

### Requirement: Runtime exposes callable box allocation
The runtime SHALL provide `zirk_rt_alloc_callable` and `zirk_rt_clone_callable` for the boxed closure representation, allocating and populating the capture block.

#### Scenario: Closure returned
- **WHEN** a function returns a captured closure
- **THEN** the runtime allocates a callable box and copies the captures into the capture block

### Requirement: Runtime exposes pinning helpers
The runtime SHALL provide `zirk_rt_pin_object` and `zirk_rt_unpin_object` to add or remove an object from the per-thread pin list, and pinned objects SHALL be ignored during collection compaction.

#### Scenario: Unsafe block pins object
- **WHEN** an `unsafe` block exposes an interior pointer
- **THEN** the runtime calls `zirk_rt_pin_object` for the base object and `zirk_rt_unpin_object` on block exit

### Requirement: Runtime exposes dependent-ref helpers
The runtime SHALL provide `zirk_rt_dependent_base` to read the base pointer from a `Dependent<T>` and the GC SHALL trace the base object through the dependent.

#### Scenario: GC marks dependent
- **WHEN** the GC reaches a `Dependent<T>` value
- **THEN** it marks the base object as strongly reachable via `zirk_rt_dependent_base`
