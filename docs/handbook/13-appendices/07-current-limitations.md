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

The delivered `Map`/`Set` runtime remains narrower than the generic API
contract: admitted keys, elements, and Map values use typed declared layouts;
supported managed references are traced; and aggregate key/element layouts
without a supported recursive `Hash`/`Equal` capability are rejected by
semantic analysis. Supported combinations include nullable layouts, nested
String/Char tuple snapshots, nullable Decimal, recursive enum payloads, direct
union keys/elements (including nullable and tuple-contained cases), active
enum-variant text snapshots, and mutable reference identity. Some compositions,
including intersections, remain unsupported; the full cross-target equality/lowering/GC
matrix is not closed. Map and Set traversal is insertion-ordered; Map exposes typed
key/value/entry snapshots, Set exposes ordered algebra and `for … in`,
`Map.set` returns `Void`, and `Set.add` returns whether it inserted a new item.
Deque, FIFO Queue, LIFO Stack, and stable minimum-first PriorityQueue have
native/runtime implementations, interpreter parity, typed empty outcomes,
iterator invalidation, and GC tracing. Their required operation and storage
costs are listed in the collection contract chapter. The
native Regex cache is bounded to 128 entries, 16 KiB per source
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
  x86_64 union ABI and parity cases pass under Docker emulation. Focused
  structural/open-template CI filters pass on Linux x86_64, Linux ARM64,
  Windows x86_64, and macOS ARM64 ([structural](https://github.com/ChristianSmith18/zirk-lang/actions/runs/40497897350),
  [generic contract](https://github.com/ChristianSmith18/zirk-lang/actions/runs/40497901428));
  these are not full workspace runs. Dynamic
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
  contract signatures. General alternative-family/container ABI coverage and
  stress coverage at additional specialized managed-container boundaries
  remain incomplete.

- `.zkinit` has a typed project model, `zirk init`/`-y`, project-aware
  entry selection without a required source `fn main`, dependency/dev-dependency
  graphs, typed application globals, and one `requires` capability list. Runtime
  consent is exact-resource and operation-local. Runtime enforcement covers
  environment/secrets, networking, and `std.fs`/`std.path`, including bounded
  directory/link/temp/lock/watch APIs and task-suspending async file/directory
  operations. Browser execution still rejects filesystem effects; Windows
  process APIs remain unavailable. Target-specific filesystem evidence is
  recorded in the status catalog. See the
  [feature status catalog](../../init/ZIRK_FEATURE_STATUS.md) and project
  chapters for the current tested surface and its target-specific evidence.

- `std.environment`, `std.net`, `std.http`, and `std.json` have bundled source
  packages. The native networking preview includes UDP, local sockets,
  interface snapshots, authenticated TLS, HTTP/3/QUIC, and the documented
  HTTP/Fetch boundary. The broader standard library remains partial. Consult
  the [standard-library status catalog](../../init/ZIRK_FEATURE_STATUS.md) for
  status evidence; the [module index](../04-standard-library/19-standard-library-indexes.md)
  and owning chapters document the public APIs.

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
  Deep `clone()` is delivered for `class` and algebraic enum values when
  their reachable fields/payloads implement `Clone`. Enum cloning dispatches
  on the active discriminant and reads only that variant's initialized
  payload. Class cloning is compiler-derived when every field is itself
  `Clone`, preserving internal sharing and cycles
  through a runtime memoization map, and rejected at compile time when the
  field graph reaches a `Pointer<T>`, `Resource`, or other non-`Clone`
  member. Legacy `record` declarations have been removed; enums with a
  non-`Clone` payload still do not satisfy `Clone`. `Dependent<T>` lifetime
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
  `NativeSlice<T>`/`NativeSliceMut<T>` are delivered as validated bounded views;
  see the native-slice chapter for provenance and pointer limits. Indexing is
  dispatched for `String`, `Array<T>`, `List<T>`, and native slices. Text and
  collection chapters document the delivered mutation, slicing, and
  transformation methods. Volatile access and untagged native-union access
  remain unsupported; language unions use tagged layouts. A native-library
  linking manifest and general provenance tracking beyond the recognized
  native-slice construction shape also remain open.
- Some Phase 3 implementation notes below were written for the former
  `record`/`interface` model and are historical. Those declarations were
  removed; current language and pipeline status is in the feature catalog.
  Current type-system gaps include broader union/intersection alternative and
  container combinations and construction of recursively self-referential
  enum payloads. Generic enum dispatch, user-defined generic bodies/defaults,
  substituted trait defaults, and nested generic contract applications now
  have local semantic/lowering/interpreter/native cases; their complete
  the named five-host cases passed in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962). Unsupported lowering combinations must
  diagnose at the documented source or pre-backend stage. See the feature
  catalog for host, target, and verification scope; those named fixtures do not
  establish every theoretical type combination.
- The core tuple, Array/List/Range, String/Char/Regex, and sequence collection
  families are delivered for their scoped contracts. `Deque`, `Queue`, `Stack`,
  and `PriorityQueue` are implemented with native/interpreter parity. Map/Set
  still reject aggregate key/element combinations without canonical
  `Hash`/`Equal` derivation. Named equality/lowering/GC cases passed on all five
  hosts in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962); the exhaustive recursive capability matrix remains
  incomplete. The `std.collections` source facade is importable; its
  current exports and the distinction between facade APIs and compiler/runtime
  implementations are documented in the collection chapter. See the feature
  catalog for exact supported combinations and host evidence.
- Locale-aware formatting is implemented for civil `Date`, `Time`, and
  `DateTime` values. `Instant` has checked factories, parsing, formatting,
  ordering and exact-duration arithmetic. `TimeZone` and `ZonedDateTime` have
  partial language APIs for pinned rule parsing, projection, instant-preserving
  conversion, and explicit local-time resolution. `Period` construction,
  components and ISO operations, plus the unified clock/calendar `TimeShift`,
  are implemented locally; direct `Date`/`DateTime ± Period`, localized
  parsing, and the complete
  named cross-host temporal cases passed in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962). See the temporal chapters and
  feature catalog for the tested surface.
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
  freedom. `RwLock`, `Semaphore`, `Barrier`, `Once`, and `ConditionVariable` are
  not available as source APIs. Native `Thread.run(fn)` is available on supported 64-bit targets
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
  `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, and `ConditionVariable` remain
  unavailable as source APIs. Windows execution evidence covers x86_64 CI only; other
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
