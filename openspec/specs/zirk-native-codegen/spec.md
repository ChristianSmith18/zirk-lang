# zirk-native-codegen

## Purpose

Defines the translation from the IR into LLVM and the production of the linked executable.

It includes the runtime safety guarantees the spec requires: ordinary overflow is a controlled error, and a division by zero never becomes undefined behaviour.
## Requirements
### Requirement: Translation from IR to LLVM

The backend SHALL translate the typed IR to LLVM IR, preserving type semantics and control flow, including block graphs with cycles.

#### Scenario: Verifiable module
- **WHEN** a well-formed IR is translated
- **THEN** the resulting LLVM module passes LLVM verification

#### Scenario: Block correspondence
- **WHEN** a function with a conditional is translated
- **THEN** the IR's basic blocks correspond to LLVM basic blocks

#### Scenario: Correspondence of a cycle
- **WHEN** a function with a loop is translated
- **THEN** the LLVM block for the loop body jumps back to the condition block
- **AND** the resulting module passes LLVM verification

### Requirement: Controlled arithmetic overflow

Arithmetic operations on integers SHALL detect overflow and produce a controlled runtime error, not silent wrapping.

This is required by `ZIRK_LANGUAGE_SPEC.md` section 3: the wrapping, saturating, or checked variants must be explicit operations, which do not exist in this subset.

#### Scenario: Addition that overflows
- **WHEN** an `Int32` addition exceeds the type's range at runtime
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT produce a wrapped result

#### Scenario: Division by zero
- **WHEN** a division by zero occurs at runtime
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

### Requirement: Linking the executable

The pipeline SHALL produce a native executable by linking the generated object with the Zirk runtime.

#### Scenario: Executable produced
- **WHEN** a valid program of the subset is compiled
- **THEN** an executable for the host platform is produced
- **AND** the executable links the runtime's static library

#### Scenario: Link failure
- **WHEN** the linker fails
- **THEN** a diagnostic that includes the linker output is emitted
- **AND** the help indicates how to verify the toolchain

### Requirement: Lifecycle of the generated program

The generated code SHALL invoke runtime initialization before the body of `main` and its shutdown afterward, per `ZIRK_RUNTIME_SPEC.md` section 2.

#### Scenario: Invocation order
- **WHEN** the generated entrypoint is inspected
- **THEN** it invokes `zirk_rt_init` before the body of `main`
- **AND** it invokes `zirk_rt_shutdown` after `main` returns

#### Scenario: Exit code
- **WHEN** a program of the subset terminates normally
- **THEN** the process terminates with exit code 0

### Requirement: Codegen for closures

The backend SHALL translate a closure into an independent LLVM function that receives the environment as the first implicit argument, plus an aggregate value (function pointer, environment pointer) for use as a first-class value.

#### Scenario: Direct call to a closure
- **WHEN** a call to a closure value is translated
- **THEN** the generated code extracts the function pointer and the environment pointer from the aggregate value
- **AND** it invokes the function with the environment as the first argument

#### Scenario: Closure without captures
- **WHEN** a lambda that captures no variable is translated
- **THEN** the generated environment occupies no space observable by the program

### Requirement: Codegen for `match`

The backend SHALL translate the `match` lowered by the IR into a sequence of comparisons and conditional jumps on the discriminant, or into an LLVM `switch` instruction when the `match` is exhaustive over an `enum`.

#### Scenario: `match` over an exhaustive `enum`
- **WHEN** a `match` that covers all constructors of an `enum` is translated
- **THEN** the generated code uses LLVM `switch` on the discriminant
- **AND** it includes no default branch observable at runtime when there is no `_`

### Requirement: Nullity check

The backend SHALL translate the explicit nullity check produced by the lowering of `??` into a comparison against the type representation's null value, with no additional cost for values the type check already proved non-null.

#### Scenario: Coalescing translated
- **WHEN** `name ?? "anonymous"` is translated
- **THEN** the generated code compares the value of `name` against null before choosing the branch

#### Scenario: No check when the type is not nullable
- **WHEN** an operation on a value of a non-nullable type is translated
- **THEN** the generated code includes no nullity comparison

### Requirement: Object layout

The backend SHALL translate a type with identity into a structure whose header precedes its fields, and whose inherited fields precede its own.

#### Scenario: Shared prefix
- **WHEN** a base class and a subclass are translated
- **THEN** the prefix of the subclass's structure matches that of the base

#### Scenario: Inline record field
- **WHEN** a `record` is a field of another declaration
- **THEN** it is translated without an intermediate pointer

### Requirement: Method tables

The backend SHALL emit one method table per type with virtual methods, and one per implemented interface.

#### Scenario: Stable index on inheritance
- **WHEN** a subclass inherits a virtual method
- **THEN** it occupies the same index as in its base's table

#### Scenario: Dispatch through an interface
- **WHEN** a method is called through an interface
- **THEN** the generated code looks up that interface's table in the descriptor and dispatches through it

### Requirement: Checked cast

The backend SHALL translate a checkable cast into a comparison of the value's type descriptor against the expected one, deferring to the runtime when it does not match.

#### Scenario: Cast that fails
- **WHEN** the descriptor does not correspond to the requested type
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

#### Scenario: Cast that needs no check
- **WHEN** the cast goes up the hierarchy, where the checker already proved it
- **THEN** the generated code includes no comparison at all

### Requirement: Codegen for each integer width over LLVM's native type

The backend SHALL emit each IR integer type as the LLVM `IntType` of the corresponding width, with the arithmetic operations and overflow intrinsics (`*.with.overflow`) matching its signedness.

#### Scenario: Signed checked addition
- **WHEN** code is emitted for an addition on `Int64`
- **THEN** the signed overflow intrinsic for that width is used

#### Scenario: Unsigned checked addition
- **WHEN** code is emitted for an addition on `UInt64`
- **THEN** the unsigned overflow intrinsic for that width is used

### Requirement: Codegen for `Float` over LLVM's native type

The backend SHALL emit each IR `BinaryFloat` type as the LLVM `FloatType` of the
corresponding width, with the `NaN`/invalid-domain check emitted explicitly
around the native operation, not delegated to the backend's floating-point
semantics. Exact-decimal `Float` SHALL NOT use an LLVM `FloatType`; it is
covered by the exact-decimal codegen requirement.

#### Scenario: Explicit check before the result

- **WHEN** code is emitted for a `BinaryFloat` operation the IR marked as
  potentially indeterminate
- **THEN** the generated code checks the condition before the result is used, and
  calls the runtime for the controlled failure if it holds

#### Scenario: Binary width maps to the native type

- **WHEN** code is emitted for a `BinaryFloat32` value
- **THEN** the LLVM `f32` type is used

### Requirement: Codegen for dispatch to `to_string()`

The backend SHALL emit, for every site where `println`/`print`/an interpolation needs to convert a value to text, a call to the `to_string()` method resolved for that value's static type — direct if it is not overridable, through the object's or the contract's table if it is, with the same mechanism as any other method call (ADR-013).

#### Scenario: Conversion of a native type
- **WHEN** code is emitted to print an `Int32`
- **THEN** the call to `to_string()` resolves to `Int32`'s native `to_string()`, with no indirection

#### Scenario: Conversion of an overridable user type
- **WHEN** code is emitted to print a value of a type whose `to_string()` can be overridden by a subclass
- **THEN** the call goes through the same table as any other virtual method of the object

### Requirement: Native code generation supports value-type field pointers
The LLVM backend SHALL emit `getelementptr` over `ValueLayout` for `PointerFromField` when the container is a pointer to a `record`.

#### Scenario: PointerFromField over record
- **WHEN** the IR contains `PointerFromField { object: Pointer<Value>, index: n }`
- **THEN** the backend emits a GEP using the `ValueLayout` of the record, with no object header offset

#### Scenario: PointerFromField over object unchanged
- **WHEN** the IR contains `PointerFromField { object: Object(id), index: n }`
- **THEN** the backend continues to emit a GEP using the `ObjectLayout` with the usual object header offset

### Requirement: Native code generation supports string grapheme offset
The LLVM backend SHALL declare and call `zirk_str_grapheme_offset` to implement `StringGraphemeOffset`.

#### Scenario: StringGraphemeOffset emitted
- **WHEN** the IR contains `StringGraphemeOffset { string, index }`
- **THEN** the backend emits a call to `zirk_str_grapheme_offset` and branches on `-1` to the bounds-fail block

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

### Requirement: Codegen for exact-decimal `Float`

The backend SHALL emit an exact-decimal `Float` as an LLVM aggregate of a
128-bit integer coefficient and an integer scale, passed and returned across the
runtime boundary by pointer, in the same manner as `Int128` and `UInt128`. The
backend SHALL emit `Float` constants directly from the parsed coefficient and
scale, never routing through any binary floating type. The backend SHALL emit
calls to the decimal runtime helpers for arithmetic, comparison, rounding,
formatting, parsing, and conversion.

#### Scenario: Constant emitted without a binary intermediate

- **WHEN** code is emitted for the `Float` literal `0.1`
- **THEN** the aggregate `{ coefficient = 1, scale = 1 }` is emitted directly

#### Scenario: Arithmetic dispatched to the runtime

- **WHEN** code is emitted for `Float` addition
- **THEN** a call to the decimal-addition runtime symbol is emitted, with the
  operands passed by pointer

