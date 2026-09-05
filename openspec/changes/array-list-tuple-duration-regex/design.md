## Context

Zirk now has a complete scalar surface (Phase 3b), exceptions (4b), resources (4c), callables (4d), and managed/unsafe memory (4e). It is missing the everyday data types that make a language usable: variable-length collections, tuples, a primitive `Duration`, and regular expressions. The `value class` construct also exists but is half-finished and semantically redundant with `record`.

This change fills those gaps in one coordinated delivery. It deliberately does not touch concurrency, the project system, or the rest of the temporal family, keeping the scope bounded to data types and regex.

## Goals / Non-Goals

**Goals:**
- Remove `value class` from the language surface, AST, checker, IR, runtime, tests and docs.
- Add `Tuple(A, B, ...)` as a value product type with constant index access and destructuring.
- Add `Array<T>` as a contiguous native reference container with indexed read/write, bounds checking, `length`, `is_empty`, and `for ... in`.
- Add `List<T>` as a resizable native reference container with `add`, `remove`, `insert`, indexed read/write, and iteration.
- Add `Duration` as a primitive with nanosecond precision and literal suffixes (`ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w`).
- Add `Regex` as a native reference type with `re'pattern'` literal syntax, capture groups, `match`, `find`, `replace`, `split`, and `matches` iteration.
- Update `Iterable<T>` / `Iterator<T>` to be satisfied by `Array`, `List`, `String`, `Range`, and `Regex` matches.
- Add full `String` surface: write `s[i] = c`, slicing `[start:end:step]`, and methods `split`, `trim`, `search`, `contains`, `starts_with`, `ends_with`, `substring`.
- Add `Char` classification and normalization: `is_uppercase`, `is_lowercase`, `to_uppercase`, `to_lowercase`, `is_digit`.
- Add `Range<T>` generic with `start`, `end`, `step`, `reverse()`, and slicing.
- Lower `type alias` so aliases are usable in executable programs.
- Derive `Clone` for `record` and `enum` values when every field is `Clone`.
- Lower generic contract satisfaction so user-defined generic `class`/`record` can implement `Iterable<T>` and similar generic contracts.
- Deliver end-to-end CLI corpus fixtures and update public docs.
- Synchronize `../zirk-lang-site` after commit.

**Non-Goals:**
- `Map<K,V>` and `Set<T>`.
- `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Period`.
- Concurrency primitives (`Task`, `Channel`, `Thread`, `Atomic`).
- Decorators, package manager, LSP, or reflection.
- `Float128` exact decimal formatting or Windows soft-float verification.
- `Map<K,V>`, `Set<T>`, `Date`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Period`.

## Decisions

### 1. Remove `value class` instead of finishing it
**Rationale**: `value class` is currently a compact, method-less `record`. `record` already provides value semantics, structural equality, named construction, and methods. Adding method bodies and `implements` to `value class` would duplicate effort without adding a new semantic capability. Java and C# ship with `class` and `record`/`struct`; `value class` is not needed for "full OOP". Removing it shrinks the language surface and eliminates a half-finished feature.

### 2. `Array<T>` and `List<T>` are native reference types, not library classes
**Rationale**: Collections are listed as "native reference types" in `ZIRK_LANGUAGE_SPEC.md`. The runtime already has a GC and object headers. Implementing them as compiler-known native types keeps indexing, bounds checking, and iteration efficient and lets `for ... in` dispatch through the existing `Iterable<T>` contract. The `Expr::Index` dispatch table added in `fase-4e-native-slice` is reused for `Array`/`List` without new grammar.

### 3. `Tuple` is a value product type with compile-time constant index
**Rationale**: Heterogeneous product types are essential for destructuring and multi-return. Tuples are copied by value, have no identity, and are indexed by a compile-time constant `tuple[n]`. A new `Base::Tuple` variant in `zirk-sema` is the cleanest way to represent them; they are not generic classes because their size and layout are fixed by the element types.

### 4. `Duration` is a primitive, not a class
**Rationale**: `Duration` has literal syntax (`250ms`, `2h`) and needs to behave like `Int` or `Float` in arithmetic. Implementing it as a `Base` primitive (internally an `i64` of nanoseconds) avoids GC overhead and makes `Duration` operations as cheap as integer operations. It satisfies `to_string()`, comparison, and a closed set of arithmetic operators.

### 5. Use the `regex` crate for the regex engine
**Rationale**: Writing a regex engine from scratch is out of scope. The `regex` crate is the standard Rust engine, is well-audited, and provides the matching/capture API Zirk needs. The `Regex` object is a native reference type that owns a compiled `regex::Regex` (or `regex::Regex` with captures) in the runtime.

### 6. `Regex` match results iterate via `Iterable<Match>`
**Rationale**: Zirk already has `Iterable<T>` / `Iterator<T>` contracts. `Regex.matches(text): Iterable<Match>` and `Regex.captures(text): Iterable<Captures>` reuse that mechanism instead of inventing a new iteration pattern. `Match` is a value record with `text`, `start`, and `end`; `Captures` is a value record with positional and named access.

### 7. `String` and `Char` methods are native operations, not contract dispatch
**Rationale**: `String` and `Char` are core types with a closed, compiler-known operation set. `String` already has `to_string` and `clone`. Adding `split`, `trim`, etc. as native methods keeps performance predictable and avoids requiring every `String` to carry a vtable. `Char` classification can call into `std` Unicode tables.

### 8. `Range<T>` is a value generic with a single representation
**Rationale**: `Range<T>` stores `start`, `end`, and `step` of type `T` (only numeric or `Duration` initially). Keeping it as a value record with derived `Iterable<T>` avoids a special-case runtime object.

### 9. `type alias` lowers to its target type
**Rationale**: An alias is transparent in every pipeline stage after name resolution. The only missing piece is lowering: the IR treats the alias as the resolved type, so `type UserId = Int32;` in a variable declaration just becomes `Int32`.

### 10. `Clone` is derived for `record` and `enum` by field-walk
**Rationale**: Records and enums are already unboxed value types. `Clone` derivation mirrors the existing `class` `Clone` derivation but without heap allocation or cycle detection. A record/enum containing a non-`Clone` field is rejected with the same diagnostic as `class`.

### 11. Generic contract satisfaction uses the existing `ContractInstance` path
**Rationale**: `Base::ContractInstance` already exists for `Iterable<Int32>`-typed references. Lowering a generic `class Box<T> implements Iterable<T>` requires the contract method body to substitute `T` at the call site. Reuse the `specialize`/`substitute` infrastructure from `fase-3-generic-substitution-recursion` rather than building a new dispatch mechanism.

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| `Tuple` requires new IR/LLVM value layout and may interact badly with existing record/class lowering | Reuse the unboxed value path already used by `Record`; add a dedicated `LoweredTuple` helper that treats the tuple as an LLVM struct. |
| `List` resizable growth in the GC requires a `realloc` path the runtime does not have yet | Add `zirk_rt_realloc` for objects the collector owns; prove it does not move the object if other roots exist (or move and update all references if the GC becomes compacting later, but keep non-moving for now). |
| `Duration` arithmetic overflow / underflow | Define overflow as a controlled runtime error, the same as integer overflow; clamp to `i64` nanoseconds. |
| `regex` crate dependency may complicate cross-compilation and target matrix | Use `regex` only at runtime; the compiler generates calls to `zirk_rt_regex_*` functions. Keep the dependency version stable and check it on the target matrix. |
| `value class` removal may break existing test corpus and docs | Audit all `.zrk` fixtures and docs for `value class`; migrate to `record`. Update `ClassKind::ValueClass` removal in parser/checker/lower. |
| `Array`/`List` generic element type with `Iterator<T>` may expose the pending generic contract dispatch gap for user-defined `Iterator` | Only native `Iterable<T>` satisfaction is implemented. User types already implement `Iterable` via the existing contract mechanism. |
|| `String` write/slice/methods require grapheme-aware mutations and bounds checks | Reuse the existing `String` grapheme index/cache; for `s[i] = c`, recompute the cache on mutation; for slices, allocate a new `String` and copy the selected grapheme range. |
|| `Range<T>` generic over `Float`/`Duration` complicates the iterator | Specialize `Range` iteration for numeric and `Duration` `T`; other `T` values reject `Range` construction at compile time. |
|| `type alias` lowering may interact with generic instantiations | Resolve aliases before specialization; an alias to `Box<Int32>` behaves exactly like `Box<Int32>`. |
|| `Clone` derivation for records/enums with `class` or non-`Clone` fields | Reject the same way `class` `Clone` does when a field is not `Clone`; skip reference-backed fields for `enum` variants. |
|| Generic contract satisfaction may need deep `substitute` recursion | Add a recursion cap and reuse the existing `substitute` memoization; test with `Box<T> implements Iterable<T>`. |

## Migration Plan

1. Remove `value class` parser production, AST node, checker handling, lowering, and tests.
2. Add `Tuple` AST/type/lowering before `Array`/`List` because `Array` and `List` methods may return `Tuple` for `get`/`remove` or `Iterator`.
3. Add `Duration` to the lexer, parser, AST, `Base`, `zirk-ir`, `zirk-runtime`.
4. Add `Array` and `List` types, runtime allocation, indexing, and `Iterable` wiring.
5. Add `Regex` type, literal, and runtime integration.
6. Add CLI corpus fixtures for all new features.
7. Update docs, `ZIRK_FEATURE_STATUS.md`, `07-current-limitations.md`.
8. Commit, sync `../zirk-lang-site`, archive the OpenSpec change.

## Open Questions

1. Should `List<T>` support `inmut` iteration by default, or should `List.iterator()` return a mutable `Iterator<T>` that can also be used for `remove`? (Recommendation: `Iterable<T>`/`Iterator<T>` are read-only; mutation goes through `add`/`remove` on the `List` itself.)
2. Should `Duration` support fractional literals (`1.5s`) or only integer literals (`1500ms`)? (Recommendation: support fractional seconds, minutes, and hours only; `ns`/`us`/`ms` are integer.)
3. Should `Tuple` support named fields (`record` already does) or only positional? (Recommendation: only positional; use `record` for named fields.)
