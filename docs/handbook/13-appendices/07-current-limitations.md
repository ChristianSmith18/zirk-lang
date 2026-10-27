# Current Limitations

Zirk's final 1.x semantics are broader than the current compiler. The repository
has a precedence table, stable diagnostic-code policy, and normative detail for
ranges, slicing, traditional `for`, patterns, generators, and callable types.
Tuples, `Duration` literals, `Regex` literals with `matches`/`find`/`replace`,
the `String` mutation/search surface, `Char` classification/normalization, and
`type` alias lowering are now delivered; `value class`, `record`, and `interface` declarations were removed; use
`Object` for open data, `contract` for structural requirements, and `class` for
nominal reference types. Their existence in documentation does not imply every pipeline
stage implements them.

The delivered `Map`/`Set` runtime remains intentionally narrower than the
generic API contract: entries use a one-word representation, supported managed
references are traced, mutable text keys/elements are normalized snapshots, and
aggregate layouts that cannot be represented safely are rejected by semantic
analysis. The native Regex cache is bounded to 128 entries, 16 KiB per source
pattern, 256 KiB total retained pattern text, and 256 KiB compiled size per
pattern; when full, new patterns are rejected instead of evicting handles that
may still be live. Numeric casts and temporal arithmetic use checked bounds;
native/interpreter parity coverage includes the documented overflow edges.

Current high-impact delivery limits include:

- Type algebra is implemented in parsing, semantic checking, the LSP, and
  native lowering for tested cases. `Int32 | String`, scalar/temporal
  singleton unions, enum-constant unions, and class-reference unions narrowed
  with `is` have native execution coverage; interpreter/native parity covers scalar, mixed singleton, nullable, enum,
  temporal, and numeric singleton alternatives, but broader control-flow,
  error, and cleanup paths remain incomplete.
  Focused macOS ARM64 native/interpreter cases now execute discriminated
  structural unions, a structural/scalar union through a function, an
  identity-preserving structural intersection, and a captured structural
  payload; forced-GC coverage verifies the managed payload stays live.
  These cases do not establish a general ABI for every structural alternative
  or collection. Unsupported combinations still fail before backend emission.
  Native union execution is verified on macOS ARM64 and Linux ARM64; Linux
  x86_64 union ABI and parity cases pass under Docker emulation, while native
  x86_64 CI and Windows x86_64 execution remain unverified. Dynamic
  JSON serialization now rejects handles that are not exact managed allocations
  before dereferencing them; Linux ARM64 object-engine parity passes after
  rebuilding the runtime and CLI. Tag-aware GC
  roots cover function frames, object fields, nested enum payloads, closure
  captures, `List<Union>` storage, channel queues, suspended-task captures,
  and caught exception fields. Additional specialized managed-container
  boundaries still need stress coverage. Generic aliases,
  `keyof`, indexed access, conditional/infer, mapped types, finite template
  types, one-hole open template inference from string singletons (including
  `"{K}Changed"` and contextual `T[K]` callbacks through generic functions
  and `Watched<T>` contract methods), standard utilities,
  flow-aware IDE queries, and bounded diagnostics have semantic test coverage.
  Generic contract-method specializations have native/interpreter parity for
  in-program calls, and compiled packages fingerprint exported generic
  contract signatures. The complete supported-target matrix is not yet
  verified.

- `.zkinit` now has a typed project model, `zirk init`/`-y`, project-aware
  entry selection without a required source `fn main`, dependency/dev-dependency
  graphs, typed application globals, and one `requires` capability list. Runtime
  consent is exact-resource and operation-local. Runtime enforcement covers
  environment/secrets, networking, and `std.fs`/`std.path`, including bounded
  directory/link/temp/lock/watch APIs and task-suspending async file/directory
  operations. Browser execution still rejects filesystem effects; Windows
  filesystem behavior has compile-only evidence in this change, and process
  APIs remain unavailable. See the
  [feature status catalog](../../init/ZIRK_FEATURE_STATUS.md) and project
  chapters for the tested surface. Focused native filesystem conformance passes
  on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64 in
  [CI run 40230912316](https://github.com/ChristianSmith18/zirk-lang/actions/runs/40230912316)
  at source revision `add179e6`. The workflow is not fully green: native jobs
  later fail at the repository-wide canonical-format check, and the independent
  browser Fetch HTTP job fails. Browser filesystem preflight tests pass locally.

- `std.environment`, `std.net`, and `std.http` are bundled native previews;
  the archived first delivery passes full-workspace native CI on macOS ARM64,
  Linux x86_64, Linux ARM64, and Windows x86_64 (run 39359107965, compiler
  revision b28daa7). A separate extension is in progress for UDP, local
  sockets, interface snapshots, extended TLS, HTTP/3/QUIC, and injected browser
  Fetch. Its working-tree evidence and remaining target gates are listed in the
  [standard-library status catalog](../../init/ZIRK_FEATURE_STATUS.md). The
  broader Phase 7 library remains partial; the [module
  index](../04-standard-library/19-standard-library-indexes.md) describes the
  public APIs.

- Package roots, local/vendored dependency traversal, `share mod`, logical
  package imports and lock metadata validation are delivered. Registry/Git
  acquisition, cryptographic archive verification, package publishing and
  complete collision resolution across opened module members are still
  pending.

- `zirk format` is canonical for the delivered source grammar, but it does not
  format `.zkinit` manifests, accept style configuration, or support
  range-formatting. Use `fmt off` / `fmt on` for deliberate exceptions.

- `Fn(P...) => R` is parseable, type-checked, and lowered in every position
  (Phase 4d). A named function, capture-less lambda, or captured closure works
  as a callable value, including return, local assignment, field storage, and
  parameter passing. `.clone()` on callables is supported when every captured
  value is `Clone`, producing an independent capture block, and capture blocks
  are GC-tracked.
- Phase 4a–4c implement expected errors, explicit exceptions, and single- and
  grouped-resource `match ... with`. Grouped acquisition runs left-to-right and
  cleanup runs right-to-left; `transfer(r)` works for `TransferableResource`
  values, and use-after-transfer is rejected at compile time. Native
  `ArithmeticOverflowError` and `InvalidCastError` are catchable as
  `RuntimeError` subclasses. `ResourceFailure<BodyError,CloseError>` merging
  and cancellation-aware close dispatch are wired in the grouped-resource error
  path.
- Managed memory reclaims automatically now: `docs/decisions/ADR-003-memoria.md`
  closed on a non-moving mark-sweep collector, real (not a placeholder),
  reclaiming unreachable memory including cycles — including all `String`
  and `Char` handles — without exposing GC, ownership, or moves as source
  semantics. `Weak<T>` is delivered
  (`Weak.from`, `.upgrade(): T?`, `.is_alive`) — a small collector-tracked
  indirection cell whose target is cleared before the collector frees it,
  never after, so `.upgrade()`/`.is_alive` never observe reclaimed memory.
  Deep `clone()` is delivered for `class` receivers: compiler-derived when
  every field is itself `Clone`, preserving internal sharing and cycles
  through a runtime memoization map, and rejected at compile time when the
  field graph reaches a `Pointer<T>`, `Resource`, or other non-`Clone`
  member. `record` and enum-typed fields do not yet derive
  `Clone` (records have no identity of their own; a pre-existing codegen
  gap leaves an enum's inactive-variant fields uninitialized in a way the
  shared field-offset walk would read unconditionally). `Dependent<T>` lifetime
  and escape analysis rejects returns, field stores, closure captures, and
  non-dependent parameter passing; valid local use runs end-to-end. `Pin<T>`
  supports construction with `Pin(obj)`, automatic unpin for field and method
  access, and rejects reassignment of the pinned variable. `inmut::strict`
  on a field declaration rejects writes through any projection and rejects
  calls to inferred-mutating methods on a strict reference (the diagnostic
  shows the mutation chain). `unsafe fn`/`unsafe {}`/
  `commit {}` parse and are context-checked; `Pointer<T>` (an ABI-safe
  element-type subset) supports construction/read/write/offset/cast with a
  conservative escape rule; `extern "C" fn` declares and calls a native
  function under a narrow ABI-safe signature. The transactional
  journal/rollback contract is delivered: `unsafe {}` journals every managed
  write to state declared outside the block, commits durably on normal
  exit, and rolls back every recorded write in reverse order when an
  exception becomes pending before that exit; `commit {}` durably publishes
  the enclosing block's journal at its own entry, before running its own
  body, and writes after that point are no longer journaled. A `try`/`catch`
  nested inside `unsafe {}` that handles the exception locally does not
  trigger a rollback; early `return`/`break`/`continue` out of `unsafe {}`
  now also rolls the active journal back before the jump.
  `NativeSlice<T>`/`NativeSliceMut<T>`
  are delivered: validated bounded views constructed only via
  `pointer.as_slice(length)`/`.as_slice_mut(length)` (both `unsafe`,
  returning `Result<view, NativeError>`), checking nullability, alignment,
  extent, and (for the syntactically direct `Pointer.from(place).as_slice(n)`
  shape only) known extent against the real underlying storage; using an
  already-constructed view needs no `unsafe`, with bounds checks active on
  every index; the `Pointer<T>` escape rule now also covers both view
  types. `String[index]` returning a `Char` is supported, with bounds
  checking and negative-index rejection, and `array-list-tuple-duration-regex`
  adds the `String[index] = c` write, `[start:end:step]` slicing, and the
  `trim`/`search`/`contains`/`starts_with`/`ends_with`/`substring` methods
  (`split` returns `List<String>` and lands with `List<T>`). Delivering
  indexing also meant adding `expr[index]` as a genuine new
  postfix expression grammar (`Expr::Index`) — indexing did not exist
  anywhere in the compiler before, and is now dispatched by receiver type
  (today: `String`, `NativeSlice<T>`/`NativeSliceMut<T>`) so the
  `Array<T>`/`List<T>` of `array-list-tuple-duration-regex` register their
  own support without another grammar change. Volatile access, untagged native-union access (no union type
  exists), language-level atomics and their source-level ordering contract,
  native-library-linking manifest, and general provenance tracking for a
  view's known extent beyond the one recognized syntactic shape also
  remain.
- Several Phase 3 constructs parse and type-check more broadly than they lower.
  Delivered since: a user-declared `abstract class` now dispatches dynamically
  through its concrete adopters (`fase-3-abstract-dispatch`), reusing the same
  vtable mechanism the compiler's own `Throwable` hierarchy already proved —
  fixing, along the way, three latent gaps in the general class-declaration
  path (method `overridden` flags never set for a user abstract class,
  method-index seeding scoped only to the three native exception classes,
  hierarchy ordering that ignored `implements`) and a pre-existing diagnostic
  bug (`MISSING_OVERRIDE` reported instead of `MISSING_IMPLEMENTATION` when no
  override at all was supplied). Derived structural equality for
  `record` is also delivered (`fase-3-structural-equality`):
  `==`/`!=` compares every field, recursing into a nested `record` field,
  and short-circuits on the first difference — a `class`-typed
  field compares by its own existing rule (its `_equals` if declared,
  otherwise `is`). A field whose type doesn't fit a scalar, nested-value,
  or object shape (for example `T?` or an algebraic enum payload) still
  reports `NOT_LOWERED`, now scoped to that specific field rather than
  rejecting the whole comparison. A user-declared generic enum
  (`fase-3-generic-enums`) also instantiates and lowers now, for a flat
  single- or multi-type-parameter shape (`enum Either<L, R> { ... }`),
  reusing the same specialization mechanism already proven by
  `Result<T,E>`. Two real, deeper gaps were found and reported rather than
  fixed as part of that change (both judged out of its own "verify an
  existing mechanism" scope): a variant payload naming another generic
  instantiation (`Bar<Baz<T>>`) — since fixed by `fase-3-generic-substitution-recursion`
  (`infer_type_params`/`substitute` now recurse into a nested instantiation's
  own type arguments the same way `substitute_type` already did, closing a
  gap shared by every generic function/method call, not just enums); and a
  self-referencing enum declaration could not be declared at all, because
  enum declaration resolved variant field types before registering the
  enum's own name. `fase-3-recursive-enums` closed the declaration-order
  half of that: an enum and a class (or two enums) can now reference each
  other regardless of order, the same way two classes already could. A
  genuinely self-referential enum field (`enum IntList { Nil; Cons(head:
  Int32; tail: IntList); }`) still cannot be constructed or used — every
  enum lowers to an inline-flattened struct with no indirection anywhere
  in the pipeline, so such a field asks for an infinitely-sized type. That
  implementation attempt found this crashed the compiler with a real stack
  overflow; it is now a clean compile-time diagnostic naming the cycle
  instead. Delivering actual construction/pattern-matching needs automatic
  heap indirection ("boxing") for such a field — a new `IrType` case plus
  runtime/GC integration — tracked as its own future change.
  `fase-3-value-type-contract-dispatch` closed the last large open design
  question from Phase 3: a `record` implementing a contract now dispatches
  correctly through a contract-typed reference. Converting a value to a
  contract-typed reference boxes it into an ordinary, collector-tracked
  heap allocation with a real descriptor, built by reusing the exact same
  object-layout construction a `class` already goes through — no new,
  divergent descriptor builder, and `CallContract`'s own existing dispatch
  needed zero changes. A real bug was found and fixed along the way: a
  value type's own method is compiled expecting `this` by value, but
  `CallContract` always calls through a pointer, so pointing a contract
  table straight at the value's own method silently miscompiled — fixed
  with a small per-method unboxing thunk, needed for a `record`'s own
  method bodies (a trait's inherited default already expects a pointer
  receiver). The Phase 3 OOP closing slice then delivered user generic
  contracts end-to-end — `class Box<T> implements Container<T>` and generic
  records dispatch through `Container<...>`-typed references, including
  members that name the contract's own `T` — plus `static` methods and
  fields on `class`, field initializers `field: Type = expr;`, the nullable
  cast `as?`, `TraitName.super.method()`, contract composition through
  `implements` on `interface`/`trait` (transitive, generic-aware, with cycle
  and signature diagnostics), trailing type-parameter defaults `<T = Int32>`
  checked against `from`, comparison operators as reserved-method contracts
  (`_less`/`_less_equal`/`_greater`/`_greater_equal`, `!=` via `_equals`),
  and positional `in`/`out` variance verification. Still open in this area:
  trait default methods under generic substitution, a nested instantiation
  as a contract argument (`Container<Box<T>>`, rejected with a diagnostic),
  type-parameter defaults and bodies for user-defined generic functions, and
  `static` members on `record`/`enum`.
- `array-list-tuple-duration-regex` delivers the everyday data surface:
  `Tuple(A, B, ...)` values with constant indexing and destructuring,
  `Duration` as an `i64`-nanosecond primitive with `ns`–`w` literal
  suffixes, and `Regex` as a native reference type with `re'...'`
  literals, `matches`, `find` (a `Regex.Match?` with positional and named
  `group`), and `replace`. `Array<T>` (fixed-capacity) and `List<T>`
  (resizable) are native collector-tracked reference collections with
  bounds-checked indexing, `length`/`is_empty`, and `for ... in`
  iteration; their delivery is completing in parallel within the same
  change. `value class` and record value semantics are removed; use Object, class, tuple, or enum according to identity and shape.
  Also delivered inside the change: `Regex.split` and
  `Regex.find_all(text): List<Regex.Match>` for match iteration
  (`matches(text)` keeps its `Boolean` meaning), `re'...'` patterns in
  `match` arms, `Range<T>` (`start`, `end`, `step`, colon steps, inferred
  direction, slicing, expansion, and `Iterable<T>` for integer `T` and
  `Duration`), derived `Clone` for
  enum, `String` writes `s[i] = c` and slicing
  `s[start:end:step]`, and lowering of user-defined generic `implements
  Contract<T>` satisfaction (contract members that name the contract's own
  `T` were since covered by the Phase 3 OOP closing slice described above).
  `String.split` remains pending.
- `Float128` arithmetic lacks complete Windows verification. `Float128`
  `to_string()` is implemented by truncating to `Float64` first, which can
  lose precision for values not exactly representable in `f64`.
- Exact `Decimal` irrational operations (`sqrt`, a fractional `pow`) carry only
  `f64`-grade precision (~15 significant digits), not the full
  28-digit budget the rational operations use. `Decimal.format(spec)` and an
  explicit `RoundingMode` for `div` / `round` are specified but not implemented;
  both round half-to-even by default.
- The reviewed Phase 5 concurrency slice is delivered for its documented
  scope. `concurrent {}`, `spawn`/`Job<T>`,
  `Timer`, `parallel`, typed channels, completion combinators, bounded
  `Concurrent.detach`, and `Mutex<T>.with` have compiler/runtime support,
  including bounded completion and collection combinators, managed
  `Thread.run`, supervised services, and scalar `Atomic<T>`. Cancellation is
  cooperative at runtime safe points and interruptible waits; it does not
  preempt arbitrary loops or blocking native calls. The runtime's `Transfer`
  / `Share` and effect checks do not yet establish general end-to-end race
  freedom. `RwLock`, `Semaphore`, `Barrier`, and `Once` are not available as
  source APIs. Native `Thread.run(fn)` is available on supported 64-bit targets
  with a 64-worker/1,024-queued-callback bound;
  blocking FFI calls without a proven collector transition remain rejected.
  Source `Atomic<T>` supports Boolean
  and signed/unsigned integers through 64 bits only when the target provides
  the matching atomic width. Its scalar payload is inline in the GC-managed
  object; pointer atomics are unsupported. Operations default to `SeqCst`,
  weaker orders require `unsafe`, and dynamically invalid orders raise
  `InvalidArgumentError`. Native-host E2E coverage passes. The runtime library
  also builds for nine configured LLVM triples plus `wasm32-unknown-unknown`,
  but those are compile-only checks, not non-host execution evidence.
  Cross-compilation does not establish complete concurrency support on every
  target. Background and service APIs are public and tested. Shutdown drains
  and reports unfinished work according to the owning runtime component; it
  does not forcibly terminate arbitrary running native work. `Thread<T>`,
  `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` remain unavailable as
  source APIs. Windows execution evidence covers x86_64 CI only; other
  non-host target builds remain compile-only unless separately recorded.
- Other standard-library, packaging, developer-tooling, decorator, and public
  documentation surfaces are specified ahead of full compiler/runtime
  delivery.

Zirk 1.x explicitly excludes a Zirk WebAssembly target and browser/DOM
integration from the source language. The optional browser-hosted verified-IR
interpreter is developer tooling with a bounded package contract; it does not
add browser APIs, platform directives, or a new deployment target. Its current
execution subset and rejected effects are recorded in the browser interpreter
support matrix, and are intentionally narrower than the native compiler.

Zirk 1.x explicitly excludes public runtime directives or
event loop, standalone `worker`, `async fn`, textual inline assembly, general
`comptime`/`defer`, multiple class inheritance, traditional overloads, Result
`?`, and public ownership/reference-counting semantics.

---

**Previous:** [← Differences from Rust](06-differences-from-rust.md) · **Next:** [ Roadmap](08-roadmap.md)
