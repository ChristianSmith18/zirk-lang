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
- **WHEN** a subclass value is passed where a supertype or contract is expected
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

### Requirement: Execution trace is an opt-in, bounded, read-only observation channel

When tracing is enabled, the interpreter SHALL emit an ordered trace of object allocation, field write, method call, method return, and throw events, each stamped with a sequence number, plus output markers that record where each accepted output event falls in that order. Allocation events SHALL carry the class name and a per-run object id assigned in allocation order. Field-write events SHALL carry the object id, field name, declared type, and a rendered value bounded in length. The trace SHALL be bounded by configured event and byte limits, SHALL NOT be readable by the interpreted program, and SHALL NOT change the program's output, completion, failure category, or instruction-limit behavior.

#### Scenario: Object lifecycle is traced
- **WHEN** a traced program constructs an object and assigns one of its fields
- **THEN** the trace contains an allocation event for that object followed by a field-write event naming the field
- **AND** both carry increasing sequence numbers

#### Scenario: Trace interleaves with output
- **WHEN** a traced program prints between two field writes
- **THEN** the output marker's sequence number falls between the two field-write sequence numbers and names the output event index

#### Scenario: Object-valued field renders as a reference
- **WHEN** a traced program assigns an object to a field of another object
- **THEN** the field-write event renders the value as a reference to the target's object id and does not render the target's contents

#### Scenario: Tracing does not change behavior
- **WHEN** the same program runs with tracing enabled and disabled
- **THEN** output, completion, failure category, and the instruction count at which a limit trips are identical

#### Scenario: Trace budget is exhausted
- **WHEN** the number of events or bytes would exceed the configured trace limit
- **THEN** recording stops and the trace is marked truncated
- **AND** the program continues to run normally

#### Scenario: Object ids are deterministic
- **WHEN** a program runs twice with identical input and limits
- **THEN** both traces contain identical object ids and sequence numbers

### Requirement: Output pieces of one line form one logical event

The interpreter SHALL treat consecutive writes to the same standard stream that are not separated by a line break as one logical output line. It SHALL count such a line once against the output-event allowance, and the captured host SHALL deliver the pieces of a line as a single event whose text is their concatenation. Byte accounting, stream separation, and ordering SHALL be unchanged.

#### Scenario: Several writes in one `println`

- **WHEN** an interpreted `println("a", 2)` writes `a`, a space, `2`, and a line break
- **THEN** the host observes one event with text `a 2\n`
- **AND** one output event is counted

#### Scenario: Interleaved streams

- **WHEN** a program writes part of a line to stdout and then to stderr
- **THEN** each stream's text stays in order and the two streams are not merged into one event

### Requirement: Deep clone executes on the interpreter heap

The interpreter SHALL execute deep `clone()` with the graph semantics of the language specification: every reference object reachable from the receiver through cloneable members SHALL receive a new identity, sharing inside the source graph SHALL be preserved inside the cloned graph, cycles SHALL terminate and mirror the source cycle among new identities, and the runtime class of each object SHALL decide its layout regardless of the static receiver type. The operation SHALL grant no host authority.

#### Scenario: Clone is independent of the source
- **WHEN** an interpreted program clones an object and then mutates a field of a nested object in the clone
- **THEN** the corresponding field in the source graph is unchanged and the output matches native execution

#### Scenario: Shared child stays shared
- **WHEN** two fields of the cloned root refer to one child before cloning
- **THEN** the cloned fields refer to one new child distinct from the source child

#### Scenario: Cyclic graph
- **WHEN** a cloned object graph contains a field that refers back to an ancestor
- **THEN** the clone completes without unbounded recursion and the cloned cycle refers to the new ancestor

#### Scenario: Clone through a supertype reference
- **WHEN** a program clones a subclass instance through a variable declared with its superclass type
- **THEN** the clone has the subclass's runtime type and fields

#### Scenario: Callables inside the graph
- **WHEN** a closure that captured an object is cloned and the original object is then mutated
- **THEN** the cloned closure observes the captured value as it was at clone time, matching native execution

### Requirement: Deep clone work is bounded

Deep clone SHALL be charged against the configured instruction budget and heap limit before it allocates, and exhausting either SHALL end execution with the existing `limit-exceeded` result without a partially visible clone and without mutating the source graph.

#### Scenario: Clone exceeds the heap limit
- **WHEN** cloning a graph would exceed the configured heap limit
- **THEN** execution ends with a `limit-exceeded` result naming `heap`
- **AND** the source graph is unchanged

#### Scenario: Clone exceeds the instruction budget
- **WHEN** the work to clone a graph would exceed the remaining instruction budget
- **THEN** execution ends with a `limit-exceeded` result naming `instructions`

### Requirement: Unsupported object-model constructs fail as capability diagnostics

When a verified module uses an object-model construct that the browser interpreter does not execute, preflight SHALL report a structured capability diagnostic that names the construct, and the entrypoint SHALL NOT execute. The interpreter SHALL NOT surface such a case as an internal failure.

#### Scenario: Construct lowers to an excluded instruction
- **WHEN** a program uses an object-model feature whose lowering requires a browser-excluded instruction
- **THEN** the run result is a capability failure that identifies the feature and its source location
- **AND** no output from the program is produced

### Requirement: Browser object-model coverage is fixture-backed and discoverable

The repository SHALL keep a checked-in catalog that lists each documented object-model construct with its browser verdict and the conformance fixture that evidences it, and the support matrix and Feature Status SHALL derive their object-model claims from that catalog.

#### Scenario: Construct without a passing fixture
- **WHEN** a documented object-model construct has no passing shared fixture
- **THEN** the catalog marks it unclaimed
- **AND** public documentation does not describe it as browser-supported

#### Scenario: Catalog drift
- **WHEN** a fixture named by the catalog is removed or fails in either engine
- **THEN** repository verification fails and names the construct and fixture

### Requirement: Interpreter values preserve dynamic object semantics
The interpreter SHALL represent every supported scalar, nullable, callable, closure, dynamic object, contract view, enum, string, regex, range, and collection value without coercing it to host JavaScript semantics. Dynamic `Object` values SHALL have stable identity and tagged string-keyed properties. Property reads, writes, safe lookups, key sealing, contract checks, spread, and rest SHALL match the native implementation. `Dynamic` values SHALL preserve their runtime tags and require the same checked narrowing as native execution.

#### Scenario: Dynamic object mutation matches native execution
- **WHEN** an interpreted program adds a property and reads it back
- **THEN** its value and identity behavior match native execution

#### Scenario: Runtime contract mismatch is catchable
- **WHEN** an interpreted dynamic object fails a contract check inside `try`
- **THEN** the same typed contract error is caught as in native execution

### Requirement: Contract and object programs execute with native-equivalent behavior
The interpreter SHALL execute the supported object-oriented surface, including classes, constructors, fields, inheritance, overrides, abstract classes, contracts, traits, static and nested classes, generic class methods, callable object properties, open dynamic objects, class instances with dynamic properties, and alias-visible key sealing with the same output, completion, and failure category as the native pipeline. Browser-support claims SHALL continue to require shared conformance fixtures.

#### Scenario: Contract dispatch uses the underlying value
- **WHEN** an interpreted class or object is called through a compatible contract view
- **THEN** dispatch invokes the compatible class method or stored callable property

#### Scenario: Sealed object remains sealed through aliases
- **WHEN** a dynamic object is assigned to a contract view and later accessed through another alias
- **THEN** no alias can add or remove keys
