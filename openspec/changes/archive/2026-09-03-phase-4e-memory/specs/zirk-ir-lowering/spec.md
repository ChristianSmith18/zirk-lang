## ADDED Requirements

### Requirement: IR supports dependent references and pinning
The language SHALL lower `Dependent<T>` construction to `DependentFrom` and `Pin<T>` construction to `PinObject`.

#### Scenario: Pin interior pointer
- **WHEN** `Pointer.from(o.field)` is used
- **THEN** the IR contains `PinObject(o)` before `PointerFromField`

#### Scenario: Dependent reference
- **WHEN** a `Dependent<T>` is created from a field
- **THEN** the IR contains `DependentFrom` with both the field pointer and the base object

### Requirement: IR supports pin release on all exits
The language SHALL lower an `UnpinObject` instruction on every exit path from the scope that owns the pin.

#### Scenario: Unsafe block returns normally
- **WHEN** an `unsafe` block containing a pinned object returns
- **THEN** the IR has `UnpinObject` before the return

#### Scenario: Unsafe block throws
- **WHEN** an `unsafe` block containing a pinned object throws
- **THEN** the IR unrolls the `UnpinObject` after journal rollback and before rethrow
