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

### Requirement: Native entrypoint lifecycle

Native code generation SHALL emit an internal platform entrypoint that initializes the runtime, initializes application globals, executes the statically selected Zirk entry body after its imported modules, and performs structured shutdown. The process entrypoint SHALL return the runtime's process status only after the application body and all descendant tasks/resources have completed or been cleaned. No source-level function named `main` SHALL be required or invoked implicitly.

#### Scenario: Native startup executes the selected file body
- **WHEN** a compiled application starts with a manifest-selected entry
- **THEN** the internal platform entrypoint runs that Zirk file's top-level body after imported module initialization

#### Scenario: Startup runs the executor loop
- **WHEN** a compiled program starts
- **THEN** the internal platform entrypoint runs the selected Zirk body as the executor's root task and waits for descendants to finish or clean up

#### Scenario: Runtime lifecycle remains internal
- **WHEN** a Zirk source file has no `fn main`
- **THEN** the generated executable initializes and shuts down the runtime without requiring a user-defined entry function

#### Scenario: Descendant task is still cleaning up
- **WHEN** the application entry body completes while a descendant task is still running or cleaning up
- **THEN** the process waits for the executor lifecycle to finish before returning its status

#### Scenario: Invocation order
- **WHEN** the generated entrypoint is inspected
- **THEN** it initializes the runtime and globals before the selected file body
- **AND** it shuts down the runtime after the body and its structured work complete

#### Scenario: Exit code
- **WHEN** a program terminates normally
- **THEN** the process returns the runtime's resulting status

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

### Requirement: Method tables

The backend SHALL emit one method table per type with virtual methods and one dispatch table per implemented contract.

#### Scenario: Stable index on inheritance
- **WHEN** a subclass inherits a virtual method
- **THEN** it occupies the same index as in its base's table

#### Scenario: Dispatch through a contract
- **WHEN** an operation is called through a contract view
- **THEN** the generated code looks up that contract's table in the descriptor and dispatches through it

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

### Requirement: Codegen for cooperative task suspension

Native code generation SHALL emit, for each suspension point (`Timer.sleep`, a
blocking channel operation, `JobWait`, an explicit cancellation check), a call
into the runtime that saves the running branch's callee-saved registers, stack
pointer, and resume address into its control block and transfers control to the
executor, plus a resume label the executor returns to. `BranchStart` SHALL emit a
per-site `extern "C" fn(ptr) -> i64` thunk that reloads captured values, calls the
lifted branch body, and widens the result to the runtime's branch-result word;
codegen SHALL call `zirk_rt_spawn(thunk, capture_block)` for the handle and
`zirk_rt_job_wait(job)` for `JobWait`. `IrType::Job` SHALL lower to LLVM `i64`.
The generated calling convention for an ordinary function SHALL be unchanged
whether or not it reaches a suspension point internally.

#### Scenario: Suspension point emits a context save and a resume label
- **WHEN** codegen processes a `JobWait` or `Timer.sleep` suspension point
- **THEN** the emitted code calls the runtime suspend routine and defines a resume label

#### Scenario: BranchStart emits a thunk and a spawn call
- **WHEN** codegen processes a `BranchStart`
- **THEN** it emits the capture block, a branch thunk, and a `zirk_rt_spawn` call yielding the `i64` job handle

#### Scenario: Function ABI is not split
- **WHEN** codegen emits two functions, one that suspends internally and one that does not
- **THEN** both are emitted with the same calling convention and are called identically

### Requirement: Context-switch support for every supported target

The runtime staticlib SHALL provide a context-switch implementation for each
target triple the toolchain supports (`ADR-004`): macOS aarch64, Linux x86_64,
Linux aarch64, and Windows x86_64. The implementation SHALL correctly save and
restore the callee-saved register set and stack pointer for that platform's
calling convention and SHALL switch to a task-owned stack.

#### Scenario: Round-trip context switch preserves state

- **WHEN** the runtime switches from the executor to a task and back on a supported target
- **THEN** all callee-saved registers and the stack pointer hold their pre-switch values on return

#### Scenario: Every supported target has an implementation

- **WHEN** the runtime is built for any supported target triple
- **THEN** a context-switch implementation for that triple is compiled in

### Requirement: Per-task shadow-stack frame registration

Native code generation SHALL emit shadow-stack frame push and pop operations that
target the running task's control block rather than a single process-global
chain. The runtime SHALL track the current task so that push and pop reach the
correct chain, and the collector SHALL walk every live task's chain during root
enumeration.

#### Scenario: Frame push reaches the running task's chain

- **WHEN** a function activation on a task's stack pushes its shadow-stack frame
- **THEN** the frame is linked into that task's chain, not another task's

#### Scenario: Collection walks all task chains

- **WHEN** a collection runs while several tasks are suspended
- **THEN** root enumeration visits the shadow-stack chain of every live task

### Requirement: Executor lifecycle wraps the entrypoint

Native code generation SHALL emit program startup that runs the selected Zirk
file body as the executor's root task and returns the process exit status only
after that task and all descendants have completed or been cleaned.

#### Scenario: Startup runs the executor loop

- **WHEN** a compiled program starts
- **THEN** the selected file body runs as the root task and the process does not exit while a descendant task is still running or cleaning up

### Requirement: Codegen for parallel regions and the worker pool

Code generation SHALL submit `ParallelRegion` chunks through
`zirk_rt_pool_submit`, join them through `zirk_rt_pool_join` without stopping
the cooperative executor, and emit `zirk_rt_safepoint_poll` at every region
loop back-edge.

#### Scenario: region submits to the pool and joins
- **WHEN** codegen processes a `ParallelRegion`
- **THEN** emitted code submits work to the pool and joins it before producing the region result

### Requirement: Codegen for channel operations

Code generation SHALL emit the `zirk_rt_chan_new`, send, receive, try, close,
and accessor C-ABI calls. In the stackful executor model, a blocking send and
receive SHALL follow the runtime suspension path, which parks and later resumes
the current branch; no separate `zirk_rt_suspend` symbol is required. The try
operations SHALL be plain calls that construct their typed outcome without a
resume label.

Construction SHALL pass the element size, alignment, and GC-reference offsets;
send and receive SHALL pass source or destination element slots indirectly.

#### Scenario: try receive is a plain call
- **WHEN** codegen processes channel `try_receive`
- **THEN** it calls the try-receive ABI without a suspension point

#### Scenario: blocking receive uses stackful suspension

- **WHEN** codegen processes a blocking channel `receive`
- **THEN** it calls `zirk_rt_chan_recv` through the cooperative runtime path and
  the generated function is unwind-capable

### Requirement: Floating-to-integer code generation is checked

Native code generation SHALL check finiteness and destination range before every floating-to-integer conversion. An out-of-range or non-finite operand SHALL use the language's controlled conversion failure path and SHALL never reach an LLVM conversion with undefined result semantics.

#### Scenario: Out-of-range conversion is compiled
- **WHEN** generated code converts a floating value outside the destination integer range
- **THEN** it branches to a controlled failure before executing the conversion instruction

#### Scenario: In-range conversion is compiled
- **WHEN** generated code converts a finite representable floating value
- **THEN** it applies the same rounding rule as the interpreter

### Requirement: Optimized code preserves checked language semantics

Any non-baseline LLVM optimization level enabled for release code SHALL preserve the observable values, diagnostics, and controlled failure categories of the unoptimized native build and the interpreter for the supported corpus and defined edge cases.

#### Scenario: Optimized overflow and conversion paths are compared
- **WHEN** optimized code is checked against baseline native and interpreted execution
- **THEN** all outputs and failure categories agree for checked arithmetic and numeric boundary fixtures

### Requirement: Codegen for synchronization and threads

Native code generation SHALL emit `zirk_rt_thread_run` and a cooperative join for
`Thread.run`, with `zirk_rt_safepoint_poll` at every loop back-edge in the
thread body. More generally, generated managed Zirk code SHALL emit a
`zirk_rt_safepoint_poll` call at every reachable control-flow back-edge. It SHALL
emit LLVM atomic instructions with the requested ordering
for `Atomic` operations, and `zirk_rt_mutex_lock` / `zirk_rt_mutex_unlock` around
a `mutex.with` closure with the unlock on every exit edge. `Concurrent.protect`
SHALL emit a shield-depth increment and a decrement-plus-check on every exit
edge.

#### Scenario: managed loop back-edges emit safepoint polls
- **WHEN** codegen processes a managed loop, including a loop with a `continue` path
- **THEN** it emits `zirk_rt_safepoint_poll` on every reachable back-edge

#### Scenario: Thread.run emits a spawn and join
- **WHEN** codegen processes `Thread.run(work)`
- **THEN** it emits an OS-thread spawn and a cooperative join

#### Scenario: atomic op carries its ordering
- **WHEN** codegen processes `counter.fetch_add(1)`
- **THEN** it emits an LLVM atomicrmw with sequentially consistent ordering

### Requirement: Native code generation supports dynamic objects and contract checks
The native backend SHALL emit operations for dynamic object creation, string-keyed property access, writes, safe lookup, spread, rest, and contract conformance. Contract checks and missing-property failures SHALL enter the ordinary catchable runtime error path. Dynamic values SHALL preserve runtime tags and managed-reference metadata.

#### Scenario: Read a typed contract property
- **WHEN** native code reads a property through a contract-typed object
- **THEN** it uses the checked contract member type and the underlying object identity

#### Scenario: Read a dynamic property
- **WHEN** native code reads a property from `Object` without a known key type
- **THEN** it returns a tagged `Dynamic` value with its runtime type preserved

#### Scenario: Contract validation failure is catchable
- **WHEN** a runtime contract check finds a missing or incompatible requirement
- **THEN** generated code raises the contract error through the normal exception mechanism

### Requirement: Native code generation preserves union value semantics
Native code generation SHALL represent every accepted heterogeneous union, including structural alternatives and mixed structural/nominal/scalar alternatives, with a target-correct tag and payload layout. It SHALL preserve alternative identity through parameters, returns, local and global storage, captures, aggregate values, and compatible package calls. Tag dispatch and payload projection SHALL not expose raw representation to safe source code. Managed-reference metadata and root enumeration SHALL describe only the active payload and remain correct through copying, collection, and control-flow joins. Inline storage SHALL be used only where its measured layout and target ABI are safe; a managed or boxed fallback SHALL be used where necessary without changing source semantics. Package boundaries SHALL agree on canonical alternatives, representation metadata, and calling convention, or reject incompatible artifacts before execution. Intersections SHALL remain static views and SHALL NOT allocate a second runtime object solely to combine member information.

#### Scenario: Union of distinct scalar representations
- **WHEN** a native function accepts and returns `String | Int32`
- **THEN** both alternatives round-trip correctly without type confusion

#### Scenario: Union with a managed object alternative
- **WHEN** a union contains a scalar alternative and a managed-reference alternative
- **THEN** tag dispatch and root enumeration identify the active representation correctly

#### Scenario: Structural union crosses calls and storage
- **WHEN** a tagged structural value is passed, returned, stored, captured, and narrowed
- **THEN** its selected alternative and property values remain correct at every boundary

#### Scenario: Mixed structural and nominal alternatives
- **WHEN** a union contains structural shapes together with class, enum, or scalar alternatives
- **THEN** native layout, payload projection, and narrowing preserve each alternative without confusing references and inline values

#### Scenario: Managed nested payload is traced
- **WHEN** an active structural payload contains one or more managed references
- **THEN** generated root metadata traces the active references and does not interpret inactive payload bytes as roots

#### Scenario: Union remains internal to a package ABI
- **WHEN** a compiled package exposes a union-typed callable to another compatible package
- **THEN** both sides agree on canonical alternatives, tag meaning, payload layout, and calling convention

#### Scenario: Incompatible package representation is rejected
- **WHEN** a package caller and callee were compiled with incompatible union representation metadata
- **THEN** loading or linking rejects the mismatch before the call executes

#### Scenario: Target ABI differs
- **WHEN** two supported native targets classify a union aggregate differently
- **THEN** generated code uses each target's correct internal ABI while preserving the same Zirk semantics

#### Scenario: Oversized inline payload uses a safe fallback
- **WHEN** a structural union payload cannot safely use the target's inline representation
- **THEN** generated code uses a managed fallback or emits a source diagnostic, never an invalid LLVM aggregate

#### Scenario: Intersection view preserves identity
- **WHEN** one object is passed through an intersection of compatible contracts or structural types
- **THEN** native execution uses the same object identity and exposes the combined checked operations

### Requirement: Generic contract methods preserve specialized dispatch
Generic contract methods SHALL dispatch to an implementation compatible with the receiver's contract arguments and the call site's inferred method arguments. The compiler SHALL specialize method implementations for concrete compile-time substitutions or use an equivalent statically verified mechanism; it SHALL NOT require a runtime generic type descriptor. Exported package metadata SHALL fingerprint method type parameters, constraints, template patterns, and substituted callable signatures so incompatible callers are rejected before code emission or execution.

#### Scenario: Dynamic generic method dispatch uses the inferred specialization
- **WHEN** a `Watched<Person>` receiver is invoked with `"ageChanged"` and the call infers `K = "age"`
- **THEN** native dispatch reaches an implementation compatible with the `Person["age"]` callback signature and passes no runtime value for `K`

#### Scenario: Stale generic contract metadata is rejected
- **WHEN** a package caller was compiled against a different generic contract-method constraint or open-template pattern
- **THEN** package validation rejects the mismatch before emitting a native call site
