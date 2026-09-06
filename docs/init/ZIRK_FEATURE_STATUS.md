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

The evidence for each row is the archived OpenSpec change, the relevant
automated tests, or the source file that gates the feature. A row is updated
only when the same change that changes the code also updates this file.

Last updated with `array-list-tuple-duration-regex`, **2026-09-05**.

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
| `stdout.println` | yes | yes | yes | yes | yes | yes | Hardcoded intrinsic, not the full `std.io` contract. |

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
| Optional/named/variadic parameters | yes | yes | yes | yes | yes | yes | — |
| Compound assignment (`+=`, etc.) | yes | yes | yes | yes | yes | yes | `TokenKind` compound operators. |

## Phase 3 — Objects and type system

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `class`, `construct` | yes | yes | yes | yes | yes | yes | — |
| Visibility (`public`/`private`/`protected`) | yes | yes | yes | yes | yes | yes | — |
| Inheritance (`extends`) | yes | yes | yes | yes | yes | yes | Single inheritance; `super`/`override`. |
| `interface`/`trait` | yes | yes | yes | yes | yes | yes | `Keyword::Interface`/`Trait` in subset. |
| `implements` | yes | yes | yes | yes | yes | yes | — |
| Generics with `from` constraints | yes | yes | yes | partial | partial | partial | User generic *class*/`enum` instantiation works for flat cases; generic contract/enum dispatch still pending. |
| Records | yes | yes | yes | yes | yes | yes | — |
| Tuples (literals, `.N` indexing, `match` destructuring) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`. |
| Algebraic enums with data | yes | yes | yes | yes | yes | yes | Includes recursive/mutual declaration order. |
| `as` casts | yes | yes | yes | yes | yes | yes | — |
| `abstract class` dynamic dispatch | yes | yes | yes | yes | yes | yes | `fase-3-abstract-dispatch`. |
| Structural equality on records | yes | yes | yes | yes | yes | yes | `fase-3-structural-equality`. |
| `record` contract dispatch | yes | yes | yes | yes | yes | yes | `fase-3-value-type-contract-dispatch`. |
| `type` alias lowering | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`; an alias resolves to its underlying type through the whole pipeline and is usable in executable programs. |
| `value class` | n/a | n/a | n/a | n/a | n/a | n/a | **Removed** by `array-list-tuple-duration-regex`; migrate to `record` (or `class` when identity/mutability is wanted). |
| Derived `Clone` for `record`/`enum` | n/a | n/a | no | no | no | no | Scoped in `array-list-tuple-duration-regex`; still pending — every field must be `Clone`. |
| Generic contract lowering (`class`/`record` implements `Contract<T>`) | yes | yes | partial | no | no | no | Scoped in `array-list-tuple-duration-regex`; still pending — native `Iterable<T>` satisfaction works, user-defined generic `implements` does not lower yet. |

## Phase 3b — Scalars and text

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| All 10 integer widths | yes | yes | yes | yes | yes | yes | `Int8`–`Int128`, `UInt8`–`UInt128`; `Type::from_name` resolves. |
| `Float16`/`Float32`/`Float64`/`Float128` | yes | yes | yes | yes | yes | yes | `Float` aliases `Float64`. |
| `Char` as grapheme | yes | yes | yes | yes | yes | yes | `ADR-014`; runtime exposes grapheme helpers. |
| Bitwise/shift operators | yes | yes | yes | yes | yes | yes | `TokenKind` `Amp`/`Pipe`/`Caret`/`Tilde`/`Shl`/`Shr`. |
| Deep contextual conversion | yes | yes | yes | yes | yes | yes | `Float(3 / 4)` etc. |
| String interpolation | yes | yes | yes | yes | yes | yes | `to_string()` contract. |
| `Float128` on Windows | yes | yes | yes | yes | partial | partial | Excluded from Windows corpus; crashes the MSVC linker due to soft-float lib calls. |
| Contextual numeric literal typing / mixed-width arithmetic | yes | yes | yes | yes | yes | yes | Every width resolves; literals take the context type; binary operators promote to the smallest common numeric type. `Float.format(spec)` is implemented with `spec` syntax `[0][width][.precision][e|E]`; `Float128` `to_string()`/`format()` still truncates to `Float64` (lossy for values not exactly representable in `f64`). |
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
| `task`/`await` | yes | partial | no | no | no | no | `Keyword::Task`/`Await` gated to Phase 5. |
| `parallel`/`thread` | yes | partial | no | no | no | no | `Keyword::Parallel`/`Thread`/`Sync` gated. |
| `Channel<T>` | yes | no | no | no | no | no | `pending_type` Phase 5. |
| `Atomic<T>` | yes | no | no | no | no | no | `pending_type` Phase 5. |

## Phase 6 — Project system and CLI

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `init.zrk` / multi-file projects | no | no | no | no | no | partial | `zirk build`/`zirk run` operate on a single file today. |
| `requires`/`during: build` manifest | no | no | no | no | no | no | Spec defined; not started. |

## Phase 7 — Standard library

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `Array<T>`, `List<T>` | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex` + `native-type-member-surface`: `Array(e0, …)`/`List(e0, …)` literals, negative indexing, `Array` slicing (`[start:end:step]`), `clone()`, `to_string()`, and `List.remove(value)` added alongside `add`/`insert`/`remove(index)`. |
| `Map<K,V>`, `Set<T>` | yes | no | no | no | no | no | `pending_type` Phase 7; concrete collection objects are not implemented. |
| `Range<T>` | yes | no | no | no | no | no | Scoped in `array-list-tuple-duration-regex`; still pending — `start`, `end`, `step`, `reverse()`, slicing, and `Iterable<T>` for numeric/`Duration` `T`. |
| `Duration` (literals, arithmetic, printing) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex`; exact signed nanosecond duration with suffixes `ns`–`w`. `native-type-member-surface` added `abs()`, `sign()`, `is_zero()`, `is_positive()`, and `is_negative()`. |
| Other temporal family (`Date`, `Time`, `DateTime`, ...) | yes | no | no | no | no | no | `pending_type` Phase 7; civil/zone types beyond `Duration` are not yet available. |
| `Regex` (literals, `matches`, `find`, `replace`) | yes | yes | yes | yes | yes | yes | `array-list-tuple-duration-regex` plus `native-type-member-surface`; `re'...'` literal, `matches`, `find` (returns `Regex.Match?` with `group(n)`/`group(name)`/`start`/`end`/`text`), `replace`, `split()`, `find_all()`, `Regex.parse()`, and `match`-expression pattern integration delivered. |

## Phase 7b — Functional style

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `fn gen` / `yield` | yes | no | no | no | no | no | `Keyword::Gen`/`Yield` gated to Phase 7b. |
| `|>` pipe | yes | no | no | no | no | no | `TokenKind::PipeGt` gated. |
| `map`/`filter`/`reduce` | no | no | no | no | no | no | Spec defined; not started. |

## Phase 8+ — Packaging, DX, metaprogramming, production

| Feature | Lexer | Parsed | Sema | Lowered | Runtime | CLI | Notes |
|---|---|---|---|---|---|---|---|
| `.zpkg` / package manager | no | no | no | no | no | no | Phase 8. |
| LSP / formatter / debugger | n/a | n/a | n/a | n/a | n/a | no | Phase 9. |
| Decorators (`fn dec`) | yes | no | no | no | no | no | `Keyword::Dec` gated to Phase 10. |

---

## Notes on evidence sources

- Lexer: `crates/zirk-lexer/src/token.rs` (`Keyword::phase`, `TokenKind::phase`); a construct whose keyword has `phase() == None` is recognized and lexed.
- Parser: `crates/zirk-parser/src/parser.rs` gates unimplemented syntax with `NOT_IMPLEMENTED`; a `yes` means the real production exists and has a parser test.
- Sema: `crates/zirk-sema/src/checker.rs` and `crates/zirk-sema/src/types.rs` (`pending_type`); a `yes` means the type resolves and no `PENDING_FEATURE`/`NOT_LOWERED` diagnostic is emitted for ordinary use.
- Lowered: `crates/zirk-ir/tests/lowering.rs` and `crates/zirk-ir/src/lower.rs` gate unsupported constructs with `NOT_LOWERED`.
- Runtime: `crates/zirk-runtime` and `crates/zirk-codegen-llvm/tests/emission.rs`.
- CLI/Tooling: `crates/zirk-cli/tests/end_to_end.rs` and the `corpus/valid`/`corpus/invalid` fixtures.

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
