## ADDED Requirements

### Requirement: The IR allocates through an abstract operation

The IR SHALL express object creation without naming a memory strategy, and the runtime SHALL materialize it behind the C ABI boundary.

This is the direct application of `docs/decisions/ADR-003-memoria.md` and `docs/decisions/ADR-002-runtime-staticlib.md`: the strategy is chosen in Phase 4, and the runtime is the single point where it is materialized.

#### Scenario: Object construction
- **WHEN** an instance's construction is lowered
- **THEN** the IR uses the abstract allocation operation naming the type
- **AND** it does NOT name `malloc`, reference counting, or garbage collection

#### Scenario: The runtime exposes allocation with no mangling
- **WHEN** the static library's symbols are inspected
- **THEN** the allocation function appears under its `extern "C"` name

### Requirement: Memory is not freed in this phase

The runtime SHALL NOT free the memory of the objects it allocates.

This is not an oversight: freeing requires having decided **when**, and that is exactly the question ADR-003 leaves open until Phase 4. Building a partial deallocation now would be work that later has to be undone, with the trap that a half-built reference count seems to work until the first cycle.

#### Scenario: A program ends without freeing
- **WHEN** a program constructs objects and terminates
- **THEN** the process terminates normally
- **AND** the operating system reclaims the memory

#### Scenario: The debt is declared
- **WHEN** the runtime's documentation is consulted
- **THEN** it states that the memory strategy arrives in Phase 4

### Requirement: Object header

Every object with identity SHALL carry a header that identifies its type at runtime, preceding its fields.

Dynamic dispatch, checkable casts, and the basic type identity that `ZIRK_LANGUAGE_SPEC.md` section 12 always guarantees all need it.

#### Scenario: Type identity available
- **WHEN** an object is inspected at runtime
- **THEN** its header identifies its type

#### Scenario: The header precedes the fields
- **WHEN** a field's offset is computed
- **THEN** it is counted starting after the header

### Requirement: Inherited fields precede a type's own

A class's layout SHALL place inherited fields before its own, in the order the hierarchy declares them.

This way a subclass's layout prefix matches its superclass's, and an inherited field's offset does not depend on the type it is viewed through.

#### Scenario: Same offset in base and subclass
- **WHEN** a subclass adds fields to its base's
- **THEN** a base field occupies the same offset in both

#### Scenario: Access through the base type
- **WHEN** a variable of the base type holds an instance of the subclass and an inherited field is read
- **THEN** the correct value is read with no runtime check
