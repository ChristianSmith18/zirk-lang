# Current Limitations

Zirk's final 1.x semantics are broader than the current compiler. The repository
has a precedence table, stable diagnostic-code policy, and normative detail for
ranges, slicing, traditional `for`, patterns, generators, and callable types.
Tuples, `Duration` literals, `Regex` literals with `matches`/`find`/`replace`,
the `String` mutation/search surface, `Char` classification/normalization, and
`type` alias lowering are now delivered; `value class` was removed (use
`record`). Their existence in documentation does not imply every pipeline
stage implements them.

Current high-impact delivery limits include:

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
  exists), weak atomic ordering (`Atomic<T>` is Phase 5), a
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
  change. `value class` is removed — `record` covers its semantics.
  Also delivered inside the change: `Regex.split` and
  `Regex.find_all(text): List<Regex.Match>` for match iteration
  (`matches(text)` keeps its `Boolean` meaning), `re'...'` patterns in
  `match` arms, `Range<T>` (`start`, `end`, `step`, `reverse()`, slicing,
  `Iterable<T>` for numeric `T` and `Duration`), derived `Clone` for
  `record`/`enum`, `String` writes `s[i] = c` and slicing
  `s[start:end:step]`, and lowering of user-defined generic `implements
  Contract<T>` satisfaction (contract members that name the contract's own
  `T` were since covered by the Phase 3 OOP closing slice described above).
  `String.split` remains pending.
- `Float128` arithmetic lacks complete Windows verification. `Float128`
  `to_string()` is implemented by truncating to `Float64` first, which can
  lose precision for values not exactly representable in `f64`.
- Exact `Float` irrational operations (`sqrt`, a fractional `pow`) carry only
  `f64`-grade precision (~15 significant digits), not the full
  28-digit budget the rational operations use. `Float.format(spec)` and an
  explicit `RoundingMode` for `div` / `round` are specified but not implemented;
  both round half-to-even by default.
- Standard-library, structured-concurrency, packaging, developer-tooling,
  decorator, and public documentation surfaces are specified ahead of full
  compiler/runtime delivery.

Zirk 1.x explicitly excludes browser/WebAssembly, public runtime directives or
event loop, standalone `worker`, `async fn`, textual inline assembly, general
`comptime`/`defer`, multiple class inheritance, traditional overloads, Result
`?`, and public ownership/reference-counting semantics.

---

**Previous:** [← Differences from Rust](06-differences-from-rust.md) · **Next:** [ Roadmap](08-roadmap.md)
