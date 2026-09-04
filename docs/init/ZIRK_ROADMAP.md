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
`fn main(): Void { stdout.println("..."); }` really compiles through LLVM and
runs as a native binary.

Language subset: `fn main`, `Void`, `String`, `Int32`, `Boolean`, literals,
basic arithmetic, `if`/`else`, `mut`/`inmut` variables, and a minimal hardcoded
`println` (not the full stdlib yet).

Explicitly out: generics, classes, `Result`, concurrency, decorators,
multi-file modules, `init.zrk`.

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
- Modules within a single crate: basic `share`/`import`, without `init.zrk` yet.

**Output:** programs with several functions, real control flow and closures —
still without classes or concurrency.

---

## Phase 3 — Objects and the type system

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
(merged) closed the derived-equality gap: `==`/`!=` on a `record`/`value
class` now lowers to a field-by-field, short-circuiting comparison —
recursing into a nested `record`/`value class` field and dispatching a
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
unchanged, non-generic call site. `value class` cannot exercise this yet —
a pre-existing, separate gap: its own compact declaration grammar has no
`implements` clause or method-body syntax at all today. Generic contracts
remain.

- `class`, `construct`, visibility (`public`/`private`/`protected`), single
  inheritance, interfaces, traits.
- Generics with `from` (constraints).
- Records, value classes, algebraic enums, unions.
- Casts (`as`, `<T>`, `unsafe` casts).

**Output:** the object-oriented subset of the spec working, including basic
generics.

---

## Phase 3b — Complete scalars, conversions and text

**Status: complete for its scoped delivery.** `Float128` formatting is now
implemented by truncating to `Float64` first, which can lose precision for
values not exactly representable in `f64`. Windows `Float128` arithmetic
remains an implementation limitation.

Phase 1 implemented one integer width, one Boolean and an opaque `String`.
Everything else the type system promises about scalars was never assigned a
phase. This one owns it:

- The remaining integer widths: `Int8`, `Int16`, `Int64`, `Int128` and the whole
  `UInt*` family, with fixed width and checked arithmetic.
- The binary floating family `Float16`, `Float32`, `Float64`, `Float128`, with
  `Float` aliasing `Float64`, explicit infinities and no valid `NaN` — an
  operation that would produce one is a controlled error.
- `Char` as exactly one Unicode grapheme, which may span several code points.
- Deep contextual conversion: `Float(3 / 4)` converts the operands before the
  division rather than converting its integer result.
- Bitwise and shift operators, levels 7 to 10 of the operator table.
- String interpolation, which needs the `to_string()` contract Phase 3 defines.

They arrive together because they depend on each other: contextual conversion
means nothing without `Float`, `Float` literals mean nothing without the family,
and interpolation needs Phase 3's contracts. Splitting them would mean
implementing each one twice.

**Output:** the complete scalar surface of the spec — every numeric width, real
`Char`, and the conversion rules that connect them.

---

## Phase 4 — Failure, callable completion, and memory

**Status: complete for its scoped delivery.** All sub-phases (4a–4e) now parse,
type-check, lower, and run end-to-end. The remaining deliberately-out-of-scope
items (volatile access, native unions, `Atomic<T>`, general slice provenance
beyond the direct shape, `record`/`value class`/`enum` `Clone` derivation) are
tracked as Phase 5+ or follow-up work.

### Phase 4a — Expected errors

**Status: complete for its scoped delivery.** Implements `Result<T,E>`,
exhaustive handling, mandatory consumption, and explicit `_ = expression`
discard.

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
  of the pinned variable. `record`/`value class` and enum-typed fields remain
  excluded from `Clone` derivation for now.
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
  access (no union type exists), weak atomic ordering (`Atomic<T>` is
  Phase 5), and general provenance tracking for a view's known extent
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
sub-steps, not in one go:

1. `Task<T>`, `task scope`, `await`, sibling-failure propagation, cancellation,
   shield and timeout on a custom single-threaded executor first.
2. `Task.all`/`first`/`settled`, `TaskSettlement<T>`, fair `select`, and
   bounded/unbounded `Channel<T>` with closure/backpressure.
3. Compiler-derived transfer/share and capture analysis sufficient to enforce
   the safe-code data-race guarantee across every supported boundary.
4. `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, and safe-default
   `Atomic<T>`; weak atomic ordering remains unsafe.
5. scoped `thread`, `task.blocking`, and application root supervision.
6. ordered/unordered `parallel` operations and associative/deterministic
   reductions on a multicore pool.

**Output:** the structured concurrency contract of
`STRUCTURED_CONCURRENCY_SEMANTICS.md`, including cleanup, selection, transfer,
and data-race guarantees, working end to end.

---

## Phase 6 — Project system and CLI

- `init.zrk` as a declarative DSL (its own parser, not reusing the Zirk parser).
- `project`, `build_targets`, `globals`, library `requires`, and application
  `permissions` with per-operation `during: build | runtime | both`.
- CLI: `new`, `init`, `run`, `build`, `check`, `test`.
- Real cross-compilation to the targets of the spec (`COMPILER_SPEC.md`
  section 6).

**Output:** real multi-file projects, with a manifest and cross-compilation.

---

## Phase 7 — Stdlib

Order suggested by real dependency, not by the order they appear in the spec:

`std.io` (already partially covered) → `std.collections` → `std.fs`/`std.path` →
`std.time` → `std.process` → `std.task`/`std.thread`/`std.sync` (wrapping
Phase 5) → `std.json` → `std.net`/`std.http` (the largest of them all) →
`std.crypto` → `std.testing` (`@test`/`@e2e`/`@bench`) → `std.reflect` →
`std.system`.

Two families in this phase are **not** ordinary library objects, and the
distinction changes who implements them:

- The **temporal family** — `Date`, `Time`, `DateTime`, `Instant`,
  `ZonedDateTime`, `TimeZone`, `Duration` and `Period` — are compiler-known
  native immutable values with their own literals, operators and type rules.
  The lexer and the checker know them before `std.time` exists; what this phase
  adds is their implementation, their IANA zone data and their API, not their
  existence as types.
- The **collection family** — `Array<T>`, `List<T>`, `Map<K,V>`, `Set<T>` — are
  native reference types under the same rule.

`String` grapheme indexing also lands here, which is the moment ADR-005's
boundary was designed to protect. With indexing come the forms that depend on
it: slicing `[start:end:step]`, and `Range<T>`'s `.step(distance)` and
`.reverse()`.

Regex also lands here — the `re'pattern'` literal and regex patterns in `match`.
The literal is core syntax and the lexer knows it earlier, but it means nothing
without an engine to run it, and the engine is a library.

**Output:** real non-trivial applications (a CLI, a simple backend) writable in
Zirk using only the stdlib.

---

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

- `.zpkg`, `zirk.lock`, `zirk add/remove/install/update`, `zirk package`,
  `zirk publish`.
- Reproducible builds (`COMPILER_SPEC.md` section 7).

---

## Phase 9 — Developer experience

- A canonical, idempotent formatter.
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
