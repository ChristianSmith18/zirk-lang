## ADDED Requirements

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
