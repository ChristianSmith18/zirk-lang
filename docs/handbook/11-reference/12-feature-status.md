# Feature Status

Status values are **specified and implemented**, **partially implemented**, **specified but not implemented**, **historical/exploratory**, or **explicitly excluded**. A repository revision and evidence are required before claiming implementation.

The type taxonomy, Float family, grapheme Char, shared mutable String, binding
permissions, native operators, temporal family, callable types, escaping
closures, projection-copy semantics, object/contract model, complete generics,
algebraic data, collections, and matching rules are authorial definitions even
where compiler delivery is pending.

The same applies to mandatory `Result`, checked explicit exceptions, implicit
typed runtime exceptions, patterned catch, resource responsibility, combined
cleanup failure, the two-block permission model, signed location-bound consent,
and incremental requester-aware validation.

Managed-memory strategy neutrality, weak/dependent references, deep graph
clone, transactional unsafe rollback, irreversible commit, typed structured
tasks, aggregation, fair selection, transfer/share derivation, and safe-code
data-race freedom are also final definitions even where delivery is pending.

| Area | Definition | Current delivery evidence | Remaining material work |
|---|---|---|---|
| frontend and diagnostics | defined | Phases 0–3b; stable codes and recovery exist | lossless/incremental syntax and complete public diagnostic catalog |
| scalar and text families | defined | Phase 3b complete for its scope | `Float128` Windows verification and formatting |
| classes, contracts and value types | defined | Phase 3 scoped implementation; `fase-3-abstract-dispatch` delivered dynamic dispatch through a user-declared `abstract class`, generalizing the vtable mechanism already proven by the native `Throwable` hierarchy; `fase-3-structural-equality` delivered derived `==`/`!=` for `record`/`value class` (field-by-field, short-circuiting, recursing into nested value fields); `fase-3-generic-enums` delivered lowering for a flat single/multi-type-parameter user generic enum; `fase-3-generic-substitution-recursion` fixed generic-inference substitution to recurse into a nested generic instantiation (a gap shared by every generic function/method call, not enum-specific); `fase-3-recursive-enums` made enum declaration order independent of self/mutual references (an enum and a class, or two enums, can now reference each other regardless of order) and turned the genuinely-self-referential-field hazard (a real compiler crash found during that work) into a clean compile-time diagnostic | generic contract lowering, value-type contract dispatch, constructing/using a genuinely self-referential enum field (needs automatic heap indirection — a new `IrType` case plus runtime/GC integration — not yet designed) |
| callable types and escaping closures | defined | Phase 4d: `Fn(...) => R`/`Function(...) => R` parseable and checked in every position (parameter, return, field, generic argument, local annotation); a named function or a capture-less lambda freely interchanges with any structurally compatible position (design D12); a single capturing closure literal written directly at a local's initializer or a function's `return` escapes its creating frame (design D14), including a recursive lambda with an explicit binding type; `is` compares two callable values by identity (design D15) | general callable-type polymorphism across two or more *differently-captured* closures at one position (design D13, needs captures heap-boxed behind a uniform representation); a captured binding written by the closure lifted into a shared cell; `.clone()` on a closure; a capturing literal at a function parameter, a field, or an argument through a variable (D14 covers only a local's initializer and a function's own return, the two positions a single static AST occurrence can size soundly without D13) |
| multiple declarations and simultaneous assignment | defined | Phase 4d complete: comma-grouped `mut`/`inmut`/`inmut::strict` declarations share one type annotation with independent defaults; simultaneous assignment evaluates every source before any destination write and rejects arity mismatches, duplicate destinations, and writes through an `inmut::strict` projection | none for this scope |
| collections and generics | defined | native iteration subset exists | complete stdlib collections and general generic contract/enum delivery |
| temporal family | defined | syntax/type design only | runtime and `std.time` delivery in Phase 7 |
| `Result<T,E>` | defined | Phase 4a scoped implementation | generic combinators that depend on callable completion |
| explicit exceptions | defined | Phase 4b scoped implementation, plus 4 of 5 implicit native safety checks catchable | overflow and invalid-cast as catchable errors, suppressed failures, complete traces and immutability |
| deterministic resources | defined | Phase 4c single-acquisition implementation | grouped acquisition, close-error composition, cancellation, transfer and lifetimes |
| permissions and secure approval | defined | specification and handbook | Phase 6 manifest, signed approval store, incremental requester audit |
| memory, native views and transactional unsafe | defined | ADR-003 closed: non-moving mark-sweep, real, reclaiming unreachable memory including cycles (`zirk_rt_alloc` no longer "never frees"); `Weak<T>` delivered (`Weak.from`, `.upgrade(): T?`, `.is_alive`, never keeps its referent alive, cleared before the referent is freed); deep `clone()` delivered for `class` receivers — compiler-derived when every field is `Clone`, preserves internal sharing and cycles via a runtime memoization map driven by the object's own runtime descriptor (correct for a polymorphic subclass), rejected at compile time (naming the offending field) when the graph reaches a `Pointer<T>`/`Resource`/other non-`Clone` member; `inmut::strict` rejects a direct rebinding and a write through a field projection off a strict binding; `unsafe fn`/`unsafe {}`/`commit {}` parse and are context-checked; `Pointer<T>` (ABI-safe element types) supports construction/read/write/offset/cast with a conservative escape rule; `extern "C" fn` declares and calls a native function under a narrow ABI-safe signature, resolved by the system linker; `unsafe {}`/`commit {}` deliver the transactional journal/rollback contract — every managed write inside `unsafe {}` is journaled and committed on normal exit or rolled back in reverse order when an exception becomes pending before that exit, `commit {}` durably publishes the enclosing block's journal before running its own body, and a `try`/`catch` nested inside `unsafe {}` that handles an exception locally does not trigger a rollback; `NativeSlice<T>`/`NativeSliceMut<T>` deliver validated bounded views constructed from `Pointer<T>`+length (`.as_slice`/`.as_slice_mut`, both `unsafe`, returning `Result<view, NativeError>`), with bounds-checked indexing/`.length`/`.is_empty` usable in safe code — this also delivered general `expr[index]` grammar (`Expr::Index`), which did not exist in the compiler before, via a receiver-type-keyed dispatch table left open for Phase 7 collections | dependent references and automatic pinning, `clone()` derivation for `record`/`value class`/enum-typed fields, field-declared strictness across a projection boundary, a mutating method call through a strict reference, volatile access, a native-library-linking manifest, extending journal cleanup to an early `return`/`break`/`continue` out of `unsafe {}` (currently leaks the journal handle without a soundness impact), general provenance tracking for a native view's known extent beyond one recognized syntactic shape |
| tasks, channels, select and cancellation | defined | specification and handbook | Phase 5 staged runtime/compiler delivery |
| parallelism, threads, synchronization and atomics | defined | specification and handbook | Phase 5 staged delivery after structured tasks |
| standard library | defined at API-contract level | handbook Block One documented | Phase 7 implementation and final example audit |
| decorators | defined | normative semantics and OpenSpec complete | Phase 10 compiler/tooling implementation |
| packaging and developer tooling | defined at architecture level | bootstrap CLI/editor pieces exist | Phases 8–9 and remaining deep documentation |

The table describes the repository state audited on **20 August 2026**. The
archived phase change and its tests are the evidence for a completed delivery
slice. A later implementation change must update this table when it changes a
row; documentation design alone must not promote delivery status.

Explicit Zirk 1.x exclusions include browser/WebAssembly, public runtime directives, standalone `worker`, `async fn`, public event loop, textual inline assembly, general `comptime`, general `defer`, multiple class inheritance, traditional function overloading, `Result` propagation `?`, and public ownership/RC semantics.

The intended surface now includes exponentiation, descending and stepped ranges,
Python-style slicing, classic and iterable `for`, `do ... while`, regex literals,
optional-`fn` lambdas, constructor signatures, mapped traditional enums,
generators, comma-grouped patterns, tuple/record destructuring, `Fn` callable
values, and bound methods. Compiler support may trail this target; consult milestone diagnostics
rather than treating absence in the current parser as a language exclusion.

Remaining artifacts include a complete generated machine grammar and exhaustive
public diagnostic catalog. Stable diagnostic-code policy and many assigned
codes already exist; missing catalog coverage does not make accepted language
semantics open.

---

**Previous:** [← Standard Library Index](11-standard-library-index.md) · **Next:** [ Type Member Index](13-type-member-index.md)
