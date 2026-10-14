# zirk-ir-interpreter Specification

## Purpose

Define the verified-IR interpreter: a deterministic execution engine that runs
`zirk_ir::Module` values through explicit frames, a bounded managed heap, and
a host-injected output sink, without LLVM, a native ABI, or ambient platform
authority.

It exists so browser-hosted tooling can execute Zirk programs with native
semantics, and it is held honest by differential conformance fixtures that
compare its observable behavior against the native pipeline.

## Requirements

### Requirement: Verified IR is the only executable input

The interpreter SHALL execute only typed `zirk_ir::Module` values that have passed the authoritative IR verifier and SHALL NOT reinterpret source syntax, resolve names, or infer types.

#### Scenario: Execute a valid module
- **WHEN** the interpreter receives a module accepted by `zirk_ir::verify`
- **THEN** it begins compatibility preflight and execution without invoking LLVM

#### Scenario: Reject malformed IR
- **WHEN** the interpreter receives a module rejected by `zirk_ir::verify`
- **THEN** it returns a structured internal compiler failure
- **AND** it executes no instruction from that module

### Requirement: Deterministic control-flow execution

The interpreter SHALL execute IR instructions in basic-block order, apply `Jump`, `Branch`, `Return`, and `Unreachable` terminators according to the IR contract, and support cyclic control flow subject to the instruction budget.

#### Scenario: Execute a conditional branch
- **WHEN** a branch terminator evaluates to true
- **THEN** execution continues at the branch's true target and does not execute the false target

#### Scenario: Execute a loop
- **WHEN** a block graph jumps back to an earlier block
- **THEN** the interpreter continues through the cycle until the program exits or a configured limit is exhausted

#### Scenario: Reach an unreachable terminator
- **WHEN** execution reaches `Unreachable`
- **THEN** the interpreter returns a structured internal failure with the originating source span

### Requirement: Typed runtime values preserve Zirk semantics

The interpreter SHALL represent each supported IR scalar, nullable, callable, closure, object, contract, record, enum, string, regex, range, and collection value without coercing it to JavaScript semantics. Width, signedness, exact-decimal behavior, Unicode grapheme behavior, nullability, identity, and structural equality SHALL match the native implementation for browser-compatible programs.

#### Scenario: Detect narrow integer overflow
- **WHEN** an interpreted integer operation exceeds its declared signed or unsigned width
- **THEN** execution produces the same controlled runtime failure category as native execution
- **AND** the value does not silently wrap

#### Scenario: Preserve exact decimal arithmetic
- **WHEN** interpreted `Float` arithmetic evaluates `0.1 + 0.2`
- **THEN** the result uses the exact-decimal representation and equals `0.3`

#### Scenario: Compare identity-bearing values
- **WHEN** `is` compares two references to the same interpreter heap identity
- **THEN** the result is true
- **AND** equivalent content stored under distinct identities does not make `is` true

#### Scenario: Compare canonically equivalent strings
- **WHEN** interpreted string equality compares canonically equivalent Unicode content
- **THEN** it returns the same result as the native runtime

### Requirement: Calls, closures, and dispatch use explicit frames

The interpreter SHALL execute direct calls, indirect callable calls, captured closure environments, object virtual dispatch, and contract dispatch using module metadata and explicit Zirk frames. It SHALL enforce a configurable call-depth limit independently of the host stack. A call whose callee returns `Void` SHALL NOT require a return value when the callee frame completes, regardless of whether the call instruction declared a result value.

#### Scenario: Invoke a captured closure
- **WHEN** an interpreted closure reads a value captured when the closure was created
- **THEN** it reads the value from that closure's environment and returns it through the normal frame mechanism

#### Scenario: Invoke a `Void` function as a statement
- **WHEN** a program calls a `Void` function or method in statement position
- **THEN** the callee executes to completion and control returns to the caller without an internal failure

#### Scenario: Invoke a `Void` function under `try`
- **WHEN** a `Void throws` function is called inside a `try` block and returns normally
- **THEN** execution continues after the call without a `non-void call returned no value` failure

#### Scenario: Dispatch an overridden method
- **WHEN** a virtual call targets an object whose runtime layout selects an override
- **THEN** the interpreter invokes the same resolved method entry selected by native dispatch metadata

#### Scenario: Exhaust call depth
- **WHEN** another call would exceed the configured Zirk call-depth limit
- **THEN** execution ends with a `limit-exceeded` result naming `call-depth`
- **AND** the result includes the bounded Zirk call stack

### Requirement: Managed interpreter heap is bounded

The interpreter SHALL use stable, non-address heap identities for managed values, SHALL track their resource cost, and SHALL enforce a configurable heap budget. All execution-owned values SHALL become reclaimable when the execution instance is discarded.

#### Scenario: Allocate within the heap budget
- **WHEN** a program allocates an object and sufficient interpreter heap budget remains
- **THEN** the interpreter returns a stable identity that remains valid for the execution

#### Scenario: Exhaust the heap budget
- **WHEN** an allocation would exceed the configured interpreter heap limit
- **THEN** the allocation is not committed
- **AND** execution ends with a `limit-exceeded` result naming `heap`

#### Scenario: Finish an execution
- **WHEN** an execution completes, fails, or is cancelled cooperatively
- **THEN** dropping its interpreter instance releases all execution-owned heap entries

### Requirement: Exceptions and runtime failures remain structured

The interpreter SHALL preserve typed throws, exact rethrows, suppressed failures supported by the IR, fatal runtime failures, source spans, and a bounded Zirk stack trace. User-triggered failures SHALL NOT be exposed as Rust panics. Type-narrowing instructions used by `catch` paths, `as`/`as?` casts, and subtype-to-supertype coercion (`Retype`) SHALL execute with the same observable semantics as the native pipeline, and an uncaught throwable's message SHALL render the throwable's semantic message rather than an internal heap identity.

#### Scenario: Catch a typed exception
- **WHEN** a thrown value matches an active Zirk catch path represented by the lowered IR
- **THEN** control transfers through that path with the original throwable value

#### Scenario: Narrow a caught throwable to its type
- **WHEN** a `catch` clause binds a throwable narrowed by `Retype`
- **THEN** the bound value has the narrowed type and the handler body executes normally

#### Scenario: Pass a subtype to a supertype parameter
- **WHEN** a subclass value is passed where a supertype or interface is expected
- **THEN** the call executes with the coerced value and dispatches as native dispatch does

#### Scenario: Return an uncaught runtime failure
- **WHEN** a failure reaches the root frame without a handler
- **THEN** execution returns a structured runtime failure with category, the throwable's semantic message, span, and bounded Zirk frames
- **AND** the message does not expose internal heap identities such as `Heap(HeapId(N))`

#### Scenario: Preserve a suppressed cleanup failure
- **WHEN** lowered cleanup records a secondary failure while another throwable is primary
- **THEN** the primary failure remains primary and the secondary failure is present in its suppressed failures

### Requirement: Host output is ordered and bounded

The interpreter SHALL route `stdout.print` and `stdout.println` through an injected host output sink, preserving UTF-8 text, call order, and newline behavior. It SHALL enforce output byte and event limits without writing to native stdout.

#### Scenario: Capture ordered output
- **WHEN** a program prints `"a"`, then prints `"b"` with a newline
- **THEN** the host observes ordered output equivalent to `ab\n`

#### Scenario: Exhaust the output limit
- **WHEN** another output event would exceed the configured byte or event limit
- **THEN** retained output is marked truncated
- **AND** execution ends with a `limit-exceeded` result naming `output`

### Requirement: Execution budgets are deterministic

The interpreter SHALL account for executed instructions and SHALL apply weighted costs or explicit input bounds to non-constant-cost semantic helpers. Limit checks SHALL occur often enough that cyclic IR cannot run indefinitely inside a cooperative invocation.

#### Scenario: Exhaust the instruction budget
- **WHEN** execution consumes the configured instruction budget before terminating
- **THEN** it returns a `limit-exceeded` result naming `instructions`
- **AND** no further Zirk instruction is executed

#### Scenario: Charge a variable-cost operation
- **WHEN** an interpreted regex, text, or collection operation processes input whose cost grows with input size
- **THEN** the interpreter charges documented weighted work or enforces the operation's input bound

### Requirement: Browser-incompatible capabilities fail preflight

The interpreter SHALL expose a compatibility preflight that rejects native-only types, instructions, extern declarations, or host effects before program execution. Rejection SHALL be deterministic, sorted by source location, and expressed as structured capability diagnostics. Externs whose semantics are pure computation — decimal arithmetic, integer/float member helpers, string members, regex operations, duration members, and pure date/time/datetime parsing and arithmetic — SHALL be provided as browser intrinsics rather than rejected. Only externs that genuinely require host authority (wall-clock time, filesystem, process, journal) SHALL fail preflight.

#### Scenario: Module declares an extern function
- **WHEN** a verified module contains an extern declaration not provided by the browser host policy
- **THEN** preflight reports an unsupported `extern` capability
- **AND** the entrypoint is not executed

#### Scenario: Module uses a pure computational extern
- **WHEN** a verified module requires `zirk_rt_decimal_add`, `zirk_int_checked_pow_*`, `zirk_str_split`, or a pure `zirk_rt_date_*`/`zirk_rt_time_*`/`zirk_rt_datetime_*` helper
- **THEN** preflight accepts it and execution produces the native-equivalent result

#### Scenario: Module uses integer division
- **WHEN** an interpreted program evaluates `10 / 3` between integers
- **THEN** the result is the exact `Decimal` quotient, matching native execution

#### Scenario: Module contains a native pointer operation
- **WHEN** a verified module contains pointer, native-slice, pinning, dependent-reference, or unsafe journal operations
- **THEN** preflight reports the relevant unsupported browser capability
- **AND** no fake pointer value is created

#### Scenario: Module uses only browser-compatible operations
- **WHEN** every required type, instruction, intrinsic, and host effect is supported
- **THEN** preflight succeeds and execution may begin

### Requirement: Effect instructions do not fabricate result values

An IR instruction whose semantics produce no value (`Void` calls in statement position, `ListAdd`, `ListInsert`, `MapSet`, `SetAdd`, `Store`, `StoreField`, `Throw`, and other effect-only instructions) SHALL NOT be emitted with a declared result value, and the interpreter SHALL NOT expect one. A verified module in which a `Void`-typed instruction declares a result value SHALL either be rejected by the verifier or executed consistently by all engines.

#### Scenario: Add to a list
- **WHEN** an interpreted program executes `l.add(1)` on a `List<Int32>`
- **THEN** the element is appended and no `instruction result was not materialized` failure occurs

#### Scenario: Set a map entry
- **WHEN** an interpreted program executes `m.set(k, v)`
- **THEN** the entry is stored and execution continues normally

#### Scenario: Statement-position `Void` call emits no result
- **WHEN** the lowering emits a call to a `Void` callee in statement position
- **THEN** the instruction carries no result value, or the verifier rejects the module deterministically

### Requirement: Interpreter and native behavior are continuously compared

The repository SHALL maintain browser-compatible conformance fixtures that execute through both the interpreter and native pipeline and compare observable output, completion category, and runtime failure category.

#### Scenario: Compatible fixture agrees
- **WHEN** CI runs a fixture marked browser-compatible through both engines
- **THEN** normalized output, completion, and failure expectations match

#### Scenario: Engine behavior diverges
- **WHEN** a fixture produces different observable behavior between native and interpreted execution
- **THEN** verification fails with the fixture name and differing observations
