## 1. Sema: sequence pipeline on `List<T>`/`Array<T>`

- [x] 1.1 `crates/zirk-sema/src/checker.rs`: extend `check_array_list_method_call` with `map(fn: (T): R): List<R>`/`Array<R>` (matching the receiver's own family), `filter(fn: (T): Boolean)` (same family), `for_each(fn: (T): Void): Void`
- [x] 1.2 `reduce(identity: R, combine: (R, T): R): R`, `sum(): T` (numeric element type only, compile-time error otherwise), `count(): Int32`, `collect(): List<T>`
- [x] 1.3 Checker tests: `map`/`filter` type and family preservation, `reduce`/`sum`/`count`, `collect`, `sum` on a non-numeric element type rejected

## 2. Sema: query and mutation companions on `List<T>`/`Array<T>`

- [x] 2.1 `contains(value: T): Boolean`, `reverse(): Void`
- [x] 2.2 `sort(): Void` (element type must have a natural ordering — reuse the comparison-operator widening rules; compile-time error directing to `sort_by` otherwise), `sort_by(cmp: (T, T): Int32): Void`
- [x] 2.3 `first(): T?`, `last(): T?`, and `pop(): T?` (`List<T>` only)
- [x] 2.4 Checker tests: `contains`, `sort` acceptance/rejection, `sort_by`, `reverse`, `first`/`last`/`pop` on empty and non-empty receivers

## 3. Sema: `Range<T>` and `Tuple`

- [x] 3.1 New `check_range_method_call`: `map`/`filter`/`reduce`/`sum`/`count`/`for_each`/`collect`, matching task 1's signatures, `collect(): List<T>`
- [x] 3.2 New `check_tuple_method_call`: `to_string(): String`, `clone(): Tuple(...)`, arity as a compile-time constant
- [x] 3.3 Checker tests: `Range<T>` pipeline + `collect`, `Tuple.to_string`/`clone`/arity, `Tuple` rejects `map`/`filter`/etc.

## 4. IR + codegen

- [x] 4.1 `crates/zirk-ir/src/lower.rs`: lower `map`/`filter`/`for_each` as a loop calling the lambda per element (into a fresh result for `map`/`filter`, discarded for `for_each`)
- [x] 4.2 Lower `reduce`/`sum`/`count` as an accumulating loop; `collect` reusing the existing collection-construction lowering
- [x] 4.3 Lower `contains`/`first`/`last`/`pop`/`reverse` against the existing `List<T>`/`Array<T>` buffer primitives (`crates/zirk-runtime/src/list.rs`/`array.rs`)
- [x] 4.4 `Range<T>`'s pipeline: materialize the range's values (reusing the existing range-loop lowering), then the same per-method lowering as task 4.1/4.2
- [x] 4.5 `Tuple.to_string`/`clone`/arity lowering
- [x] 4.6 End-to-end corpus and sema coverage for each new lowering shape
- [x] 4.7 Performance specialization: `Array.map` writes directly into an exact final array; `Array.filter` preserves one-pass callback semantics.

## 5. Runtime: `sort`/`sort_by`

- [x] 5.1 Resolved in `design.md` D5: stable insertion sort is emitted inline through existing indexed load/store primitives; no additional sort C ABI is required.
- [x] 5.2 Sort correctness, custom comparator, and nullable edge behavior covered by end-to-end execution and corpus tests.
- [x] 5.3 `List` dynamically contracts after substantial removals without exposing a capacity API.

## 6. Fixtures + docs

- [x] 6.1 `valid/*.zrk` for the pipeline, companions, `Range<T>`, and `Tuple`
- [x] 6.2 `invalid/*.zrk`: non-numeric `sum`, unordered `sort`, and `Tuple.map`
- [x] 6.3 Handbook `12-collections`/`13-iteration-and-functional-style` documents the new methods
- [x] 6.4 Feature-status rows updated for `List<T>`/`Array<T>`/`Range<T>`/`Tuple`

## 7. Closeout

- [x] 7.1 `cargo test --workspace`; `cargo fmt --check`; `cargo clippy --workspace --all-targets`
- [ ] 7.2 Commit `zirk-lang`; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review the status catalog; commit `../zirk-lang-site` separately; record both revisions
- [x] 7.3 `openspec validate collection-sequence-pipeline --strict`
- [x] 7.4 Dependency notes for `parallel-cpu-regions` can now be resumed against real pipeline and `reduce` call sites.
