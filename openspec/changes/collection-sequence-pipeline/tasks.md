## 1. Sema: sequence pipeline on `List<T>`/`Array<T>`

- [ ] 1.1 `crates/zirk-sema/src/checker.rs`: extend `check_array_list_method_call` with `map(fn: (T): R): List<R>`/`Array<R>` (matching the receiver's own family), `filter(fn: (T): Boolean)` (same family), `for_each(fn: (T): Void): Void`
- [ ] 1.2 `reduce(identity: R, combine: (R, T): R): R`, `sum(): T` (numeric element type only, compile-time error otherwise), `count(): Int32`, `collect(): List<T>`
- [ ] 1.3 Checker tests: `map`/`filter` type and family preservation, `reduce`/`sum`/`count`, `collect`, `sum` on a non-numeric element type rejected

## 2. Sema: query and mutation companions on `List<T>`/`Array<T>`

- [ ] 2.1 `contains(value: T): Boolean`, `reverse(): Void`
- [ ] 2.2 `sort(): Void` (element type must have a natural ordering — reuse the comparison-operator widening rules; compile-time error directing to `sort_by` otherwise), `sort_by(cmp: (T, T): Int32): Void`
- [ ] 2.3 `first(): T?`, `last(): T?`, and `pop(): T?` (`List<T>` only)
- [ ] 2.4 Checker tests: `contains`, `sort` acceptance/rejection, `sort_by`, `reverse`, `first`/`last`/`pop` on empty and non-empty receivers

## 3. Sema: `Range<T>` and `Tuple`

- [ ] 3.1 New `check_range_method_call`: `map`/`filter`/`reduce`/`sum`/`count`/`for_each`/`collect`, matching task 1's signatures, `collect(): List<T>`
- [ ] 3.2 New `check_tuple_method_call`: `to_string(): String`, `clone(): Tuple(...)`, arity as a compile-time constant
- [ ] 3.3 Checker tests: `Range<T>` pipeline + `collect`, `Tuple.to_string`/`clone`/arity, `Tuple` rejects `map`/`filter`/etc.

## 4. IR + codegen

- [ ] 4.1 `crates/zirk-ir/src/lower.rs`: lower `map`/`filter`/`for_each` as a loop calling the lambda per element (into a fresh result for `map`/`filter`, discarded for `for_each`)
- [ ] 4.2 Lower `reduce`/`sum`/`count` as an accumulating loop; `collect` reusing the existing collection-construction lowering
- [ ] 4.3 Lower `contains`/`first`/`last`/`pop`/`reverse` against the existing `List<T>`/`Array<T>` buffer primitives (`crates/zirk-runtime/src/list.rs`/`array.rs`)
- [ ] 4.4 `Range<T>`'s pipeline: materialize the range's values (reusing the existing range-loop lowering), then the same per-method lowering as task 4.1/4.2
- [ ] 4.5 `Tuple.to_string`/`clone`/arity lowering
- [ ] 4.6 IR + codegen golden tests for each new lowering shape

## 5. Runtime: `sort`/`sort_by`

- [ ] 5.1 `crates/zirk-runtime/src/list.rs`/`array.rs`: `zirk_rt_list_sort`/`zirk_rt_array_sort`-style C-ABI entry points (design's own open question — resolve there whether this is a native helper or an inline-emitted algorithm, and update `design.md` if the answer differs)
- [ ] 5.2 Runtime unit tests: sort correctness and stability, `sort_by` with a custom comparator, empty-receiver edge cases

## 6. Fixtures + docs

- [ ] 6.1 `valid/*.zrk` for each new method (pipeline, companions, `Range<T>`, `Tuple`)
- [ ] 6.2 `invalid/*.zrk`: `sum()` on a non-numeric element type, `sort()` on a type without a natural ordering, `Tuple.map` (rejected)
- [ ] 6.3 Handbook `12-collections`/`13-iteration-and-functional-style`: document every new method
- [ ] 6.4 `docs/init/ZIRK_FEATURE_STATUS.md`: `List<T>`/`Array<T>`/`Range<T>`/`Tuple` rows updated for the new methods

## 7. Closeout

- [ ] 7.1 `cargo test --workspace`; `cargo fmt --check`; `cargo clippy --workspace --all-targets`
- [ ] 7.2 Commit `zirk-lang`; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review the status catalog; commit `../zirk-lang-site` separately; record both revisions
- [ ] 7.3 `openspec validate collection-sequence-pipeline --strict`
- [ ] 7.4 `openspec/changes/parallel-cpu-regions/tasks.md` + `design.md`: mark that tasks 5.2/5.3's dependency is resolved and resume them against this change's `reduce`/`.parallel` call sites
