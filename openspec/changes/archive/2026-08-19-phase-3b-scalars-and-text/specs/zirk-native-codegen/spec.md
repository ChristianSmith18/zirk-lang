## ADDED Requirements

### Requirement: Codegen for each integer width over the native LLVM type

The backend SHALL emit each IR integer type as the LLVM `IntType` of the corresponding width, with the arithmetic operations and overflow intrinsics (`*.with.overflow`) matching its signedness.

#### Scenario: Signed checked addition
- **WHEN** code for an addition on `Int64` is emitted
- **THEN** that width's signed overflow intrinsic is used

#### Scenario: Unsigned checked addition
- **WHEN** code for an addition on `UInt64` is emitted
- **THEN** that width's unsigned overflow intrinsic is used

### Requirement: Codegen for `Float` over the native LLVM type

The backend SHALL emit each IR `Float` type as the LLVM `FloatType` of the corresponding width, with the `NaN`/invalid-domain check emitted explicitly around the native operation, not delegated to the backend's floating-point semantics.

#### Scenario: Explicit check before the result
- **WHEN** code is emitted for a `Float` operation the IR flagged as potentially indeterminate
- **THEN** the generated code checks the condition before the result is used, and calls the runtime for controlled failure if it holds

### Requirement: Codegen for `to_string()` dispatch

The backend SHALL emit, at every site where `println`/`print`/an interpolation needs to convert a value to text, a call to the `to_string()` method resolved for that value's static type — direct if it is not overridable, through the object's or the contract's table if it is, with the same mechanism as any other method call (ADR-013).

#### Scenario: Converting a native type
- **WHEN** code to print an `Int32` is emitted
- **THEN** the call to `to_string()` resolves to `Int32`'s native `to_string()`, with no indirection

#### Scenario: Converting an overridable user type
- **WHEN** code to print a value of a type whose `to_string()` can be overridden by a subclass is emitted
- **THEN** the call goes through the same table as any other virtual method of the object
