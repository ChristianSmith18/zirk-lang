## Why

`zirk-collections` already describes collections at a conceptual level (eager
vs. lazy materialization, `collect()` as the lazy-to-eager boundary, iterator
invalidation), but the compiler implements none of the concrete methods that
contract depends on: `List<T>`/`Array<T>` have no `map`/`filter`/`reduce`/
`sum`/`count`/`collect`/`for_each`, no `contains`/`sort`/`reverse`/`first`/
`last`/`pop`, `Range<T>` has no method surface beyond iteration in a `for`
loop, and `Tuple` has no method surface beyond construction, indexing, and
destructuring. This was discovered concretely while implementing
`parallel-cpu-regions`: that change's own tasks 5.2 (`.parallel` adapter,
`ParallelSeq<T>`) and 5.3 (associativity check on a parallel `reduce`) are
blocked because there is no `.reduce`/`.map`/etc. call site anywhere in the
checker to attach either to (see `openspec/changes/parallel-cpu-regions/
design.md`'s "Implementation addenda" section and `tasks.md` 5.2/5.3 for the
exact finding). This change is what closes that gap — and, once landed,
`parallel-cpu-regions` should return to finish 5.2/5.3 against it.

## What Changes

- Add the sequence pipeline to `List<T>`/`Array<T>`: `map(fn)`, `filter(fn)`,
  `reduce(identity, fn)` (eager, left-to-right; the checker requires an
  associative combiner when reached through a `parallel` adapter, deferred to
  `parallel-cpu-regions` 5.3 once this lands), `sum()` (numeric element types),
  `count()`, `collect()` (the lazy-to-eager terminal `zirk-collections`
  already names), `for_each(fn)`.
- Add common companions to `List<T>`/`Array<T>`: `contains(value)`,
  `sort()`/`sort_by(cmp)`, `reverse()`, `first()`/`last()` (typed `T?`, `null`
  on empty), `pop()` (`List<T>` only — removes and returns the last element,
  or `null` if empty).
- Add an equivalent read-only iteration surface to `Range<T>`: `map`/
  `filter`/`reduce`/`sum`/`count`/`collect`/`for_each`, matching `List`/
  `Array`'s own signatures, since a `Range` is already iterable in a `for`
  loop today.
- Add `Tuple` parity methods: `to_string()`, `clone()`, `length`/arity as a
  compile-time constant — matching what `List<T>`/`Array<T>` already have,
  scoped to what a fixed-arity heterogeneous value can support (no `map`/
  `filter`/`reduce` — a tuple's elements are not of one uniform type).
- Record, in `parallel-cpu-regions`'s own `tasks.md` and `design.md`, that
  tasks 5.2 and 5.3 depend on this change and should resume once it lands.

## Capabilities

### New Capabilities

(none — this fleshes out an existing capability with concrete, checkable
requirements rather than introducing a new one)

### Modified Capabilities

- `zirk-collections`: adds concrete method-level requirements for the
  sequence pipeline and its companions on `List<T>`/`Array<T>`, the
  equivalent surface on `Range<T>`, and parity methods on `Tuple` — the
  general "eager vs. lazy" and `collect()` contract this capability already
  states, made concrete enough for the checker, IR, and codegen to implement
  and verify against.

## Impact

- `crates/zirk-sema/src/checker.rs`: extends `check_array_list_method_call`
  (and a new `check_range_method_call`, `check_tuple_method_call`) with the
  new methods' typing, including the associative-combiner heuristic a
  parallel `reduce` needs (recorded but unused until `parallel-cpu-regions`
  5.3 resumes).
- `crates/zirk-ir/src/lower.rs` + `crates/zirk-codegen-llvm`: lowers each new
  method to real IR/codegen — `map`/`filter`/`for_each` as loops calling a
  lambda per element; `reduce`/`sum`/`count` as an accumulating loop;
  `collect` as the existing collection-construction path; `sort`/`reverse`/
  `contains`/`first`/`last`/`pop` against the existing `List<T>`/`Array<T>`
  buffer primitives (`crates/zirk-runtime/src/list.rs`/`array.rs`).
- `crates/zirk-runtime`: a `zirk_rt_list_sort`-style native helper if sorting
  is implemented in the runtime rather than lowered inline (design's own
  call).
- Fixtures (`crates/zirk-cli/tests/corpus/valid`/`invalid`) and handbook
  chapter `12-collections`/`13-iteration-and-functional-style` for every new
  method.
- `openspec/changes/parallel-cpu-regions/tasks.md` + `design.md`: record the
  dependency and the resume plan for tasks 5.2/5.3.
- Public documentation/status: this changes what `ZIRK_FEATURE_STATUS.md`
  reports for `List<T>`/`Array<T>`/`Range<T>`/`Tuple`, so `../zirk-lang-site`
  needs the same `./scripts/sync-website-content.sh --audit-date` workflow
  once landed.
