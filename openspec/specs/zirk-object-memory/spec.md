# zirk-object-memory Specification

## Purpose
Defines strategy-neutral object allocation, runtime type headers, inherited
layout, value layouts, and object-memory ABI boundaries.
## Requirements
### Requirement: The IR allocates through an abstract operation

The IR SHALL express the creation of an object without naming a memory strategy, and the runtime SHALL materialize it beyond the C ABI boundary.

This is the direct application of `docs/decisions/ADR-003-memoria.md` and `docs/decisions/ADR-002-runtime-staticlib.md`: the strategy is chosen in Phase 4, and the runtime is the single point where it is materialized.

#### Scenario: Construction of an object
- **WHEN** the construction of an instance is lowered
- **THEN** the IR uses the abstract allocation operation naming the type
- **AND** it does NOT name `malloc`, reference counting, or garbage collection

#### Scenario: The runtime exposes allocation without mangling
- **WHEN** the symbols of the static library are inspected
- **THEN** the allocation function appears with its `extern "C"` name

### Requirement: Object header

Every object with identity SHALL carry a header identifying its type at runtime, before its fields.

Dynamic dispatch, checkable casts, and the basic type identity that `ZIRK_LANGUAGE_SPEC.md` section 12 always guarantees all need it.

#### Scenario: Type identity available
- **WHEN** an object is inspected at runtime
- **THEN** its header identifies its type

#### Scenario: The header precedes the fields
- **WHEN** the offset of a field is computed
- **THEN** it is counted starting after the header

### Requirement: Inherited fields precede the class's own fields

A class's layout SHALL place inherited fields before its own, in the order in which the hierarchy declares them.

This way the prefix of a subclass's layout matches that of its superclass, and the offset of an inherited field does not depend on the type through which it is viewed.

#### Scenario: Same offset in base and subclass
- **WHEN** a subclass adds fields to those of its base
- **THEN** a field of the base occupies the same offset in both

#### Scenario: Access through the base type
- **WHEN** a variable of the base type holds an instance of the subclass and an inherited field is read
- **THEN** the correct value is read without a runtime check

### Requirement: Deep-clone traversal state is collector-safe for its whole duration

A `clone()` call's memoization table (mapping each already-cloned source address to its new clone's address) SHALL be scoped to one top-level `clone()` invocation, and every partially-built clone allocation reachable from that call SHALL remain reachable to the collector for the call's entire duration, so a collection triggered by one of the call's own allocations cannot reclaim a partially-built clone or the objects it already points to.

#### Scenario: Collection triggered mid-clone
- **WHEN** allocating a node during a deep `clone()` call crosses the collector's threshold and triggers a collection
- **THEN** every clone allocation produced so far by that call remains reachable and is not collected
