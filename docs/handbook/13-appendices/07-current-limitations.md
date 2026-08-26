# Current Limitations

Zirk's final 1.x semantics are broader than the current compiler. The repository
has a precedence table, stable diagnostic-code policy, and normative detail for
ranges, slicing, traditional `for`, patterns, value classes, generators, and
callable types. Their existence in documentation does not imply every pipeline
stage implements them.

Current high-impact delivery limits include:

- `Fn(P...) => R` is parseable and type-checked in every position (Phase 4d),
  and a named function, capture-less lambda, or a single capturing closure
  literal written directly at a local's initializer or a function's `return`
  freely satisfies it — but general callable-type polymorphism across two or
  more *differently-captured* closures at one position still needs the
  captures heap-boxed behind a uniform representation, not built yet.
- Phase 4a–4c implement expected errors, explicit exceptions, and the initial
  one-resource `match with`, but final suppressed/combined cleanup failures,
  grouped acquisition, complete stack traces, throwable immutability,
  cancellation cleanup, resource transfer, and dependent lifetimes remain.
- Managed memory reclaims automatically now: `docs/decisions/ADR-003-memoria.md`
  closed on a non-moving mark-sweep collector, real (not a placeholder),
  reclaiming unreachable memory including cycles without exposing GC,
  ownership, or moves as source semantics. `Weak<T>` is delivered
  (`Weak.from`, `.upgrade(): T?`, `.is_alive`) — a small collector-tracked
  indirection cell whose target is cleared before the collector frees it,
  never after, so `.upgrade()`/`.is_alive` never observe reclaimed memory.
  Deep `clone()` is delivered for `class` receivers: compiler-derived when
  every field is itself `Clone`, preserving internal sharing and cycles
  through a runtime memoization map, and rejected at compile time when the
  field graph reaches a `Pointer<T>`, `Resource`, or other non-`Clone`
  member. `record`/`value class` and enum-typed fields do not yet derive
  `Clone` (records have no identity of their own; a pre-existing codegen
  gap leaves an enum's inactive-variant fields uninitialized in a way the
  shared field-offset walk would read unconditionally). Dependent
  references and automatic pinning remain Phase 4e work. `inmut::strict` rejects a direct rebinding and a write through a field
  projection off a strict binding; strictness declared on a field itself
  (independent of its container's own mutability) and a mutating method call
  reached through a strict reference remain open. `unsafe fn`/`unsafe {}`/
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
  trigger a rollback. Known limitation: an early `return`/`break`/`continue`
  out of `unsafe {}` leaks that block's journal handle (not a soundness
  issue — just an unfreed allocation). `NativeSlice<T>`/`NativeSliceMut<T>`
  are delivered: validated bounded views constructed only via
  `pointer.as_slice(length)`/`.as_slice_mut(length)` (both `unsafe`,
  returning `Result<view, NativeError>`), checking nullability, alignment,
  extent, and (for the syntactically direct `Pointer.from(place).as_slice(n)`
  shape only) known extent against the real underlying storage; using an
  already-constructed view needs no `unsafe`, with bounds checks active on
  every index; the `Pointer<T>` escape rule now also covers both view
  types. Delivering this also meant adding `expr[index]` as a genuine new
  postfix expression grammar (`Expr::Index`) — indexing did not exist
  anywhere in the compiler before, and is now dispatched by receiver type
  (today: `NativeSlice<T>`/`NativeSliceMut<T>`) so Phase 7's `Array<T>`/
  `List<T>` can register their own support later without another grammar
  change. Volatile access, untagged native-union access (no union type
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
  `record`/`value class` is also delivered (`fase-3-structural-equality`):
  `==`/`!=` compares every field, recursing into a nested `record`/`value
  class` field, and short-circuits on the first difference — a `class`-typed
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
  instantiation (`Bar<Baz<T>>`) fails in shared generic-inference
  machinery that doesn't yet recurse into a nested instantiation's own
  type arguments; and a self-referencing enum declaration — recursive,
  generic **or plain** — cannot be declared at all today, because enum
  declaration resolves variant field types before registering the enum's
  own name (unlike a class, which registers its name first). Still
  required: user generic contracts and value-type contract dispatch.
- `Float128` arithmetic lacks complete Windows verification and `Float128`
  currently lacks `to_string()` support.
- Standard-library, structured-concurrency, packaging, developer-tooling,
  decorator, and public documentation surfaces are specified ahead of full
  compiler/runtime delivery.

Zirk 1.x explicitly excludes browser/WebAssembly, public runtime directives or
event loop, standalone `worker`, `async fn`, textual inline assembly, general
`comptime`/`defer`, multiple class inheritance, traditional overloads, Result
`?`, and public ownership/reference-counting semantics.

---

**Previous:** [← Differences from Rust](06-differences-from-rust.md) · **Next:** [ Roadmap](08-roadmap.md)
