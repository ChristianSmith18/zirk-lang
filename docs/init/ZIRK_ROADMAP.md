# Zirk — Construction roadmap

This roadmap translates the five normative specs — which describe a mature Zirk
— into a real order of construction. Each phase has a verifiable goal: you do
not move to the next one until the current goal actually runs, not merely "is
almost there".

No real language was ever built by implementing its full spec in one pass. This
document is the discipline that keeps this one from trying.

For the concrete, per-feature, per-pipeline-stage status, see
[`docs/init/ZIRK_FEATURE_STATUS.md`](./ZIRK_FEATURE_STATUS.md). This roadmap
keeps the phase narrative; the catalog is the authoritative feature matrix.

---

## Phase 0 — Decisions before writing code

This is not a coding phase but a decision phase whose consequences cascade into
everything else. Write them as short ADRs, one per decision, in
`docs/decisions/`.

- [x] **Memory strategy.** The spec promises automatic memory without exposing
  ownership (`RUNTIME_SPEC.md` section 9) and the absence of data races on
  shared globals (`LANGUAGE_SPEC.md` section 2). Define explicitly: generational
  GC, reference counting with cycle detection, regions, or a hybrid? This is the
  highest-leverage decision of the project — it determines how closures,
  `parallel for` and `Resource<E>` behave later on.
- [x] **Workspace layout.** Proposed crates: `zirk-lexer`, `zirk-parser`,
  `zirk-ast`, `zirk-sema` (name resolution + type checker), `zirk-ir`,
  `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`.
- [x] **LLVM sanity check.** Before writing a line of Zirk, confirm that
  `inkwell` generates, links and runs a trivial native binary from pure Rust.
  This step depends on nothing about the language — it validates that the
  toolchain works on your machine and in CI before building on top of it.
- [x] **Diagnostic format.** Implement the format of `COMPILER_SPEC.md`
  section 8 as its own crate from day one (`zirk-diagnostics`), even if only the
  lexer uses it at first. Migrating the format after five other layers already
  use it is far more expensive than starting right.

**Phase output:** ADRs written, workspace created, `cargo build` compiles a
binary that invokes LLVM and produces a "hello world" executable written
directly in Rust (with no Zirk parser yet).

**Status: complete.** Verified in CI on Linux (x86_64, aarch64), macOS aarch64
and Windows x86_64. The decisions live in `docs/decisions/` as ADR-001 to
ADR-006.

---

## Phase 1 — Zirk 0.1: minimal end-to-end pipeline

**Status: complete.** The archived `fase-1-pipeline-minimo` change and CI
closeout provide the implementation evidence.

**Goal:** `zirk run` over a `.zrk` containing
`import { stdout } from std.io;` and
`fn main(): Void { stdout.println("..."); }` really compiles through LLVM and
runs as a native binary.

Language subset: `fn main`, `Void`, `String`, `Int32`, `Boolean`, literals,
basic arithmetic, `if`/`else`, `mut`/`inmut` variables, and a minimal hardcoded
`println` (not the full stdlib yet).

Explicitly out: generics, classes, `Result`, concurrency, decorators,
multi-file modules, `.zkinit`.

**Phase output:** a real `.zrk`, with real syntax from the spec, compiling to a
real native binary. This is the milestone that validates that the whole
architecture (lexer → parser → types → IR → LLVM → binary) works end to end —
everything that follows extends this backbone rather than building a new one.

---

## Phase 2 — Core language surface

**Status: complete for its scoped delivery.** Closures are callable locally,
but annotated callable types and escaping closures are explicitly owned by
Phase 4d rather than silently treated as complete here.

- Complete control flow: `for`, `for ... in`, `while`, `loop`, `break`,
  `continue`, `if` as an expression.
- Complete the phase-local function surface: optional, named and variadic
  parameters, default values, and locally callable closures/lambdas.
- `match` with basic exhaustiveness (over simple enums).
- Nullability: `T?`, `?.`, `??`.
- Modules within a single crate: basic `share`/`import`, without `.zkinit` yet.

**Output:** programs with several functions, real control flow and closures —
still without classes or concurrency.

---

## Phase 3 — Objects and the type system

> **Historical implementation notes:** this phase narrative preserves decisions
> and work from the former `record`/`interface` model. Those declarations were
> later removed. References below to record equality, dispatch, or cloning
> describe historical work, not current language support. Use
> `ZIRK_FEATURE_STATUS.md` and current limitations for today's delivery state.

**Status: complete for its scoped delivery.** The archived
`fase-3-objects-and-type-system` change recorded remaining pipeline
limitations for generic contracts/enums, abstract dispatch, value-type
contract dispatch, and derived structural equality. `fase-3-abstract-dispatch`
(merged) closed the abstract-dispatch gap: a user-declared `abstract class`
now dispatches dynamically through its concrete adopters, reusing the vtable
mechanism the compiler's own `Throwable` hierarchy already proved in
production. Doing so surfaced and fixed three latent gaps in the general
class-declaration path that had never been exercised for a user abstract
class (method `overridden` flags, method-index seeding, `implements`-aware
hierarchy ordering) and a pre-existing diagnostic bug (`MISSING_OVERRIDE`
reported instead of `MISSING_IMPLEMENTATION`). `fase-3-structural-equality`
(merged) closed the derived-equality gap: `==`/`!=` on a `record`
now lowers to a field-by-field, short-circuiting comparison —
recursing into a nested `record` field and dispatching a
`class`-typed field to its own existing equality rule (`_equals` if
declared, otherwise `is`) — reusing the same short-circuit block shape `&&`
already builds, generalized from two operands to however many fields a type
declares. A residual field type that fits none of these shapes (e.g. `T?`)
keeps its own `NOT_LOWERED` diagnostic, now scoped to that field rather than
the whole comparison. `fase-3-generic-enums` (merged) delivered lowering for
a flat single- or multi-type-parameter user generic enum (`enum Either<L,
R> { ... }`), reusing `Result<T,E>`'s own already-generic specialization
mechanism — removing both the checker's instantiation gate and a second,
previously undocumented declaration-time gate (`declare_enum` minted its own
type parameters through the generic-function-only `enter_type_params` path,
unconditionally rejecting every user generic enum at declaration regardless
of the instantiation gate). Two deeper gaps were found and reported rather
than fixed, judged out of that change's own "verify an existing mechanism"
scope — both since closed. `fase-3-generic-substitution-recursion` (merged)
fixed `infer_type_params`/`substitute` to recurse into a nested generic
instantiation's own type arguments the same way `substitute_type` already
did (`Bar<Baz<T>>` now resolves), a gap shared by every generic
function/method call in the compiler, not enum-specific — the implementing
agent also found and fixed the identical shallow-substitution bug in
`specialize_enum`'s own local substitution closure, flagged as outside the
proposal's stated impact but necessary to avoid turning a clean type error
into an internal-compiler-error panic for the exact scenario the change
targeted. `fase-3-recursive-enums` (merged) split `declare_enum` into
`register_enum`/`declare_enum_variants`, mirroring classes' own
`register_class`/`declare_class_members` split — an enum and a class (or two
enums) can now reference each other regardless of declaration order. That
change's own verification found its design's prediction wrong: a genuinely
self-referential enum field is not a `specialize_enum` memoization question —
every enum lowers to an inline-flattened struct with no indirection
anywhere in the pipeline, so lifting the old rejection crashed the compiler
with a real, confirmed stack overflow. Fixed with `reject_unindirected_enum_cycles`,
a graph search over every declared enum's "variant field names enum" edges
that rejects the specific unindirected shape with a clean diagnostic naming
the cycle, before it can reach IR lowering. Actually constructing and using
a self-referential enum (`enum IntList { Nil, Cons(head: Int32, tail:
IntList) }`) still needs automatic heap indirection ("boxing") — a new
`IrType` case, a runtime allocation kind, and GC integration — none of which
exists yet; tracked as its own future change, not invented here.
`fase-3-value-type-contract-dispatch` (merged) closed the last large open
design question Phase 3 left behind: a `record` implementing a contract
now dispatches correctly through a contract-typed reference. The
design converts a value to a contract-typed reference by boxing it into an
ordinary, collector-tracked heap allocation carrying a real descriptor —
built by reusing the exact same object-layout construction a `class`
already goes through, not a second, divergent builder — so `CallContract`'s
own existing dispatch needed zero changes, exactly as design predicted.
Implementation found one real bug design's own reasoning had implicitly
missed: a value type's own method is compiled expecting `this` by value,
while `CallContract` always calls through a pointer, so pointing a contract
table straight at the value's own method body silently miscompiled. Fixed
with a small per-method unboxing thunk, synthesized only for a value
type's own contract-method bodies (a trait's inherited default already
expects a pointer receiver, so needs none). Verified against the scenario
that specifically rules out a static-monomorphization alternative: two
different concrete `record` adopters dispatched correctly through the same
unchanged, non-generic call site. Generic method calls through parameterized
contracts now specialize their class implementations for in-program native
and interpreter dispatch; package-boundary specialization metadata remains.

- `class`, `construct`, visibility (`public`/`private`/`protected`), single
  inheritance, interfaces, traits.
- Constructor sugar: member-modified constructor parameters promote fields;
  classes without a written constructor admit zero-argument construction and
  implicit subclass construction delegates to an accessible zero-argument base.
- Generics with `from` (constraints).
- Algebraic enums and transparent type aliases.
- Casts (`as`, `<T>`, `unsafe` casts).

**Output:** the object-oriented subset of the spec working, including basic
generics.

The separate native type-algebra work supplies recursive n-ary unions and
intersections, canonical singleton values and constant evaluation, structural
type expressions, generic type operators, and flow narrowing. Focused
macOS ARM64 native/interpreter cases now cover discriminated structural unions,
a structural/scalar union through a function, identity-preserving structural
intersections, and captured structural payloads. One-hole open template
inference also resolves a key such as `"age"` from `"ageChanged"` and uses
`T[K]` to check the callback. The full structural alternative/container ABI,
some mixed nominal/scalar combinations, cross-target runtime ABI execution,
and additional GC-storage boundaries remain open. See the current
[feature-status](ZIRK_FEATURE_STATUS.md) and
[limitations](../handbook/13-appendices/07-current-limitations.md) before
assigning additional delivery work.

---

## Phase 3b — Complete scalars, conversions and text

**Status: complete for its scoped delivery.** The fractional default is now the
exact base-ten `Float` (128-bit coefficient + scale, no `NaN`, no infinity);
the IEEE 754 binary family is `Float16`–`Float128` (`Float` =
`Float64`), written with an `f` literal suffix. `Float128` formatting
truncates to `Float64` first, which can lose precision for values not
exactly representable in `f64`. Windows `Float128` arithmetic remains an
implementation limitation.

Phase 1 implemented one integer width, one Boolean and an opaque `String`.
Everything else the type system promises about scalars was never assigned a
phase. This one owns it:

- The remaining integer widths: `Int8`, `Int16`, `Int64`, `Int128` and the whole
  `UInt*` family, with fixed width and checked arithmetic.
- The exact base-ten `Float` (the fractional default), and the binary floating
  family `Float16`, `Float32`, `Float64`, `Float128`
  (`Float` = `Float64`) with explicit infinities and no valid `NaN`
  — an operation that would produce one is a controlled error.
- `Char` as exactly one Unicode grapheme, which may span several code points.
- Final-result scalar conversion: `T(expression)` evaluates the complete
  expression first and converts only its result. Integer `/` produces exact
  `Decimal`, so `Float64(3 / 4)` converts that completed quotient.
- Bitwise and shift operators, levels 7 to 10 of the operator table.
- String interpolation, which needs the `to_string()` contract Phase 3 defines.

They arrive together because numeric conversions depend on the complete scalar
family, while interpolation needs Phase 3's contracts.

**Output:** the complete scalar surface of the spec — every numeric width, real
`Char`, and the conversion rules that connect them.

---

## Phase 4 — Failure, callable completion, and memory

**Status: complete for its original scoped delivery.** All sub-phases (4a–4e)
parse, type-check, lower, and run end-to-end for their assigned features. Native
tagged unions are now implemented separately for selected type families; this
work does not imply complete alternative-family, target-runtime, or GC-boundary
coverage. Remaining follow-up work includes volatile access and general slice
provenance beyond the direct shape. Enums with cloneable payloads already
implement `Clone`; legacy record declarations were removed.

### Phase 4a — Expected errors

**Status: complete for its scoped delivery.** Implements `Result<T,E>` and
exhaustive handling. Standalone Result statements discard success and propagate
failure as an exception; `_ = expression` deliberately discards either outcome.

### Phase 4b — Exceptions

**Status: complete for its scoped delivery.** Implements explicit
`throws`/`try`/`catch`/`finally`, typed catch dispatch, rethrow, `fatalError`,
and throwable runtime support. `fase-4b-excepciones` closed the implicit-native-error gap:
`ArithmeticOverflowError` and `InvalidCastError` are concrete `RuntimeError`
subclasses and are catchable without a `throws` declaration, joining
division-by-zero, out-of-range shift, negative string-repeat, and `NaN`
production. `Throwable.suppressed()`, lazy `Throwable.stack_trace()`, and
deep thrown-object immutability (caught bindings are `inmut::strict`) are
now delivered, with CLI fixtures for each.

### Phase 4c — Deterministic resources

**Status: complete for its scoped delivery.** Single-resource `match with` and
grouped `match ... with` both parse, type-check, lower, and run end-to-end.
Acquisition runs left-to-right and cleanup runs right-to-left, with earlier
resources closed when a later acquisition fails. `transfer(r)` works for
values implementing the `TransferableResource` contract, and use-after-transfer
is rejected at compile time. `ResourceFailure<BodyError,CloseError>` is
registered and lowered in the error merge path, and cancellation-aware close
dispatch is wired.

### Phase 4d — Callable and binding completion

**Status: complete for its scoped delivery.**

- [x] Parse and type `Function(P...) => R` and preferred alias `Fn(P...) => R`
  in parameters, returns, attributes, generic arguments, and local
  annotations.
- [x] Same-type multiple declarations with independent defaults and
  exact-arity simultaneous assignment (`left, right = right, left`) are
  delivered.
- [x] The boxed-callable mechanism (`MakeCallable` / `CallCallable`) is
  implemented end-to-end: a captured lambda can be stored in a field, passed
  through locals, returned from a function, and called later. Capture blocks
  are heap-allocated, GC-tracked objects described by a runtime descriptor.
  `.clone()` on a callable is supported when every captured value is `Clone`.
  Named functions and capture-less lambdas freely satisfy any structurally
  compatible `Fn(...) => R` position, and `is` compares callable values by
  identity.

**Output:** named functions, capture-less lambdas, and captured closures
interoperate with `Fn(...) => R` anywhere; `left, right = right, left` and
`mut a, b: String;` work. Callable polymorphism and deep cloning of capture
blocks are delivered.

### Phase 4e — Managed memory and unsafe boundaries

**Status: complete for its scoped delivery. ADR-003 closed (non-moving mark-sweep);
the real collector, `inmut::strict` field checking
(`fase-4e-inmut-strict-proyeccion`), the `unsafe`/`Pointer<T>`/`extern`
core (`fase-4e-unsafe-pointer-extern`), `Weak<T>` (`fase-4e-weak`), deep
`clone()` for reference graphs (`fase-4e-clone`), the transactional
unsafe journal/rollback (`fase-4e-unsafe-journal`),
`NativeSlice<T>`/`NativeSliceMut<T>` (`fase-4e-native-slice`, which also
delivered general `expr[index]` grammar), `Dependent<T>` lifetime/escape
analysis, and `Pin<T>` automatic pin/unpin are delivered.**

- [x] Deliver the strategy chosen by the Phase 0 memory ADR without exposing it as
  public ownership syntax — **delivered** (`fase-4e-colector-mark-sweep`):
  `docs/decisions/ADR-003-memoria.md` closed on non-moving mark-sweep, with
  root enumeration via a function-granularity shadow stack (every function's
  managed-reference-typed slots — named locals and compiler-synthesized
  spills of otherwise-transient SSA values, design D4 — are pushed as roots
  on entry and popped before every `return`) and cooperative,
  allocation-triggered, single-threaded collection. `zirk_rt_alloc` stops
  being "never frees"; the object header grows from one word to three
  (dispatch descriptor unchanged, plus an intrusive next-allocation link
  with the mark bit in its low bit, plus the allocation's own size) exactly
  as `ADR-012` anticipated. Verified against real allocation-pressure
  programs, including the specific hazard the design work surfaced before
  any code was written: a managed-reference value that lives only in an
  LLVM SSA register between two constructor arguments of one call, which a
  naïve shadow stack cannot see and would collect out from under the first
  argument — closed by spilling every such value to a synthetic root slot
  the instant it is produced.
- [x] Implement safe/weak/dependent references, deep clone graph semantics and
  automatic bounded native pinning — **delivered** (`fase-4e-weak`,
  `fase-4e-clone`): `Weak<T>` delivered, restricted to reference-typed
  referents, with `Weak.from`, `.upgrade(): T?`, and `.is_alive: Boolean`.
  Deep `clone()` is compiler-derived for a `class` whose every field is
  itself `Clone`, preserving internal sharing and cycles within the new
  graph via a runtime memoization map. `Dependent<T>` lifetime/escape analysis
  rejects returns, field stores, closure captures, and non-dependent parameter
  passing; valid local use runs end-to-end. `Pin<T>` supports construction with
  `Pin(obj)`, automatic unpin for field/method access, and rejects reassignment
  of the pinned variable. Algebraic enums implement `Clone` when their payload
  types do; legacy record declarations were removed.
- [x] Implement `inmut::strict` with reachable-alias analysis — **delivered**:
  a field declared `inmut::strict` is unwritable through any projection
  regardless of the container's mutability, and mutating method calls with an
  `inmut::strict` receiver are rejected.
- [x] Implement `unsafe {}`, `Pointer<T>`, native slices, and
  compiler-enforced memory-safety boundaries — **delivered**
  (`fase-4e-unsafe-pointer-extern`, `fase-4e-native-slice`): `unsafe fn`/
  `unsafe {}`/`commit {}` parse and are context-checked; `Pointer<T>` (an
  ABI-stable element-type subset) supports construction from an
  addressable local/field, read/write/offset/offset-bytes/cast, with a
  conservative escape rule (cannot be returned, stored in a field, or
  captured); `extern "C" fn` declares and calls a native function
  (`docs/decisions/ADR-015-declaracion-extern.md` closed the syntax this
  needed, which no prior spec had decided) under a narrow ABI-safe type
  surface, resolved by the system linker with no new library-linking
  manifest. `NativeSlice<T>`/`NativeSliceMut<T>` delivered
  (`fase-4e-native-slice`): validated bounded views constructed only via
  `pointer.as_slice(length)`/`.as_slice_mut(length)` (both `unsafe`,
  returning `Result<view, NativeError>`), checking nullability, alignment,
  extent, and — for the syntactically direct `Pointer.from(place).as_slice(n)`
  shape — known extent against the real underlying storage; using an
  already-constructed view (indexing, `.length`, `.is_empty`) needs no
  `unsafe`, with bounds checks active on every index. The generalized
  `Pointer<T>` escape rule now also rejects a view returned, stored in a
  field, or captured. **This change also discovered and delivered a real
  grammar gap**: `expr[index]` did not exist anywhere in the compiler
  before it (lexed tokens, no parser production, no AST node) — added as
  a genuine new postfix expression (`Expr::Index`), usable as a read or,
  through a receiver-type-keyed dispatch table (today: `NativeSlice<T>`
  read-only, `NativeSliceMut<T>` read/write), as an assignment target,
  deliberately left open for Phase 7's `Array<T>`/`List<T>` to register
  their own support later without further grammar changes. **Not**
  covered: `.read_volatile()`/`.write_volatile()`, untagged native-union
  access (distinct from Zirk's tagged type unions), and general provenance tracking for a view's
  known extent
  beyond the one syntactic shape recognized so far.
- [x] Implement transactional write journals and rollback for managed/validated
  ranges, followed by explicit irreversible `commit` effects — **delivered**
  (`fase-4e-unsafe-journal`, D5/D6 of `fase-4e-unsafe-pointer-extern`'s own
  `design.md`, implemented as its own follow-up change once cut from that
  one's original scope): `unsafe {}` journals every managed write to state
  declared outside the block before executing it, commits durably on
  normal exit, and rolls back every recorded write in reverse order when
  an exception becomes pending before that exit; `commit {}` durably
  publishes the enclosing block's journal at its own entry, before running
  its own body, and further writes after that point are no longer
  journaled (a commit boundary makes prior writes irreversible, matching
  the spec). A `try`/`catch` nested inside an `unsafe {}` block that
  handles the exception locally correctly does not trigger a rollback — the
  rollback decision is made by walking `try`/`unsafe` frames in true
  lexical nesting order, not just checking the innermost `unsafe` block in
  isolation. Early `return`/`break`/`continue` out of an `unsafe {}` block
  now also rolls the active journal back before the jump, closing the
  `phase-4e-pending-closeout` follow-up.
- [ ] Finish `Dependent<T>` lifetime/escape analysis, `Pin<T>` automatic bounded
  native pinning, and throwable cleanup interactions (suppressed failures,
  fully materialized traces, deep thrown-object immutability) that require the
  complete memory model. **Partial; `transfer(r)` and surface types are in,
  full analysis and runtime semantics remain.**

**Output:** complete failure and memory behavior, with no use-after-free,
uncontrolled null dereference, silent cleanup loss, or undefined behavior in
safe code, verified end to end.

---

## Phase 5 — Concurrency and parallelism (the hardest and least trodden part)

This is the phase of highest technical risk in the project — build it in
sub-steps, not in one go. Each is a focused OpenSpec change against the
`async-runtime-core` / `zirk-structured-concurrency` specs (the full design was
recorded and archived as `fase-5-async-core`). `ADR-017` fixes the suspension
model: stackful coroutines on a single-threaded cooperative executor, no
function coloring.

0. **Executor infrastructure** — `fase-5-executor-core` (**runtime delivered**):
   the cooperative executor, scheduler control block, stackful-coroutine
   suspend/resume, monotonic timer, per-task garbage-collection root chains, and
   `zirk_rt_run_main` running `main` as the executor's root scheduler task.
1. `concurrent { }`, dynamic `spawn`, `Job<T>`, cleanup edges, cooperative
   cancellation, and `Timer.sleep` / `after` / `every`
   (`concurrent-blocks-and-timers`, **delivered**): the block hoists its
   bindings and orders branches by a Rule B dataflow DAG; a branch's
   cancellation or unhandled exception unwinds natively through its own stack
   and the scope aggregates the failure; `main` runs in an implicit scope.
2. CPU-bound `parallel { }` regions on a real multicore worker pool
   (`parallel-cpu-regions`, **delivered**): a separate worker pool and a
   multi-threaded stop-the-world safepoint collector, alongside
   `concurrent`/`spawn`'s single-threaded executor. Ordered `map`/`filter`,
   `for_each`, `Parallel.each`, `sum`, and associative `reduce` run through
   the worker path; `reduce_ordered` deliberately keeps sequential grouping.
3. Typed bounded/unbounded `Channel<T>` with closure and backpressure
   (`typed-channels`, delivered; `drain` and `select` remain follow-ups).
4. Completion APIs and safety boundaries (`concurrency-completion`; scoped
   delivery completed 14 October 2026). The source surface now includes
   bounded completion and collection combinators, managed `Thread.run`,
   supervised background work and services, scalar source `Atomic<T>`, and
   `Mutex<T>.with`. The Linux x86_64/aarch64, macOS aarch64, and Windows
   x86_64 CI matrix passed; the Windows job executed the focused GC/thread
   lifecycle checks and full workspace tests. `Thread<T>`, `RwLock<T>`,
   `Semaphore`, `Barrier`, and `Once<T>` remain explicitly gated with
   diagnostics. Cancellation is cooperative, native work is not forcibly
   terminated, and current `Transfer`/`Share` and effect rules do not establish
   general end-to-end race freedom. Other target claims remain limited to
   their recorded compile/build evidence.

`task`, `await`, `Task<T>`, task scopes, `select`, and cancellation shields are
removed constructs. Their parser diagnostic names the applicable replacement.

**Output:** the Phase 5 scoped delivery works end to end: structured
`concurrent`/`spawn`/`Job<T>`/`Timer`, CPU `parallel` regions, typed channels,
completion and collection combinators, managed `Thread.run`, background and
service supervision, scalar `Atomic<T>`, and `Mutex<T>.with`. This does not
establish general data-race freedom or complete `Transfer`/`Share` enforcement.
`Thread<T>`, `RwLock`, `Semaphore`, `Barrier`, and `Once<T>` remain gated and
unavailable as source APIs; `task`, `await`, and `select` remain removed.
See the feature-status catalog and archived `concurrency-completion` evidence
for target-specific validation and limitations.

---

## Phase 6 — Project system and CLI

- Delivered slice: `.zkinit` discovery for `application` and `package`,
  logical package identity, quoted local imports, unquoted package imports,
  `share mod` package roots, local/vendored traversal and pre-frontend lock
  metadata validation. Registry/Git acquisition and publication remain work
  for the package phase.
- A typed declarative `.zkinit` with `project`, source roots, targets, normal/dev dependencies, application globals, and one finite `requires` list.
- Applications select `entry`; packages select importable `root`. The selected file executes top-level statements, so source `fn main` is not required.
- Exact user consent is stored outside the repository and checked at each protected operation; no permission phase or application `permissions` block exists.
- CLI includes `zirk init` prompts and `zirk init -y`, project-aware `run`, dependency resolution/migration, and consent inspection/revocation.
- Local/vendored package graphs and the current native network/environment APIs are implemented. `deliver-std-fs-core` and `complete-std-fs-advanced` deliver exact-resource permissions plus bounded core and advanced filesystem APIs; registry/Git acquisition, package publication, and process permission APIs remain open.

**Output:** real multi-file projects, with a manifest and cross-compilation.

---

## Phase 7 — Stdlib

Order suggested by real dependency, not by the order they appear in the spec:

`std.io` (synchronous standard streams delivered; async and terminal work remains) → `std.collections` → `std.fs`/`std.path` →
`std.time` → `std.process` → `std.thread`/`std.sync` (wrapping
Phase 5) → `std.json` → `std.net`/`std.http` (the largest of them all) →
`std.crypto` → `std.testing` (`@test`/`@e2e`/`@bench`) → `std.reflect` →
`std.system`.

`std.environment`, `std.net`, `std.http`, and `std.json` have compiler-bundled
source packages and native runtime previews; their delivery changes are
archived, not active. These previews cover permission-
checked environment reads and typed parsing, address/DNS/TCP/TLS operations,
and bounded HTTP/1.1 clients, routing, URL-encoded forms, multipart parsing and
in-memory request building, and HTTP/1.1 plus bounded multiplexed HTTP/2 over
accepted TLS connections. HTTP handlers can return bounded streaming responses
through `HTTPResponse.from_stream`; finite server scopes can close their
listener after the configured bound. Compiled-Zirk tests cover HTTP/1.1 and
HTTP/2 chunks, and runtime tests verify capture-root cleanup after disconnect,
task cancellation, and SIGPIPE-safe peer shutdown.
The opt-in `HTTP.get_cached`/`HTTP.request_cached` preview also stores bounded,
fresh GET/HEAD/QUERY responses by approved project and manifest, matches `Vary`,
and supports ETag/Last-Modified conditional revalidation. It is process-local
and is not owned by `HTTPClient`. The reusable client preview owns a bounded
per-origin HTTP/1.1 and HTTPS pool, optional cookies, and connections using
system or caller-supplied DER roots. An `HTTPClient` may be shared across tasks
in the same verified executor and project/manifest identity. HTTP/1.1 requests
use a cooperative bounded queue; HTTP/2 allows up to eight simultaneous
streams per connection. `close()` waits for active requests and reopens admission if cooperatively cancelled before draining; executor shutdown releases any remaining client handles. The
client advertises and incrementally decodes `br`, `gzip`, and `deflate` with
16 MiB compressed/expanded bounds and a 1000:1 expansion ceiling; buffered
and streaming server responses negotiate and compress the same codings.
Request decoding and the cumulative codec CPU budget are implemented. Explicit
HTTP and HTTPS forward proxies support absolute-form forwarding and HTTPS
CONNECT with separately authorized proxy/destination origins and independent
TLS validation. Basic proxy credentials flow through `SecretString` and are
allowed for loopback HTTP proxies and HTTPS proxies; remote cleartext HTTP
proxy authentication fails closed.
Reusable-client redirects are opt-in (`Never`, `SameOrigin`, or `Follow`);
303 drops the body and changes to GET except for HEAD, other supported statuses
replay the method/body, each hop rechecks authority, and cross-origin hops strip
known credentials plus custom headers marked with `HTTPHeaders.add_sensitive`/
`set_sensitive`. Stream producers restart deterministically from index zero.
Configurable timeout values are available across direct, proxy, and custom-root
client factories. TLS identity checks derive SNI and certificate-name validation from the URL and use either platform or explicit custom roots; no custom verifier, hostname override, or public mutual-TLS client credential API is exposed. Cache state is exposed through the opt-in module-level API rather than `HTTPClient`.
Ordinary buffered, cached, reusable-client, secret-header, and streaming
requests reject exact `CONNECT` with `HTTPError.UnsupportedMethod` before
permission lookup; the proxy and WebSocket APIs own their tunnel flows.
Safe-method retries are implemented. A compiled-Zirk E2E also verifies
that canceling an active cooperative HTTP/1.1 handler does not send a late
response and releases the accepted listener on macOS aarch64 and Linux ARM64.
Generic response-producer slow-consumer stress beyond the existing bounded-demand
concurrent admission across separate server connections and
debugger/snapshot/test-report secret sinks remain follow-up work. Handler
cancellation is cooperative by design: CPU-only code is not preempted. The full
native target matrix is now green on macOS ARM64, Linux x86_64, Linux ARM64, and
Windows x86_64 (run 39359107965). Generic response-producer backpressure tests cover HTTP/1.1 and TLS/HTTP/2 on Unix and TLS/HTTP/2 on Windows; SSE slow-consumer transport tests cover identity and gzip over HTTP/1.1 and TLS/HTTP/2 on macOS and Linux ARM64. Configurable server heartbeats and automatic client reconnect are implemented. A demand-driven SSE client is also
available and validated through compiled Zirk code; its parser retains bounded
state and closes the response on EOF, cancellation, or error. The feature status
catalog and module handbook chapters track the verified subset and current gaps.

Current target evidence (2026-10-18): the full compiled-Zirk CLI E2E suite passes
155/155 on native macOS ARM64 and 155/155 on Linux ARM64 Docker on the
immediately preceding source revision. The subsequent typed `Env.get_or<T>`
evidence is historical: that API was removed by `contextual-result-value-projection`
and does not validate the current source. The focused signed approval E2E passes
after rebuilding the runtime static archive, and the fail-closed environment
E2E passes. A focused parity case confirms Result projection across native LLVM
and the browser interpreter. Linux/Windows native parity remains unverified;
the full current-source `cargo test -p zirk-cli` run was interrupted after
producing no output for several minutes and remains inconclusive.
The earlier Linux run was rebuilt with the current runtime archive and included
the QUERY wildcard rejection and structured runtime-diagnostic cases. The combined native
macOS runtime/runtime-core package run passes 1,019 tests with 12 ignored across
28 suites. On Linux ARM64 Docker, runtime passes 498/3 ignored and runtime-core
163/1 ignored; all package integration targets and doc tests exit successfully,
including 309 passing optimization-benchmark tests. The reusable-client redirect
matrix checks JSON, bytes, form, multipart, and deterministic stream replay across
301/302/307/308, 303 handling, cross-origin credential stripping, and SameOrigin
behavior. The updated IPv4/IPv6 IANA hostname-address policy matrix also passes
on Linux x86_64 Docker emulation. Windows runtime-core and frontend CLI test
targets compile through `cargo xwin`, but Windows execution remains unverified.
Docker execution does not count as native Linux x86_64 evidence. The full native
target gate remains open.
The latest native macOS ARM64 full-workspace rerun passes 2,791 tests with 16
ignored across 75 suites; this count supersedes earlier same-day workspace
totals in the task log. Windows remains compile-only, and native Linux x86_64
execution remains unverified.

The current Linux x86_64 Docker-emulated run passes the CLI end-to-end suite
155/155 (834.21 seconds), `zirk-runtime` 498 tests with 3 ignored, and
`zirk-runtime-core` 164 tests with 1 ignored. This is an emulated target run;
native Linux x86_64 execution and Windows execution remain unverified.

Two families in this phase are **not** ordinary library objects, and the
distinction changes who implements them:

- The **temporal family** — `Date`, `Time`, `DateTime`, `Instant`,
  `ZonedDateTime`, `TimeZone`, `Duration`, `Period` and `TimeShift` — are compiler-known
  native immutable values with their own literals, operators and type rules.
  `Duration` (literals, arithmetic and printing) is already implemented in
  `array-list-tuple-duration-regex`, and the civil subset — `Date`, `Time`
  and `DateTime`, with validated constructors, component properties, ISO
  8601 `to_string()`, civil arithmetic and the host-clock `today`/`now_*`
  statics — is implemented in `date-and-time-types`. `Instant` has a partial
  language surface for RFC 3339 parsing, checked Unix-unit factories/properties,
  UTC rendering, ordering and exact `Duration` arithmetic. Explicit-locale
  formatting is available for civil values; `DateTime.format_locale_in_zone`
  supports pinned IANA/fixed-offset output with explicit DST policies and
  ICU4X 2.4.0 / CLDR 49. Partial `TimeZone`/`ZonedDateTime` language APIs now
  cover parsing, instant projection/reprojection, local projection, instant
  comparison, and explicit local-time resolution. `Period` construction/ISO parsing and tagged `TimeShift` clock/calendar
  application now have local native/interpreter evidence. Localized parsing
  and cross-host evidence remain pending. `std.time`
  re-exports all nine compiler-owned temporal type aliases.
- The **collection family** — `Array<T>`, `List<T>`, `Map<K,V>`, `Set<T>`,
  `Deque<T>`, `Queue<T>`, `Stack<T>`, and `PriorityQueue<T>` — are native
  reference types. Array/List, ordered Map/Set, and Deque/FIFO/LIFO/priority
  families have constructors, typed runtime storage, GC tracing, native and
  interpreter behavior, and focused parity tests. Deque end operations and
  Queue/Stack operations are amortized O(1); PriorityQueue uses a stable min-heap
  with O(log n) enqueue/dequeue and O(n log n) ordered iteration. Map/Set
  support scalar and reference keys/elements, nullable layouts, tuples, eligible
  payload enums, nullable Decimal, selected union compositions, and recursive
  String/Char content snapshots. Mutable/dynamic structural shapes and some
  nested generic compositions remain rejected; the verified admission matrix
  is in `openspec/changes/complete-data-types-and-standard-library-coverage/collection-support-matrix.md`.
  The compiler-owned collection types are re-exported by the importable
  `std.collections` facade.

`String` grapheme indexing, `s[i] = c` writes, `[start:end:step]` slicing and
the search/trim/`substring` methods are delivered in
`array-list-tuple-duration-regex` — the moment ADR-005's boundary was designed
to protect. `String.split` returns `List<String>` and lands with `List<T>`;
`Range<T>` now supports readable `start`/`end`/`step`, colon steps, inferred
descending direction, slicing, and expansion in `Array`/`List` literals and
constructors; `.reverse()` is intentionally not a range-builder API.

`Regex` core support also lands here — the `re'pattern'` literal, the
`Regex.matches(text): Boolean`, `Regex.find(text): Regex.Match?` and
`Regex.replace(text, replacement): String` operations are delivered in
`array-list-tuple-duration-regex`, including capture-group access on
`Regex.Match`, splitting, and iteration over `Regex.Match` results. The
`re'...'` literal still cannot be used directly as a `match` pattern.

**Output:** real non-trivial applications (a CLI, a simple backend) writable in
Zirk using only the stdlib.

---

## 2026-10-20 native standard-library verification

The baseline `std.environment`, `std.net`, and `std.http` preview passed the
full serial workspace test suite in GitHub Actions run [39359107965](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39359107965),
at compiler revision `b28daa73379a527159c9c303509632bd3ecf413a`, on macOS
ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. Each target passed
155/155 compiled-Zirk CLI E2E tests; runtime and runtime-core suites also
passed. The active `complete-stdlib-networking-and-http` change implements
bounded UDP and local IPC, interface snapshots, extended TLS, optional HTTP/3
client/server and RFC 9220 WebSocket paths, and browser Fetch injection. Its
current working-tree lifecycle changes are verified locally on macOS ARM64,
but still need the complete native CI matrix and cross-target independent-peer
evidence. Browser sockets and listeners remain excluded. The wider Phase 7
standard-library surface remains incomplete.

## Phase 7b — Functional style and generators

- Generators: `fn gen` with `yield`, preserving locals between yield points and
  implementing both `Iterator<T>` and `Iterable<T>`.
- The pipe operator `|>`.
- `map`, `filter` and `reduce` over collections, without mutating the source.

It lands after the collections it operates on and after the iteration contracts
Phase 3 defines. It is numbered `7b` rather than taking a number of its own so
packaging, developer experience and metaprogramming keep the numbers they have
had since the roadmap was written.

**Output:** the functional style of `LANGUAGE_SPEC.md` section 8 working over
the real collections.

---

## Phase 8 — Packaging and distribution

- Package API inspection scaffolding exists, but `.zpkg` publication remains
  blocked on reproducible archives and cryptographic integrity verification.
- `.zpkg`, `zirk.lock`, `zirk add/remove/install/update`, `zirk package`,
  `zirk publish`.
- Reproducible builds (`COMPILER_SPEC.md` section 7).

---

## Phase 9 — Developer experience

- A canonical, idempotent formatter is delivered for whole `.zrk` documents;
  future formatter work is configuration and range-formatting, not a basic
  document formatter.
- A linter sharing parser/types with the compiler.
- An LSP with incremental snapshots.
- A debugger with symbols and mapping back to `.zrk`.

This phase is large in volume of work but low in conceptual risk — nothing here
is new territory, it is high-volume engineering.

---

## Phase 10 — Metaprogramming

- Decorators (`fn dec`), a versioned and immutable Syntax API.
- Advanced `std.reflect`.

It is left for the end on purpose: it depends on the rest of the compiler being
stable, because decorators touch nearly every layer (parser, types, IR).

---

## Phase 11 — Production hardening

- Fuzzing, differential debug/release testing, a public benchmark suite
  (`COMPILER_SPEC.md` section 1).
- Full coverage of the target matrix.

---

## Phase 12 — Self-hosting (long horizon, years)

Rewrite the compiler in Zirk once the language is mature and stable enough. It
is the same path Rust followed (rustc started in OCaml), and Go (its compiler
started in C), and Zig (started in C++). None of these projects self-hosted from
day one, and Zirk should not attempt it prematurely either.

---

## How to use this roadmap

- A phase counts as complete when its "Output" runs with real tests, not when
  the code "is almost there".
- If mid-phase there is a temptation to bring something forward from a later
  phase, note it as a pending item and continue with the current phase — it is
  not implemented halfway.
- This is a living document: if a phase turns out to be badly sized (too large,
  or with unforeseen dependencies), it is adjusted here rather than improvised
  in the code.
