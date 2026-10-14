# Browser interpreter support matrix

This is the source-of-truth policy for `add-browser-ir-interpreter`, audited
against `crates/zirk-ir/src/ir.rs` at revision `7183e15`. "Direct" means the
interpreter owns the operation. "Runtime-core" means it is executable after
the platform-neutral semantic helper exists. "Excluded" means browser
preflight must issue a capability diagnostic.

The browser interpreter executes the delivered scalar, text, collection and
object-model surface: integers, binary floats, exact decimal `Float`, booleans,
nullable values, strings and chars; variables, branches, loops, direct and
indirect calls, closures; classes with constructors, inheritance, `#override`,
abstract classes, interfaces, traits with default implementations, static
members, nested/`inner`/local classes, generic classes, records, enums and typed
exceptions; `List`, `Map`, `Set`, `Array`, ranges, regexes and temporal values;
arithmetic, comparisons, conversions, `to_string`, and `stdout.print` /
`stdout.println`. `print(x)` is executable: lowering already inserts `ToString`
for non-strings.

A construct is a public browser claim only when a shared fixture or a program
of the native end-to-end corpus runs through both engines and agrees. The
object-model evidence is catalogued in
[`browser-oop-coverage.md`](browser-oop-coverage.md) and enforced by
`crates/zirk-browser/tests/corpus_parity.rs`, which runs every valid native
corpus program through the browser `run` operation and requires either the
recorded native output or a listed capability exclusion. Structured concurrency, timers, channels, `parallel`, `Weak<T>`, pointers,
native slices, `unsafe` journals and resource transfer are excluded and are
reported as capability diagnostics that name the feature.

## Types

| IR type | Policy | Notes |
| --- | --- | --- |
| `Void`, `Never`, `Int(_)`, `Float(_)`, `Decimal`, `Boolean` | Direct | `Never` only describes controlled divergence. |
| `String`, `Char`, `Regex`, `Range`, `Array(_)`, `List(_)`, `Map(_)`, `Set(_)` | Runtime-core | Own browser heap identity where applicable; bounded work. |
| `Nullable(_)`, `Closure(_)`, `Callable(_)`, `Object(_)`, `Contract(_)`, `Value(_)`, `Enum(_)` | Direct / runtime-core | Direct container/frame representation; layouts and semantic operations require runtime-core as applicable. |
| `Job`, `Channel(_)` | Excluded | Scheduler and channel authority are outside the first host. |
| `Pointer(_)`, `Weak(_)`, `JournalHandle`, `NativeSlice(_)`, `NativeSliceMut(_)`, `Dependent(_)`, `Pin(_)` | Excluded | Native memory, collector and unsafe interoperability. |

## Instructions

| Policy | `InstKind` variants |
| --- | --- |
| Direct | `ConstInt`, `ConstBool`, `Load`, `Store`, `BuildValue`, `BuildEnum`, `Discriminant`, `Undefined`, `Retype`, `IntCast`, `WordCast`, `ConstFloat`, `FloatCast`, `IntToFloat`, `FloatToInt`, `ConstDecimal`, `IntToDecimal`, `DecimalToInt`, `DecimalToFloat`, `FloatToDecimal`, `Unary`, `Binary`, `CheckedArithmetic`, `Call`, `NullValue`, `Wrap`, `IsNull`, `Unwrap`. |
| Runtime-core | `ConstString`, `ConstChar`, `StringGraphemeOffset`, `GraphemeLenAt`, `GraphemeSlice`, `Alloc`, `CheckedCast`, `IsInstance`, `Concat`, `Repeat`, `CallContract`, `CallVirtual`, `LoadField`, `StoreField`, `ToString`, `MakeClosure`, `CallClosure`, `MakeCallable`, `CallCallable`, `ArrayLength`, `ListLength`, `ArrayNew`, `ListNew`, `MapNew`, `SetNew`, `ArrayListLoad`, `ArrayListStore`, `ListAdd`, `ListInsert`, `ListRemove`, `ListRemoveValue`, `ArrayClone`, `ListClone`, `MapLength`, `MapIsEmpty`, `MapSet`, `MapContainsKey`, `MapGet`, `MapRemove`, `SetLength`, `SetIsEmpty`, `SetAdd`, `SetContains`, `SetRemove`, `ArraySlice`, `Clone`. |
| Bounded host | `Print`, `Println`, `IsCancelled`. Output is the only public language effect; cancellation is host-controlled. |
| Structured runtime failure | `FatalError`, `Throw`, `SetSuppressed`, `StackTrace`, `Suppressed`, `HasPendingException`, `TakePendingException`. |
| Excluded | `ScopeEnter`, `ScopeExit`, `BranchStart`, `ParallelForStart`, `JobWait`, `JobDone`, `JobCancel`, `TimerSleep`, `ChannelNew`, `ChannelSend`, `ChannelRecv`, `ChannelTrySend`, `ChannelTryRecv`, `ChannelClose`, `ChannelLength`, `ChannelIsClosed`, `ChannelCapacity`, `ChannelDrain`, `PointerFromSlot`, `PointerFromField`, `PointerRead`, `PointerWrite`, `PointerOffset`, `PointerOffsetBytes`, `PointerCast`, `PointerIsNull`, `WeakFrom`, `WeakUpgrade`, `WeakIsAlive`, `JournalBegin`, `JournalRecordSlot`, `JournalRecordField`, `JournalCommit`, `JournalRollback`, `NativeSliceValidate`, `NativeSliceValue`, `NativeSliceLength`, `NativeSliceLoad`, `NativeSliceStore`, `ResourceTransfer`, `DependentFrom`, `PinObject`, `UnpinObject`. |

## Terminators, externs and host effects

`Return`, `Jump`, `Branch`, and `Unreachable` are direct interpreter control
flow. Every `Module::externs` entry is excluded unless a later browser host
policy explicitly names a bounded implementation; the initial policy names
none. The only permitted host effects are ordered, bounded UTF-8 output and
cooperative cancellation checks. Filesystem, storage, network, environment,
process, shell, native libraries, clocks, randomness, DOM access, timers,
parallel execution, channels and input are excluded.

## Limits

| Resource | Default | Hard maximum |
| --- | ---: | ---: |
| UTF-8 source bytes | 256 KiB | 1 MiB |
| Diagnostics | 100 | 500 |
| Instructions | 2,000,000 | 20,000,000 |
| Call depth | 128 | 1,024 |
| Interpreter heap | 16 MiB | 64 MiB |
| Output bytes | 256 KiB | 1 MiB |
| Output events | 2,048 | 8,192 |
| Captured stack frames | 64 | 256 |

Callers may only lower these limits. The site additionally enforces a
wall-clock deadline by terminating and recreating its worker; that deadline is
containment, not a language semantic.

## Execution trace (observation, not a host effect)

`run` accepts an opt-in `trace` option and then returns ordered, read-only
events: object allocation, field write, call, return, throw, and `output`
markers that interleave accepted output events with object events. Object ids
are per-run and never derive from heap identifiers; object-valued fields render
as references. The trace has its own bounds (defaults 2,048 events, 128 KiB, 64
bytes per rendered value; maxima 16,384 events, 1 MiB, 256 bytes), callers can
only tighten them, and exhausting them truncates the trace without failing the
program. Interpreted code cannot read or influence it. See ADR-022.

## Pinned boundary dependencies

The browser boundary will use `wasm-bindgen 0.2.128`, `serde 1.0.229` with
`derive`, and `serde_json 1.0.151`. All are permissively dual-licensed
(`MIT OR Apache-2.0`), have Rust MSRVs below this workspace's Rust 1.94, and
are the maintained ecosystem interfaces for worker-compatible WASM bindings
and JSON-compatible contracts. No binding or serialization dependency is used
by `zirk-runtime-core` or `zirk-interpreter`.
