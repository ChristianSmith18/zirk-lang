# Zirk — Construction roadmap

This roadmap translates the five normative specs — which describe a mature Zirk
— into a real order of construction. Each phase has a verifiable goal: you do
not move to the next one until the current goal actually runs, not merely "is
almost there".

No real language was ever built by implementing its full spec in one pass. This
document is the discipline that keeps this one from trying.

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
`fase-3-objects-and-type-system` change records remaining pipeline limitations
for generic contracts, abstract dispatch, value-type contract dispatch, and
derived structural equality.

- `class`, `construct`, visibility (`public`/`private`/`protected`), single
  inheritance, interfaces, traits.
- Generics with `from` (constraints).
- Records, value classes, algebraic enums, unions.
- Casts (`as`, `<T>`, `unsafe` casts).

**Output:** the object-oriented subset of the spec working, including basic
generics.

---

## Phase 3b — Complete scalars, conversions and text

**Status: complete for its scoped delivery.** Platform and formatting limits
for `Float128` remain tracked as implementation limitations.

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

Phase 4 is split so completed slices are not confused with final semantics that
still need delivery.

### Phase 4a — Expected errors

**Status: complete for its scoped delivery.** Implements `Result<T,E>`,
exhaustive handling, mandatory consumption, and explicit `_ = expression`
discard.

### Phase 4b — Exceptions

**Status: complete for its scoped delivery.** Implements explicit
`throws`/`try`/`catch`/`finally`, typed catch dispatch, rethrow, `fatalError`,
and initial throwable runtime support. A follow-up slice
(`native-runtime-errors-catcheable`, archived) made four of the five
compiler-known implicit safety checks — division by zero, an out-of-range
shift, a negative string-repeat count, and `Float` producing `NaN` — catchable
`RuntimeError` subclasses instead of unconditional aborts. Suppressed
failures, fully materialized traces, deep thrown-object immutability, and the
remaining two implicit native safety errors (arithmetic overflow, invalid
cast) remain final requirements, not completed claims.

### Phase 4c — Deterministic resources

**Status: complete for its scoped delivery.** Implements one-resource
`match with` and cleanup across ordinary control flow and explicit exception
propagation. Grouped acquisition, surfaced/combined close failures,
cancellation integration, transfer, and dependent-resource analysis remain
pending final behavior.

### Phase 4d — Callable and binding completion

**Status: complete for its scoped delivery — both slices shipped
(`fase-4d-callables`; `fase-4d-declaraciones-multiples`).**

- [x] Parse and type `Function(P...) => R` and preferred alias `Fn(P...) => R`
  in parameters, returns, attributes, generic arguments, and local
  annotations.
- [x] Permit a closure to escape its creating frame through a `Fn(...) => R`
  annotation, for the shapes this pass covers: a named function or a
  capture-less lambda freely interchanges with any structurally compatible
  position (design D12); a *single* capturing closure literal written
  directly at a local's initializer or a function's own `return` escapes
  too, including a recursive lambda with an explicit binding type (design
  D14). `is` compares two callable values by identity (design D15).
  **Not** covered by this slice: general callable-type polymorphism across
  two or more differently-captured closures at one position (design D13 —
  needs the captures heap-boxed behind a uniform, `{function pointer,
  capture-block pointer}` representation); a captured binding written by
  the closure, lifted into one shared mutable cell; `.clone()` on a
  closure's environment; a capturing literal reaching a function parameter,
  a field, or an argument passed through a variable (D14's single-literal
  rule only covers the two positions — a local's own initializer, a
  function's own return — where exactly one static AST occurrence can size
  the position soundly without D13's boxing).
- [x] Implement same-type multiple declarations with independent defaults.
- [x] Implement exact-arity simultaneous assignment: evaluate all sources
  before writes, reject duplicate destinations, and preserve projection
  copy/place semantics. **Note:** this reuses the single-target assignment's
  existing writability rule set unchanged (design D4), which today only
  checks a field's own `inmut`, not the strict-aliasing of an
  `inmut::strict` base reference (`p.x = 5;` through `p: inmut::strict`
  already compiles on `main` before this change) — a pre-existing gap in
  `inmut::strict` projection-write checking, reproduced faithfully rather
  than fixed here; tracked under Phase 4e's `inmut::strict` reachable-alias
  analysis.

**Output:** a named function or capture-less lambda interoperates with
`Fn(...) => R` anywhere it is written; a single capturing closure escapes its
creating function through a typed local or a `return`, including recursive
lambdas; `is` works between callables. `left, right = right, left` and `mut
a, b: String;` are implemented — comma-grouped declarations share one type
annotation and independent defaults, and simultaneous assignment evaluates
every source before any destination write, rejects arity mismatches and
duplicate destinations with dedicated diagnostics. General callable-type
polymorphism (any two differently-captured closures sharing one position) is
deferred to a follow-up change.

### Phase 4e — Managed memory and unsafe boundaries

**Status: in progress. ADR-003 closed (non-moving mark-sweep); the real
collector, `inmut::strict` projection-write checking
(`fase-4e-inmut-strict-proyeccion`), the `unsafe`/`Pointer<T>`/`extern`
core (`fase-4e-unsafe-pointer-extern`), `Weak<T>` (`fase-4e-weak`), deep
`clone()` for reference graphs (`fase-4e-clone`), and the transactional
unsafe journal/rollback (`fase-4e-unsafe-journal`) shipped; native slices
remain.**

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
  automatic bounded native pinning — **partial** (`fase-4e-weak`,
  `fase-4e-clone`): `Weak<T>` delivered, restricted to reference-typed
  referents, with `Weak.from`, `.upgrade(): T?`, and `.is_alive: Boolean`.
  Represented as a small collector-tracked indirection cell (a "WeakCell")
  whose own target field is never traced as a strong edge during mark, and
  gets cleared — before the collector frees the referent, never after — by
  a dedicated pass between mark and sweep, skipped entirely in any program
  that never allocates a `Weak<T>`. Corrected the pre-existing "Weak
  references" requirement's own wording along the way: it named
  `Option<T>`/`None`, neither of which exists in Zirk — the delivered (and
  now normative) signature is `upgrade(): T?`, `null`, matching the
  language's real nullable idiom. Deep `clone()` also delivered
  (`fase-4e-clone`): compiler-derived for a `class` whose every field is
  itself `Clone`, preserving internal sharing and cycles within the new
  graph via a runtime memoization map keyed by source address, driven by
  the object's own runtime descriptor (so it walks a polymorphic
  subclass's real fields, not just its declared static type — the same
  descriptor-embedded field table `mark_object` already reads), and
  rejected at compile time — naming the offending field or transitive path
  — when the field graph reaches a `Pointer<T>`, a `Resource`, or another
  non-`Clone` member. Also corrected a wording inconsistency found while
  scoping this: `Cloneable` appeared in two isolated spec scenarios against
  `Clone` everywhere else (including the primary deep-clone-contract text
  itself); both corrected to `Clone`. A genuine collector bug surfaced by
  `Clone`'s own end-to-end soundness test was fixed along the way: an
  absent nullable's payload was left as LLVM `undef` rather than zeroed,
  which the collector's own `mark_object` (and now `Clone`'s traversal)
  reads unconditionally regardless of the nullable's flag — `undef` could
  lower to any bit pattern and crash a pointer dereference; now zeroed.
  **Not** covered: dependent references and automatic bounded native
  pinning remain their own, separate work. `record`/`value class` and
  enum-typed fields are excluded from `Clone` derivation for now (records
  have no identity; a latent, pre-existing codegen gap leaves an enum's
  inactive-variant fields `undef`, which the shared field-offset walk would
  read unconditionally — too broad a fix for this change's own scope).
- [x] Implement `inmut::strict` with reachable-alias analysis — **partial**:
  direct rebinding (Phase 3) and writing through a field projection off a
  strict binding (`fase-4e-inmut-strict-proyeccion`) are both rejected.
  Strictness declared on a field itself, independent of its container's own
  mutability, and a mutating method call reached through a strict
  reference, remain open — not full reachable-alias analysis yet.
- [x] Implement `unsafe {}`, `Pointer<T>`, native slices, and
  compiler-enforced memory-safety boundaries — **partial**
  (`fase-4e-unsafe-pointer-extern`): `unsafe fn`/`unsafe {}`/`commit {}`
  parse and are context-checked; `Pointer<T>` (an ABI-stable element-type
  subset) supports construction from an addressable local/field,
  read/write/offset/offset-bytes/cast, with a conservative escape rule
  (cannot be returned, stored in a field, or captured); `extern "C" fn`
  declares and calls a native function (`docs/decisions/ADR-015-declaracion-extern.md`
  closed the syntax this needed, which no prior spec had decided) under a
  narrow ABI-safe type surface, resolved by the system linker with no new
  library-linking manifest. **Not** covered: `NativeSlice<T>`/`NativeSliceMut<T>`
  validated views, `.read_volatile()`/`.write_volatile()`, untagged
  native-union access (no union type exists), weak atomic ordering
  (`Atomic<T>` is Phase 5).
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
  isolation. **Known, accepted limitation**: an early `return`/`break`/
  `continue` out of an `unsafe {}` block leaks that block's journal handle
  (not a soundness issue, just an unfreed allocation) — extending the
  existing `try`-only cleanup-on-early-exit mechanism to `unsafe` frames is
  future work, not delivered here.
- [ ] Finish resource transfer/dependency and throwable cleanup interactions that
  require the complete memory model. **Not started.**

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
