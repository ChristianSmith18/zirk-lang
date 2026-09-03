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

### Requirement: Dependent references are supported
The language SHALL define `Dependent<T>` as a reference whose lifetime is tied to the allocation that contains the value it refers to.

#### Scenario: Dependent field outlives base
- **WHEN** a function returns a `Dependent<T>` that refers to a field of a local object
- **THEN** compilation fails with an escaping-dependent diagnostic

#### Scenario: Dependent is stored without keeping base
- **WHEN** a `Dependent<T>` is stored in a location that does not also keep its base alive
- **THEN** compilation fails with a lifetime diagnostic

### Requirement: Dependent references keep bases alive
A `Dependent<T>` value SHALL keep its base allocation alive as long as the dependent itself is reachable, and the GC SHALL trace the base object through the dependent.

#### Scenario: Dependent is stored in a long-lived container
- **WHEN** a `Dependent<T>` is stored in a global or heap object
- **THEN** the base object is also reachable from the GC roots through the dependent

### Requirement: Bounded native pinning is automatic
The language SHALL introduce `Pin<T>` to keep an object's address stable for native interop, and the compiler SHALL infer a `Pin<T>` when an interior address is exposed to native code.

#### Scenario: Interior pointer needs pin
- **WHEN** `Pointer.from(o.field)` is used inside an `unsafe` block
- **THEN** the compiler inserts a `Pin<typeof(o)>` for the duration of the block

#### Scenario: Explicit pin is accepted
- **WHEN** a binding is annotated `p: Pin<MyClass>` and passed to an `extern "C"` function
- **THEN** the object is pinned for the lifetime of the `Pin<T>` value

### Requirement: Pin is released at block exit
A `Pin<T>` SHALL be released when the enclosing `unsafe`/`commit` block exits normally, by exception, or by `return`/`break`/`continue`, and the release SHALL occur after any journal rollback and before the target jump.

#### Scenario: Pin after unsafe block
- **WHEN** an `unsafe` block containing `Pointer.from(o.field)` returns
- **THEN** the object is unpinned after the journal is rolled back and before the return value leaves the frame

