## MODIFIED Requirements

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

## ADDED Requirements

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
