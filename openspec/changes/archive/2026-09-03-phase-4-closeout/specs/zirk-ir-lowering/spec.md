## ADDED Requirements

### Requirement: IR supports catchable overflow and invalid cast
The language SHALL lower catchable overflow and invalid-cast failures to a `ThrowNative` instruction carrying a `RuntimeError` subclass.

#### Scenario: Overflow in addition
- **WHEN** `x + y` is an integer addition that may overflow
- **THEN** the IR contains an overflow check followed by `ThrowNative(ArithmeticOverflowError)` on failure

#### Scenario: Invalid cast
- **WHEN** `value as T` requires a runtime cast check
- **THEN** the IR contains a cast check followed by `ThrowNative(InvalidCastError)` on failure

### Requirement: IR supports grouped resource cleanup
The language SHALL lower grouped `match with` to a sequence of acquisition, body, and right-to-left close instructions with correct failure branches.

#### Scenario: Two resources
- **WHEN** `match a with ..., b with ...` is lowered
- **THEN** the IR has `Acquire a; OnFailure (Close a)`, `Acquire b; OnFailure (Close b; Close a)`, `Close b`, `Close a`

#### Scenario: Close failure composition
- **WHEN** a grouped `match with` has both a body error and a close error
- **THEN** the IR builds a `ResourceFailure` value in the error merge block

### Requirement: IR supports boxed callables
The language SHALL lower escaping captured closures to a `MakeCallable` instruction and calls to a `CallCallable` instruction.

#### Scenario: Store closure in field
- **WHEN** a captured closure is stored in a field
- **THEN** the IR contains `MakeCallable` for the value and `CallCallable` for the call site

#### Scenario: Call through variable
- **WHEN** a `Fn(...)` local is called
- **THEN** the IR loads the two-word representation and emits `CallCallable`

### Requirement: IR supports dependent references and pinning
The language SHALL lower `Dependent<T>` construction to `DependentFrom` and `Pin<T>` construction to `PinObject`.

#### Scenario: Pin interior pointer
- **WHEN** `Pointer.from(o.field)` is used
- **THEN** the IR contains `PinObject(o)` before `PointerFromField`

#### Scenario: Dependent reference
- **WHEN** a `Dependent<T>` is created from a field
- **THEN** the IR contains `DependentFrom` with both the field pointer and the base object
