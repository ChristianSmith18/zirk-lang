## ADDED Requirements

### Requirement: Callable values support polymorphic captures
The language SHALL allow the same `Fn(P...) => R` binding to hold differently-captured closures at different times without changing the static type.

#### Scenario: Two captured closures share a local
- **WHEN** `mut f: Fn() => Int32` is first assigned a closure capturing `a`, then a closure capturing `b`
- **THEN** both assignments type-check and calls dispatch to the correct closure at runtime

#### Scenario: Captured closure passed through a variable
- **WHEN** a captured closure is stored in a parameter, field, or local and later called
- **THEN** the call uses the stored closure's capture block

### Requirement: Escaping captured closures are boxed
The language SHALL lower captured closures that escape or are stored to a two-word `{function pointer, capture-block pointer}` representation, where the capture block is heap-allocated and GC-tracked.

#### Scenario: Returned closure keeps environment
- **WHEN** a function returns `Fn() => Int32` that captures a mutable counter
- **THEN** the returned callable owns a boxed capture block and repeated calls share the same counter

#### Scenario: Closure stored in a field
- **WHEN** a captured closure is stored in a `class` or `record` field
- **THEN** the capture block is heap-allocated and the field holds the two-word representation

### Requirement: Cloned callables produce independent environments
The language SHALL support `.clone()` on a callable when all captures are `Clone`, producing an independent deep copy of the capture block.

#### Scenario: Cloned counter is independent
- **WHEN** a callable is cloned and the original's captured counter is mutated
- **THEN** the clone's counter is unchanged

#### Scenario: Non-cloneable capture is rejected
- **WHEN** a callable captures a non-`Clone` resource and `.clone()` is called
- **THEN** compilation fails
