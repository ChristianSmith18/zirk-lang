# Zirk feature status catalog

This is the single source of truth for the implementation status of every
language feature that has an owning phase in the roadmap. Other documents
(`README.md`, `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_AGENT_PROMPT.md`,
`docs/decisions/README.md`) link here instead of restating the same
enumeration.

Status values for the pipeline columns:

- **yes** — the stage is fully implemented for the feature's scoped delivery.
- **partial** — the stage works for the subset the change actually shipped, with
  known gaps documented in the *Notes* column.
- **no** — the stage is not implemented; the feature is still gated by a
  `NOT_IMPLEMENTED` (parser), `NOT_LOWERED` (IR), `PENDING_FEATURE`
  (checker), or equivalent diagnostic.

The evidence for each row is the relevant active or archived OpenSpec change, the
automated tests, or the source file that gates the feature. A row is updated
only when the same change that changes the code also updates this file.

Last reviewed for `deliver-std-http-and-environment`, **2026-10-20**.

## Browser-hosted interpreter tooling

`add-browser-ir-interpreter` adds an optional, versioned WASM tooling package,
not a Zirk target. It executes verified IR for scalar and text values, exact
Decimal through `zirk-runtime-core`, closures, collections, regex, temporal
values, the object model (classes, inheritance, abstract classes, interfaces,
traits, static and nested classes, generic classes, records, enums, typed
exceptions) and bounded output. The package contract has deterministic compiler,
capability, runtime, and resource-limit responses.

The conformance evidence is the native end-to-end corpus: 184 of its 218 programs
reproduce their native output in the browser, and the remaining 34 are the
listed capability exclusions (concurrency, timers, `parallel`, channels,
pointers, native slices, `unsafe` journals, `Pin`/`Dependent` and resource
transfer); deep `clone()` runs on the interpreter heap. Object-model coverage is catalogued in
[`browser-oop-coverage.md`](../browser-oop-coverage.md). `run` can also return an opt-in, bounded, read-only execution trace of object
events (ADR-022). Native effects and multi-file/package execution remain
outside the browser baseline. The exhaustive
policy is [`browser-interpreter-support-matrix.md`](../browser-interpreter-support-matrix.md);
no native feature becomes browser-available merely because its frontend or
native backend is delivered.

---

## Legend

| Phase | Meaning |
|---|---|
| 0 | Decisions before writing code (ADRs, workspace, toolchain). |
| 1 | Minimal end-to-end pipeline. |
| 2 | Core language surface. |
| 3 | Objects and the type system. |
| 3b | Complete scalars, conversions and text. |
| 4 | Failure, callable completion and memory. |
| 4a | Expected errors (`Result<T,E>`). |
| 4b | Exceptions (`throws`/`try`/`catch`/`finally`). |
| 4c | Deterministic resources (`Resource<E>`, `match ... with`). |
| 4d | Callable and binding completion. |
| 4e | Managed memory and unsafe boundaries. |
| 5 | Concurrency and parallelism. |
| 6 | Project system and CLI. |
| 7 | Standard library. |
| 7b | Functional style and generators. |

---

## Phase 0 — Decisions

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| LLVM 20.1 toolchain pin | yes | yes | yes | yes | yes | yes | `ADR-001`; CI on all four active platforms. |
| `zirk-runtime` as staticlib | yes | yes | yes | yes | yes | yes | `ADR-002`; linked into every produced binary. |
| Memory strategy constraints | n/a | n/a | n/a | n/a | partial | n/a | `ADR-003` closed; non-moving mark-sweep delivered in Phase 4e. |
| Portability A/B | n/a | n/a | n/a | n/a | n/a | n/a | `ADR-004`; B verified by target-matrix tests; A verified in CI for Linux x86_64/aarch64, macOS aarch64, Windows x86_64. |

## Phase 1 — Minimal end-to-end pipeline

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `fn main(): Void` | yes | yes | yes | yes | yes | yes | `ZIRK_AGENT_PROMPT.md` Phase 1; end-to-end `hello.zrk` works. |
| `Void`, `Int32`, `Boolean`, `String` | yes | yes | yes | yes | yes | yes | `Type::from_name` resolves every one. |
| `mut`/`inmut` | yes | yes | yes | yes | yes | yes | — |
| Literals, arithmetic, comparison, logic | yes | yes | yes | yes | yes | yes | Overflow checks in codegen. |
| `if`/`else` | yes | yes | yes | yes | yes | yes | — |
| `std.io` standard streams | yes | yes | yes | yes | yes | yes | Source-defined `stdin`, `stdout`, and `stderr`; synchronous native/interpreter input, destination-labelled output, and browser stdin preflight. Async and terminal controls remain deferred. |

## Phase 2 — Core language surface

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `for`, `for ... in` over ranges | yes | yes | yes | yes | yes | yes | `Keyword::For`, `Keyword::In` no longer gated. |
| `while`, `loop`, `break`, `continue` | yes | yes | yes | yes | yes | yes | — |
| `if` as expression | yes | yes | yes | yes | yes | yes | — |
| `match` with exhaustiveness over simple enums | yes | yes | yes | yes | yes | yes | — |
| `Enum` declarations | yes | yes | yes | yes | yes | yes | Including associated data after `fase-3-recursive-enums`. |
| Nullability (`T?`, `?.`, `??`) | yes | yes | yes | yes | yes | yes | `T?` folds into the `nullable` bit. |
| `share`/`import`/`use` | yes | yes | yes | yes | yes | yes | Single-crate module linking; per-module namespacing is Phase 6. |
| Local closures (non-escaping) | yes | yes | yes | yes | yes | yes | `lower_lambda` copies captures at creation time. |
| Optional/named/variadic parameters | yes | yes | yes | yes | yes | yes | Includes spread/rest operators: `...expr` in calls/collections, `[first, ...rest]`, `{ ...source, field }`, `{ field, ...rest }` (`spread-rest-operators`) |
| Compound assignment (`+=`, etc.) | yes | yes | yes | yes | yes | yes | `TokenKind` compound operators. |

## Phase 3 — Objects and type system

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `class`, `construct` | yes | yes | yes | yes | yes | yes | — |
| Promoted constructor parameters and implicit constructors | yes | yes | yes | yes | yes | yes | `add-class-construction-sugar`: member-modified class constructor parameters become fields; constructor-less classes accept zero arguments; an implicit subclass constructor delegates to an accessible zero-argument base constructor. |
| Visibility (`public`/`private`/`protected`) | yes | yes | yes | yes | yes | yes | — |
| Inheritance (`extends`) | yes | yes | yes | yes | yes | yes | Single inheritance; `super`/`#override`; `final` seals classes and methods. |
| `interface`/`trait` | yes | yes | yes | yes | yes | yes | `Keyword::Interface`/`Trait` in subset. |
| `implements` | yes | yes | yes | yes | yes | yes | — |
| Generics with `from` constraints | yes | yes | yes | yes | partial | partial | `fase-3-verificacion-constraints`; `from` verified at use and in the body, declared variance verified positionally; generic contract/enum dispatch still pending. |
| Records | yes | yes | yes | yes | yes | yes | — |
| Tuples (literals, `.N` indexing, `match` destructuring) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`. |
| Algebraic enums with data | yes | yes | yes | yes | yes | yes | Includes recursive/mutual declaration order. |
| Enum static members (`E.count`, `E.keys()`, `E.values()`, `E.from_name()`, `E.from_value()`, `E.to_string`, `Enums.*`) | yes | yes | yes | yes | yes | yes | `enum-static-members`; `count`/`keys`/`values`/`to_string` lower to constants, the lookups expand to a comparison chain returning `Result<E, LookupError>`. `values()`/`from_*` apply to traditional enums only. |
| `static` class members (`static` methods, `static` fields, `ClassName.member`) | yes | yes | yes | yes | yes | yes | `fase-3-miembros-y-defaults`; statics excluded from vtables, contract tables, record equality, and clone graphs. |
| Field default initializers (`field: T = expr;`) | yes | yes | yes | yes | yes | yes | `fase-3-miembros-y-defaults`; evaluated in declaration order before the constructor body, unless the constructor assigns the field. |
| `as` casts | yes | yes | yes | yes | yes | yes | — |
| `as?` nullable casts | yes | yes | yes | yes | yes | yes | `fase-3-cierre-contratos`; returns `T?` with `null` on mismatch. |
| `TraitName.super.method()` | yes | yes | yes | yes | yes | yes | `fase-3-cierre-contratos`; explicit trait default selection. |
| Type-parameter defaults (`<T = Int32>`) | yes | yes | yes | yes | yes | yes | `fase-3-cierre-contratos`; defaults filled at use sites and checked against `from` constraints. |
| Contract `implements` clauses | yes | yes | yes | yes | yes | yes | `fase-3-cierre-contratos`; parsed, DAG checked, transitive conformance enforced. |
| Comparison operator contracts (`<`, `<=`, `>`, `>=`) | yes | yes | yes | yes | yes | yes | `fase-3-cierre-contratos`; `_less`/`_less_equal`/`_greater`/`_greater_equal` reserved methods. |
| `abstract class` dynamic dispatch | yes | yes | yes | yes | yes | yes | `fase-3-abstract-dispatch`. |
| Structural equality on records | yes | yes | yes | yes | yes | yes | `fase-3-structural-equality`. |
| `record` contract dispatch | yes | yes | yes | yes | yes | yes | `fase-3-value-type-contract-dispatch`. |
| `type` alias lowering | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`; an alias resolves to its underlying type through the whole pipeline and is usable in executable programs. |
| `value class` | n/a | n/a | n/a | n/a | n/a | n/a | **Removed** by `array-list-tuple-duration-regex`; migrate to `record` (or `class` when identity/mutability is wanted). |
| Derived `Clone` for `record`/`enum` | n/a | n/a | yes | yes | yes | yes | `fase-3-miembros-y-defaults`; automatic deep clone when the reachable type graph is cloneable; no `derive` keyword. |
| Generic contract lowering (`class`/`record` implements `Contract<T>`) | yes | yes | partial | partial | partial | partial | `fase-3-dispatch-generico` + `fase-3-cierre-contratos`: user-defined `class`/`record` generic `implements Contract<T>` lowers and dispatches; generic contract composition (`interface B<T> implements A<T>`) substitutes transitively; trait default bodies under substitution and user-defined generic function bodies remain pending. |

## Phase 3b — Scalars and text

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| All 10 integer widths | yes | yes | yes | yes | yes | yes | `Int8`–`Int128`, `UInt8`–`UInt128`; `Type::from_name` resolves. |
| Exact base-ten `Float` | yes | yes | yes | yes | yes | yes | `Base::Decimal`; 128-bit coefficient + scale, no `NaN`/infinity; the fractional default. Division precision uses exact decimal magnitude and integer powers retain the full `i128` exponent. `0.1 + 0.2 == 0.3`. |
| `Float16`/`Float32`/`Float64`/`Float128` | yes | yes | yes | yes | yes | yes | `Float` aliases `Float64`; IEEE 754, `f` literal suffix. The old `BinaryFloat*` spellings no longer resolve (redirect diagnostic). |
| `Char` as grapheme | yes | yes | yes | yes | yes | yes | `ADR-014`; runtime exposes grapheme helpers. |
| Bitwise/shift operators | yes | yes | yes | yes | yes | yes | `TokenKind` `Amp`/`Pipe`/`Caret`/`Tilde`/`Shl`/`Shr`. |
| Final-result scalar conversion | yes | yes | yes | yes | yes | yes | `T(expr)` evaluates `expr`, then converts its completed value. Float-to-integer native conversion checks finiteness and exact target bounds before LLVM conversion; interpreter and native boundary failures are compared. |
| String interpolation | yes | yes | yes | yes | yes | yes | `to_string()` contract. |
| `Float128` on Windows | yes | yes | yes | yes | partial | partial | Excluded from Windows corpus; crashes the MSVC linker due to soft-float lib calls. |
| Contextual numeric literal typing / mixed-width arithmetic | yes | yes | yes | yes | yes | yes | Every width resolves; literals take the destination context where applicable; binary operators promote to the smallest common numeric type, except integer `/`, which returns `Decimal`. `Float.format(spec)` is implemented with `spec` syntax `[0][width][.precision][e|E]`; `Float128` `to_string()`/`format()` still truncates to `Float64` (lossy for values not exactly representable in `f64`). |
| `String` methods and mutation | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex` plus `native-type-member-surface`: `s[i] = c` write with grapheme-cache invalidation, `[start:end:step]` slicing, `trim`/`search`/`contains`/`starts_with`/`ends_with`/`substring`, `length`/`byte_length`/`is_empty`, `find()`, `replace()`, `trim_start()`/`trim_end()`, `to_lowercase()`/`to_uppercase()`, `normalize()`, `split()`/`split_whitespace()`/`lines()`, `bytes()`/`codepoints()`/`chars()` views, `clone()`, and `to_string()`. |
| `Char` classification and normalization | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex` plus `native-type-member-surface`; `byte_length`, `codepoint_count`, `ascii_code()`, `is_ascii()`, `is_alphabetic()`, `is_numeric()`, `is_alphanumeric()`, `is_letter()`/`is_digit()` aliases, `is_whitespace()`, `is_uppercase()`, `is_lowercase()`, `to_uppercase()`, `to_lowercase()`, `normalize()`, and `to_string()`. |

## Phase 4a — Expected errors

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `Result<T,E>` | yes | yes | yes | yes | yes | yes | Native `enum Result<T,E>`; mandatory consumption. |
| `_ = expr` discard | yes | yes | yes | yes | yes | yes | — |

## Phase 4b — Exceptions

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `throws`/`try`/`catch`/`finally` | yes | yes | yes | yes | yes | yes | `Keyword::Throws`/`Try`/`Catch`/`Finally` in subset. |
| `throw`, rethrow | yes | yes | yes | yes | yes | yes | — |
| `fatalError` / `Never` | yes | yes | yes | yes | yes | yes | — |
| Typed catch dispatch | yes | yes | yes | yes | yes | yes | `Throwable` hierarchy. |
| Catchable implicit native errors | yes | yes | yes | yes | yes | yes | All six concrete `RuntimeError` subclasses (div0, range shift, negative repeat, `NaN`, arithmetic overflow, invalid cast) are catchable; `ArithmeticOverflowError` and `InvalidCastError` closed the last gap. Fixtures in `crates/zirk-cli/tests/corpus/`. |
| `Throwable.suppressed()` | yes | yes | yes | yes | yes | yes | Runtime links a new exception thrown inside a `catch` to the exception being handled. |
| `Throwable.stack_trace()` | yes | yes | yes | yes | yes | yes | Lazily builds and caches a `String` per exception object. |
| Deep immutability of caught objects | yes | yes | yes | yes | yes | yes | Caught exception bindings are `inmut::strict`; writes and mutable aliases through their reachable graph are rejected. |

## Phase 4c — Resources

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `match ... with` | yes | yes | yes | yes | yes | yes | Single-resource and grouped `match ... with` parse, type-check, and lower; left-to-right acquisition and right-to-left cleanup are implemented. |
| `Resource<E>` contract | yes | yes | yes | yes | yes | yes | `register_native_resource_contract`; the base `Resource<E>` contract is fully usable. `ResourceFailure` merging and cancellation-aware cleanup are implemented end-to-end. |
| Single-resource cleanup | yes | yes | yes | yes | yes | yes | `fase-4c-recursos`. |
| Grouped acquisition / transfer | yes | yes | yes | yes | yes | yes | Grouped `match ... with` acquisition/transfer parses, type-checks, lowers, and runs end-to-end. `transfer(r)` works with `TransferableResource`; use-after-transfer is rejected at compile time. `ResourceFailure` merging and cancellation-aware cleanup are implemented. |

## Phase 4d — Callable and binding completion

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `Fn(P...) => R` / `Function(P...) => R` annotations | yes | yes | yes | yes | yes | yes | `fase-4d-callables`; named/capture-less lambda freely interchangeable. |
| Escaping single-capturing closure literal | yes | yes | yes | yes | yes | yes | Directly at local initializer or `return` only; D14. |
| `is` between callables | yes | yes | yes | yes | yes | yes | D15; `==`/`!=` still rejected. |
| Recursive lambda with explicit binding | yes | yes | yes | yes | yes | yes | `fase-4d-callables` corpus. |
| Multiple declarations, simultaneous assignment | yes | yes | yes | yes | yes | yes | `fase-4d-declaraciones-multiples`; `left, right = right, left` swaps. |
| General callable polymorphism | yes | yes | yes | yes | yes | yes | `MakeCallable`/`CallCallable` works end-to-end. `.clone()` on callables, GC tracking of capture blocks, and stored/returned callables are implemented. |
| Optional return annotation (inferred return types) | yes | yes | yes | yes | yes | yes | `inferred-return-types`: `fn`, methods, and trait default bodies may omit `: T`; demand-driven inference unifies the body's `return`s, detects recursion cycles, and keeps the explicit annotation authoritative. `zirk check --types` emits the declaration/expression type map consumed by the VS Code hover. |

## Phase 4e — Managed memory and unsafe

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| Non-moving mark-sweep GC | n/a | n/a | n/a | yes | yes | partial | `ADR-003` closed; `zirk_rt_alloc` now reclaims ordinary objects, `String`/`Char` handles, and cycles. |
| `Weak<T>` | yes | yes | yes | yes | yes | yes | `fase-4e-weak`. |
| Deep `clone()` for reference graphs | yes | yes | yes | yes | yes | yes | `fase-4e-clone`. |
| `unsafe fn`/`unsafe {}` | yes | yes | yes | yes | yes | yes | `Keyword::Unsafe` in subset. |
| `Pointer<T>` | yes | yes | yes | yes | yes | yes | `fase-4e-unsafe-pointer-extern`. |
| `NativeSlice<T>` / `NativeSliceMut<T>` | yes | yes | yes | yes | yes | yes | `fase-4e-native-slice`; also delivered `expr[index]`. |
| `extern "C" fn` | yes | yes | yes | yes | yes | yes | `ADR-015`; `Keyword::Extern` in subset. |
| `commit {}` | yes | yes | yes | yes | yes | yes | `Keyword::Commit` in subset. |
| Transactional unsafe journal/rollback | yes | yes | yes | yes | yes | yes | `fase-4e-unsafe-journal`; `phase-4e-pending-closeout` closed `return`/`break`/`continue` early-exit rollback. |
| `Pointer.from` on `record` fields | yes | yes | yes | yes | yes | yes | `phase-4e-pending-closeout`. |
| `String[index]` grapheme access | yes | yes | yes | yes | yes | yes | `phase-4e-pending-closeout` delivered read access; `array-list-tuple-duration-regex` adds `s[i] = c` write. |
| Dependent references | yes | yes | yes | yes | yes | yes | `Dependent<T>` lifetime and escape analysis rejects returns, field stores, captures, and non-dependent parameter passing; valid local use runs end-to-end. |
| Automatic bounded native pinning | yes | yes | yes | yes | yes | yes | `Pin(c)` constructs a `Pin<T>`, automatic unpin provides field/method access, and reassignment of the pinned variable is rejected. |

## Phase 5 — Concurrency and parallelism

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| Cooperative executor + scheduler control block + timer + per-task GC roots | n/a | n/a | n/a | n/a | yes | n/a | `fase-5-executor-core` runtime infrastructure: `executor.rs` / `task.rs` / `timer.rs` and `collector.rs` per-task root chains. `zirk_rt_run_main` runs `main` as the executor's root scheduler task. The scope table, join-or-cancel-and-clean protocol, and ambient-timer registration were added by `concurrent-blocks-and-timers`. |
| removed `task` / `await` / `select` surface | n/a | targeted diagnostic | n/a | n/a | retained runtime only | yes | `remove-task-await-model`: `task`, `await`, `Task<T>`, task scope, selection, and shields were removed from lexer, AST, sema, IR, codegen, and corpus. Construct-shaped legacy syntax reports `E0318` with its replacement. |
| `concurrent { }` / `spawn` / `Job<T>` / `Timer` | yes | yes | yes | yes | yes | yes | `concurrent-blocks-and-timers`: the `concurrent` block hoists its bindings and orders branches by a Rule B dataflow DAG; `spawn` adds a dynamic branch yielding a linear must-use `Job<T>` (`wait`/`cancel`/`done`); `Timer.sleep` is a cancellation safe point, `Timer.after` / `Timer.every` are ambient branches the scope cancels on exit. IR: `ScopeEnter` / `ScopeExit` / `BranchStart` / `JobWait`; codegen emits a per-site `extern "C" fn(ptr) -> i64` branch thunk and the renamed C-ABI (`zirk_rt_spawn` / `zirk_rt_job_wait` / `zirk_rt_scope_*` / `zirk_rt_timer_*` / `zirk_rt_sleep`). A branch's cancellation or unhandled exception unwinds natively through its own `uwtable`-marked stack; `main` runs in an implicit scope when it starts a branch directly. |
| `Concurrent.of(...).first()` / `.settled()` / `.within(Duration)` | n/a | yes | yes | yes | yes | yes | `concurrency-completion`: direct homogeneous thunk pipelines, deterministic completion selection, ordered `Outcome<T>` payload roots, cleanup-aware cancellation, and timeout with caller-cancellation precedence. `.settled()` maps a thrown `CancelledError` to `Outcome.Cancelled` after cleanup; a cleanup failure becomes `Rejected`. `Void` and `Never` result types are rejected. |
| `Concurrent.each` / `Concurrent.each_settled` | n/a | yes | yes | yes | yes | yes | Native scheduler only; Array/List of at most 256 elements, input held through `inmut::strict` with a deeply `Share` element graph, default concurrency 32 and explicit limit 1..=256. Both preserve input order and validate bounds before admission. `each` cancels and joins siblings after callback failure; `each_settled` returns one typed outcome per callback. Caller cancellation joins admitted work and returns no partial list. Browser interpreter excludes native managed scopes/outcomes. |
| `parallel` | yes | yes | yes | yes | yes | yes | `parallel-cpu-regions`: `parallel { }` (`;`-separated `cores`/`chunk` header) opens a CPU region on a worker pool (`crates/zirk-runtime/src/pool.rs`), separate from the cooperative executor. `for` over `Array`/`List` work-splits through `InstKind::ParallelForStart`; ordered `map`/`filter`, `for_each`, `Parallel.each`, `sum`, and associative `reduce` use the same worker dispatch, with input-order materialization. `reduce_ordered` remains sequential. I/O/suspension is rejected in a region and mutable reference capture is rejected at its boundary. The stop-the-world safepoint collector is delivered alongside it (`ADR-019`). Language-level synchronization and full `Transfer`/`Share` enforcement remain incomplete. |
| `Thread.run(fn)` | n/a | yes | yes | yes | yes | yes | Native supported 64-bit targets only. Uses a 64-worker process-wide managed pool with 1,024 queued callbacks and cooperative backpressure. Sema enforces callback shape, complete effects, no suspension/spawn, capture ownership, and result ownership. Runtime registers worker roots and joins before releasing caller slots; codegen fails closed on Wasm and 32-bit pointer ABIs. Blocking FFI whose effects/collector transition cannot be proven remains rejected; browser interpreter excludes the operation. |
| `Mutex<T>` / `mutex.with` | n/a | yes | yes | yes | yes | yes | Native managed mutex with inline, descriptor-traced payload. Only `with` exposes a writable borrowed view; sema rejects escape and transitive suspension/block/spawn, IR verifies region/lock/root invariants, and LLVM calls the FIFO cooperative runtime ABI. Unwind and source-exception cleanup are verified; other public synchronizers remain unavailable. |
| `Channel<T>` | yes | yes | yes | yes | yes | yes | `typed-channels`: bounded/rendezvous/growable channels, cooperative send/receive, try outcomes, closure, GC tracing, and indirect native ABI. `select`/drain follow-ups remain. |
| `Atomic<T>` language feature | yes | yes | yes | yes | yes | yes | Native source API and managed lowering support `Boolean` and signed/unsigned 8/16/32/64-bit integers when the target provides that atomic width; the scalar payload is inline in the GC-managed object. Operations default to `SeqCst`; weaker orders require `unsafe`, and a dynamically invalid order raises `InvalidArgumentError`. Pointer atomics are unsupported. Native-host E2E coverage passes. Runtime build checks pass on nine configured LLVM triples plus `wasm32-unknown-unknown`, but are compile-only and do not establish non-host execution support. `Thread<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` remain unavailable. |
| Detached background work and supervisor | n/a | yes | yes | yes | yes | yes | `concurrency-completion`: native `Concurrent.detach` validates zero-argument `Void` callbacks and capture ownership, lowers to the bounded executor-owned supervisor, retains capture roots, reports failures through `App.on_background_error`, and maps the 256-job ceiling to `BackgroundCapacityError`. `application.spawn_service` provides readiness, dependency ordering, failure propagation, and bounded shutdown; `App.set_shutdown_grace(Duration)` configures cooperative drain and generated CLI exits 124 at deadline. Host shutdown signals are integrated. Cancellation remains cooperative and does not forcibly terminate arbitrary native work. |

## Phase 6 — Project system and CLI

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `.zkinit` / multi-file projects | yes | yes | yes | partial | no | yes | `add-packages-and-module-exports`: project discovery, package/application identity, root-confined quoted local imports, logical package imports, `share mod` roots, local/vendored graph traversal and lock metadata validation reach `check`/`build`/`run`. Registry/Git acquisition, verified archive hashes and publication remain pending. |
| `requires`/`during: build` manifest | no | no | no | no | no | no | Spec defined; not started. |

## Phase 7 — Standard library

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `Array<T>`, `List<T>` | yes | yes | yes | yes | yes | yes | `collection-sequence-pipeline` adds eager `map`/`filter`/`for_each`/`reduce`/`sum`/`count`/`collect`, membership, in-place reverse/sort, and nullable first/last/pop; existing indexing, slicing, cloning, rendering, and mutations remain supported. |
| `Map<K,V>`, `Set<T>` | yes | yes | yes | yes | partial | yes | `map-set-collections` plus `type-safety-and-runtime-hardening`; the public operations and end-to-end fixtures are present. The runtime stores one-word keys/values/elements, traces declared managed references, snapshots normalized String keys/elements to keep hashes stable after mutation, and reclaims backing storage with its managed handle. Sema rejects unsupported aggregate representations; this remains a deliberately limited implementation, not full generic collection support. |
| `Range<T>` | yes | yes | yes | yes | yes | yes | `collection-sequence-pipeline` adds eager `map`/`filter`/`for_each`/`reduce`/`sum`/`count`/`collect`; ranges retain their arithmetic properties and materialize as `List<T>` at the terminal boundary. |
| `Duration` (literals, arithmetic, printing) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`; exact signed nanosecond duration with suffixes `ns`–`w`. `native-type-member-surface` added `abs()`, `sign()`, `is_zero()`, `is_positive()`, and `is_negative()`. |
| `Date` / `Time` / `DateTime` (civil temporal) | yes | yes | yes | yes | yes | yes | `date-and-time-types` + `temporal-rich-api`: validated constructors throwing `InvalidDateError`/`InvalidTimeError`; component, calendrical (`day_of_week`, `day_of_year`, ISO `week_of_year`, `quarter`, `days_in_month`/`days_in_year`, `is_leap_year`) and sub-second (`millisecond`, `microsecond`, `nanosecond`) properties; `is_before`/`is_after`/`is_same`/`is_same_or_before`/`is_same_or_after`/`is_between`/`is_weekday`/`is_weekend` queries; validated `with_*` replacements; `start_of`/`end_of(unit)` → `Result<_, ParseError>`; strict ISO `Date.parse`/`Time.parse`/`DateTime.parse`; `format(pattern)`; `to_iso_string`; `Date ± Int`, `Date - Date`, `Time ± Duration` (wrapping with checked wide intermediates), `Time - Time`, `Date + Time`, `DateTime ± Duration`, `Date ± Duration` → `DateTime`, `DateTime - DateTime` → `Duration`; `Date.today()`/`Time.now_local()`/`now_utc()`/`DateTime.now_*`. |
| Zone-aware temporal family (`Instant`, `ZonedDateTime`, `TimeZone`, `Period`, `TimeShift`, locales) | yes | no | no | no | no | no | `pending_type` Phase 7; `date-and-time-types` + `temporal-rich-api` delivered the civil surface. |
| `Regex` (literals, `matches`, `find`, `replace`) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex` plus `native-type-member-surface`; includes `re'...'`, `matches`, `find`, `replace`, `split`, `find_all`, `Regex.parse`, and `match` integration. Runtime cache is bounded to 128 entries, 16 KiB per source pattern, and 256 KiB total retained source; compiled regex size is capped at 256 KiB. New patterns are rejected when limits are reached; entries are not evicted because handles may refer to them. |
| `std.testing` package and `zirk test` runner | no | no | no | no | no | no | The handbook defines a target contract only; no bundled `std.testing` package or test reporter exists, and the CLI returns `NOT_IMPLEMENTED` for `zirk test`. Secret redaction in test reports is not implemented or verified. |
| `std.environment` | partial | partial | partial | partial | partial | partial | The scoped `deliver-std-http-and-environment` feature is complete: bundled `Env`/`Environment` provide exact-grant reads and queries, lexicographically ordered names, compile-time `EnvironmentParsable<T>` parsing, and redacted `SecretString`. Approved reads fail closed on stale grants. GitHub Actions run 39359107965 passes the full native workspace matrix on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; each target passes 155/155 CLI E2Es. Secret values have explicit HTTPS-header and authenticated-proxy sinks; debugger views, snapshots, test reports, and other future reveal sinks remain outside this delivery. The overall standard library is still partial. See [the environment handbook](../handbook/04-standard-library/18-std-environment.md) and archived `deliver-std-http-and-environment` evidence. |
| `std.net` | partial | partial | partial | partial | partial | partial | The baseline feature delivers bundled IP/address values, bounded DNS, TCP streams/listeners, and authenticated TLS client/server APIs with exact grants, peer identity validation, cancellation, and owner-task cleanup. The `complete-stdlib-networking-and-http` change adds bounded UDP, local IPC, interface snapshots, configurable TLS, and controlled shutdown. Focused native conformance passes on all targets in run 39739088146; full per-target suite evidence passes across runs 39739088146 and 39702042337, with the changed Windows regression test also passing in 39748669844. Browser socket APIs remain excluded and the overall standard library is still partial. See [the network handbook](../handbook/04-standard-library/11-std-net.md). |
| `std.http` | partial | partial | partial | partial | partial | partial | The baseline includes HTTP/1.1 and TLS HTTP/2 clients/servers, QUERY (RFC 10008), bounded bodies and streaming, redirects, cookies, caching, retries, proxies, routing/middleware, SSE, and HTTP/1.1 plus HTTP/2 WebSocket paths. The `complete-stdlib-networking-and-http` change adds optional native HTTP/3/QUIC client and server paths, RFC 9220 WebSocket, validated server options, and shareable controlled shutdown across TCP/TLS/HTTP/3. Full workspace and independent-peer interoperability evidence covers macOS ARM64 and both Linux targets in run 39739088146 and Windows x86_64 in run 39702042337; the current Windows regression-test change passes focused run 39748669844. Browser Fetch passes; browser listeners and sockets are excluded. The overall standard library is still partial. See [the HTTP handbook](../handbook/04-standard-library/12-std-http.md). |

### Result and nullable standard-library outcome consistency (working tree, 2026-10-22)

The active OpenSpec change `unify-result-nullable-collection-access` makes
Array/List/Map `get` the single safe lookup and represents HTTP/TLS successful
absence with non-null outcome enums. Compiler sema and IR crates pass
`cargo check`; compiled CLI `check` and `build` accept typed Result lookups,
nullable projection, the new HTTP/TLS signatures, and their outcome enums.
Runtime behavior and the existing
workspace suite have not been verified for this change. See
[ADR-025](../decisions/ADR-025-unified-result-and-nullable-stdlib-outcomes.md).

## Native standard-library target verification (2026-10-20)

GitHub Actions run [39359107965](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39359107965), on compiler revision `b28daa73379a527159c9c303509632bd3ecf413a` (runtime/test revision `73a8bbccbf878b85f6b093de4e279cc033d73d22`), passes the full serial workspace suite on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. Each target passes 155/155 compiled-Zirk CLI E2E tests. `zirk-runtime` passes 498 tests with 3 ignored and `zirk-runtime-core` passes 164 with 1 ignored on macOS and both Linux targets; Windows passes 500/3 ignored and 165/1 ignored, respectively. Generic response-producer backpressure fixtures pass for HTTP/1.1 and HTTP/2 on Unix and HTTP/2 on Windows; the Windows HTTP/1.1 socket-buffer fixture remains Unix-gated.

This closes the native execution gates for the archived `deliver-std-http-and-environment` baseline: exact-grant environment access, TCP/DNS/TLS, HTTP/1.1, TLS HTTP/2, QUERY, streaming and the listed server protocols. The UDP, local sockets, interface enumeration, HTTP/3/QUIC, browser networking, and secret-redaction follow-ups below describe the separate active extension; the broader Phase 7 standard-library surface remains incomplete.

## Current networking extension evidence (working tree, 2026-10-21)

The active OpenSpec change `complete-stdlib-networking-and-http` adds UDP, local
IPC, interface snapshots, extended TLS, optional HTTP/3 and HTTP/3 WebSocket,
and injected browser Fetch. These changes are still uncommitted and have not
passed the complete native CI matrix. Evidence gathered on macOS ARM64:

- `cargo test -p zirk-runtime local_transport::tests -- --nocapture`: 5 local
  transport tests passed; the Zirk CLI Unix-socket exchange E2E passed.
- `python3 scripts/test-http3-websocket-public-api.py`: HTTP/3 WebSocket echo
  and close passed between two compiled Zirk programs and in both directions
  with independent `aioquic` 1.7.0 peers; an unapproved Origin receives 403
  before the Zirk handler runs. The Zirk HTTP/3 client also completed GET and
  QUERY against aioquic, with the peer validating the query body and content
  type and the client confirming the negotiated HTTP/3 response protocol.
- `cargo test -p zirk-runtime --features http3 --lib
  http3_transport::tests -- --test-threads=1`: 15 HTTP/3 transport tests passed,
  including HTTP/3 handler cancellation after the configured drain deadline.
- The independent `aioquic` peer also verifies HTTP/3 response QPACK limits:
  17 bounded response fields exceeding the aggregate limit produce a typed
  `Protocol` error. It verifies that an unsupported RFC 9220 WebSocket peer
  returns `Protocol` without cross-version replay. A focused runtime test
  verifies proxy policy: `Auto` retains its ordinary proxy configuration while
  forced HTTP/3 refuses an unsupported non-MASQUE proxy.
- `cargo check -p zirk-runtime --features http3 -p zirk-ir
  -p zirk-codegen-llvm -p zirk-sema -p zirk-cli` passed. Compiled-Zirk E2Es
  passed for cross-task TCP shutdown, idempotent shutdown/options validation,
  and the official HTTP/3 server binding. TCP and TLS accept-wakeup runtime
  tests also passed and confirmed listener-port release.
- `scripts/test-browser-http-fetch.sh`: the `wasm32-unknown-unknown` build and
  injected-Fetch checks passed, including GET, POST, QUERY, origin denial,
  body limits, CORS failure, and cancellation.
- `cargo test -p zirk-browser -- --test-threads=1`: 41 passed, 1 ignored.
  `cargo deny check all` and `cargo audit` passed for the current lockfile.
- `cargo check -p zirk-runtime --features http3 --target
  x86_64-pc-windows-msvc` could not run on this Mac because the Windows C
  compiler/SDK is unavailable (`assert.h` was not found). This is not Windows
  build evidence; the configured Windows CI runner must provide it.
- CI run 39558631485 failed in the Windows CLI unit suite because two bundled
  standard-package tests compared Windows `\\` path separators with `/`-based
  package-relative names. The loader now normalizes the relative path before
  matching; both affected tests pass locally on macOS. The current fix still
  requires a new Windows CI run.

The follow-up browser preflight suite now also covers both shared HTTP server
control instructions as explicit native-only capabilities. On 2026-10-21,
`cargo test -p zirk-interpreter -p zirk-browser -- --test-threads=1` passed
(65 interpreter tests, 38 browser unit tests, 3 browser corpus tests; one
browser test ignored), `cargo check -p zirk-browser --target
wasm32-unknown-unknown` passed, and `cargo test -p zirk-sema` passed 688 tests.
Compiled-Zirk E2Es passed for validated server options, cross-task shutdown,
and the official HTTP/3 server binding after rebuilding the default-feature
runtime archive. An initial standalone CLI failure came from the previous
HTTP/3-enabled static archive being linked into the default-feature compiler;
the rebuilt default archive produced the expected result.

## Current networking extension evidence (2026-10-22)

GitHub Actions run [39739088146](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39739088146)
passed the focused networking and HTTP/3 conformance step on macOS ARM64,
Linux x86_64, Linux ARM64, and Windows x86_64. Complete workspace suites and
public independent-peer HTTP/3 WebSocket interoperability also passed on both
Linux targets and macOS ARM64; the browser Fetch job passed. Windows passed the
same complete suite and peer step in [run 39702042337](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39702042337)
at `634179a`; the only code difference in `2c06c67` is the test-only
deterministic repair, verified on Windows by [focused run 39748669844](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39748669844).
The latest push run [39752131334](https://github.com/ChristianSmith18/zirk-lang/actions/runs/39752131334)
passes Linux x86_64, Linux ARM64, macOS ARM64, and browser Fetch. Its Windows
job passes the main test suite and networking conformance, then exits with
`WinError 5` while the public HTTP/3 WebSocket interoperability script removes
its temporary `program.exe`. The log does not show a protocol assertion failure,
but this run is not green; Windows HTTP/3 WebSocket remains pending a clean CI
rerun. The previous Windows full suite and peer interoperability passed in
run 39702042337, and the deterministic regression test passed in focused run
39748669844. The complete per-target evidence is therefore composite across
runs rather than one green matrix. OpenSpec tasks 2.8 and 4.6 are implemented;
the Windows cleanup result remains an outstanding CI issue. The recorded macOS
ARM64 release-size comparison remains the only final linked application size
measurement; non-macOS final executable sizes are not measured. The companion
website was synchronized and built against compiler docs commit `d760edd`; the
final OpenSpec close-out commit advances its provenance manifest.

## Phase 7b — Functional style

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `fn gen` / `yield` | yes | no | no | no | no | no | `Keyword::Gen`/`Yield` gated to Phase 7b. |
| `|>` pipe | yes | no | no | no | no | no | `TokenKind::PipeGt` gated. |
| `map`/`filter`/`reduce` | no | no | no | no | no | no | Spec defined; not started. |

## Phase 8+ — Packaging, DX, metaprogramming, production

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `.zpkg` / package manager | partial | partial | partial | no | no | partial | Package API/artifact inspection scaffolding exists, but publication remains gated on reproducible archives and cryptographic integrity verification. |
| LSP / formatter / debugger | n/a | n/a | n/a | n/a | n/a | partial | `zirk-format` provides canonical whole-document formatting to the CLI, `zirk-lsp`, and browser package; the LSP MVP also serves diagnostics, hover, and document symbols over stdio. Incremental snapshots, completion, navigation, rename, range formatting, and a debugger remain Phase 9. |
| Decorators (`fn dec`) | yes | no | no | no | no | no | `Keyword::Dec` gated to Phase 10. |

---

## Notes on evidence sources

- Lexer: `crates/zirk-lexer/src/token.rs` (`Keyword::phase`, `TokenKind::phase`); a construct whose keyword has `phase() == None` is recognized and lexed.
- Parser: `crates/zirk-parser/src/parser.rs` gates unimplemented syntax with `NOT_IMPLEMENTED`; a `yes` means the real production exists and has a parser test.
- Sema: `crates/zirk-sema/src/checker.rs` and `crates/zirk-sema/src/types.rs` (`pending_type`); a `yes` means the type resolves and no `PENDING_FEATURE`/`NOT_LOWERED` diagnostic is emitted for ordinary use.
- Lowered: `crates/zirk-ir/tests/lowering.rs` and `crates/zirk-ir/src/lower.rs` gate unsupported constructs with `NOT_LOWERED`.
- Runtime: `crates/zirk-runtime` and `crates/zirk-codegen-llvm/tests/emission.rs`.
- CLI/Tooling: `crates/zirk-cli/tests/end_to_end.rs` and the `corpus/valid`/`corpus/invalid` fixtures.
- Standard-library documentation: the current `std.net`, `std.http`, and
  `std.environment` handbook chapters contain 10 canonical complete Zirk
  examples that pass `zirk check`; 32 additional fences are intentional
  fragments and are excluded from standalone compilation.
- Dependency/security audit for `deliver-std-http-and-environment` (2026-10-18):
  `cargo deny check` passes advisories, bans, licenses, and sources under the
  explicit permissive allowlist with narrowly scoped Unicode exceptions;
  `cargo audit --no-fetch` reports no vulnerability findings across 219 locked
  dependencies. The macOS ARM64 optimized executable-size regression passes.
  A fresh `hello.zrk` application is 3,626,712 bytes (3.459 MiB), below the
  5 MiB macOS ceiling. The Linux ARM64 release compiler size is separately
  recorded in the OpenSpec task evidence; the complete target size matrix is
  still open.
- Contextual Result projection refresh (2026-10-22): `Result<T,E>` remains intact in explicit `match` and typed contexts; ordinary value use projects to `T?`, and redundant extraction APIs plus `Env.get_or`/`Env.get_or_null` are removed. Native macOS ARM64 passes `cargo fmt --all -- --check`, semantic typing (647), IR lowering (171), CLI check (45), browser corpus parity, native LLVM/interpreter numeric parity, focused environment approval/fail-closed E2Es, and the full `cargo test --locked --workspace -- --test-threads=1` suite (2,852 passed, 16 ignored across 75 suites). The full run initially exposed five E2E callers that still inferred nullable values or used removed `unwrap`; they were migrated to explicit Result handling and all five now pass. The browser corpus records native-slice fixtures as excluded because `unsafe` journal blocks remain unsupported there. Linux and Windows native Result parity remain unverified. See [ADR-024](../decisions/ADR-024-contextual-result-value-projection.md) and the active `contextual-result-value-projection` change.
- Current hostile-input/security matrix refresh (2026-10-18): `cargo test
  --locked -p zirk-runtime -p zirk-runtime-core -- --test-threads=1` passes
  1,019 tests with 12 ignored across 28 suites on native macOS ARM64, and the
  full compiled-Zirk CLI E2E suite passes 155/155. This refresh covers the
  redirect, DNS-rebinding, proxy, cookie, credential-redaction, TLS-identity,
  cancellation, and memory-bound regressions; native Linux x86_64 and Windows
  execution remain separate open gates.
- Full compiler workspace refresh (2026-10-18): `cargo test --locked
  --workspace -- --test-threads=1` passes 2,786 tests with 16 ignored across
  75 suites on native macOS ARM64, including compiler, runtime, and CLI E2E
  coverage. This does not close the Linux x86_64 or Windows native execution
  gates.
- Windows x86_64 MSVC compile-only refresh (2026-10-18): runtime/runtime-core
  test targets and the no-LLVM CLI checker test target compile through
  `cargo xwin`; no Windows binary has been executed.
- Linux x86_64 Docker emulation refresh (2026-10-18): after fixing the signed
  approval fixture's isolated toolchain `PATH` and extending the cache fixture's
  accept deadline for emulated compiler startup, the complete current CLI E2E
  suite passes 155/155 in 834.21 seconds. An earlier concurrent run showed
  Rosetta guard-page and fixture-startup failures; a clean serial rerun passed
  all cases. This is emulation evidence only; native Linux x86_64 execution
  remains open.
- Linux x86_64 Docker runtime refresh (2026-10-18): the complete
  `zirk-runtime` and `zirk-runtime-core` package test suites exit successfully
  with the pinned LLVM 20 toolchain path; runtime passes 498 with 3 ignored and
  runtime-core passes 164 with 1 ignored, including integration and doc tests.
  A prior runtime attempt aborted once with a stack-guard `ENOMEM`, but the
  direct full-suite rerun passed. These results use emulation and do not replace
  native Linux x86_64 execution.
- Linux x86_64 Docker checker/compiler refresh (2026-10-18): CLI package checks
  pass 40/40, including bundled `std.http`, `std.environment`, and `std.net`
  declarations. The `zirk-sema`, `zirk-ir`, and `zirk-codegen-llvm` test suites
  also exit successfully, including host object emission. These tests run under
  Rosetta emulation and are not native Linux execution.

## Recommended future direction

Maintaining this table by hand is the source of the drift this change fixes.
The long-term solution is to generate the catalog from the code:

1. Extract `Keyword::phase()` and `TokenKind::phase()` into a known map.
2. Extract the `pending_type` table from `crates/zirk-sema/src/types.rs` and the
   `NOT_IMPLEMENTED`/`PENDING_FEATURE`/`NOT_LOWERED` diagnostics from the parser,
   checker, and lowerer.
3. For each named feature, query whether it has a valid `.zrk` fixture in
   `crates/zirk-cli/tests/corpus/valid`, an IR lowering test in
   `crates/zirk-ir/tests/lowering.rs`, and an emission test in
   `crates/zirk-codegen-llvm/tests/emission.rs`.
4. Fail a documentation lint if the catalog and the code disagree.

That pipeline is compiler/tooling work and is not implemented here; this file
is intentionally single and manually maintained until that change is proposed.
