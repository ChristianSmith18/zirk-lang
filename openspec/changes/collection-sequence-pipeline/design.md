## Context

`zirk-collections` already commits to an eager/lazy boundary — "collection
transformations SHALL materialize eagerly, iterator adapters SHALL remain
single-pass and lazy, and materialization SHALL require explicit `collect`
or a typed `to_*` terminal" — but no concrete method exists yet to test that
contract against. `List<T>`/`Array<T>` today have exactly `add`/`insert`/
`remove`/`clone`/`to_string` plus `length`/`is_empty` and indexing/slicing
(`crates/zirk-sema/src/checker.rs`'s `check_array_list_method_call`);
`Range<T>` has no method surface at all beyond being iterable in a `for`
loop; `Tuple` has construction, indexing, destructuring, and pattern
matching, nothing else.

This was found while implementing `parallel-cpu-regions`: its own tasks 5.2
(`.parallel` adapter typing, `ParallelSeq<T>`) and 5.3 (associativity check
on a parallel `reduce`) are blocked on exactly this gap — there is no
`.reduce`/`.map`/etc. call site to attach either check to. That change's
`design.md` "Implementation addenda" section and `tasks.md` 5.2/5.3 record
the finding; this change is the fix, and `parallel-cpu-regions` names it as
a dependency to resume against.

## Goals / Non-Goals

**Goals**: a real, eager sequence pipeline on `List<T>`/`Array<T>`
(`map`/`filter`/`reduce`/`sum`/`count`/`collect`/`for_each`), the common
companions (`contains`/`sort`/`sort_by`/`reverse`/`first`/`last`/`pop`), an
equivalent read-only pipeline on `Range<T>`, and `Tuple` parity methods
(`to_string`/`clone`/arity) — enough that `parallel-cpu-regions` 5.2/5.3 have
something concrete to build on.

**Non-Goals**: lazy iterator adapters as their own type (`zirk-collections`
reserves that distinction for later — everything here is eager, matching
what already exists); `Map<K,V>`/`Set<T>` pipeline parity (out of scope —
`map-collections`/`set-collections` are their own capabilities and can adopt
this same shape in a follow-up); actually parallelizing any of this (that is
`parallel-cpu-regions` 5.2/5.3's own job, once this lands); a general
`Iterable<T>`/`Iterator<T>` contract users can implement (`zirk-collections`
names `Iteration<T>.Item/Done` already; this change adds concrete methods on
the four built-in families, not a new user-facing contract).

## Decisions

### D1: One eager method table per family, mirroring `check_array_list_method_call`'s own shape

`List<T>`/`Array<T>` share one dispatch table today (`check_array_list_method_call`)
because they share a representation closely enough (`crates/zirk-runtime/src/list.rs`/
`array.rs` both use the same `element_pointer`-style layout). The new
sequence-pipeline methods extend that same table rather than a separate one,
so `[1,2,3].map(f)` and `list.map(f)` share one implementation and one set of
diagnostics. `Range<T>` gets its own table (`check_range_method_call`) since
it has no backing buffer to index into — each method there first
materializes the range's own values (the same "for i in a..b" counter
lowering already does) before running the pipeline, i.e. `range.map(f)` is
sugar for "iterate, apply, collect into a `List<T>`". `Tuple` gets a third,
much smaller table, since its elements are not one uniform type.

**Alternative**: a single polymorphic `Iterable<T>` dispatch shared by all
four. Rejected for this change — `zirk-collections` explicitly defers a real
iterator contract, and forcing one now to save a bit of table duplication
would be exactly the kind of premature abstraction the four families don't
need yet (each already has its own concrete representation).

### D2: `reduce`'s signature and the associativity obligation

`reduce(identity: R, combine: (R, T): R): R` — eager, strict left-to-right
fold, matching a sequential reduce in any mainstream language and giving
`sum`/`count` an obvious definition in terms of it (`sum` is
`reduce(0, (a,b) => a+b)` for a numeric element type; `count` needs no
combiner at all — it is just `length` for anything already materialized).
No associativity is required or checked for the *sequential* `reduce` this
change adds — associativity only matters once a caller tries to parallelize
the fold, which is `parallel-cpu-regions` 5.3's own concern, not this
change's. This change deliberately does not implement that check; it only
makes sure `reduce` exists so 5.3 has a call site to attach it to. 5.3's own
design addendum already records the intended heuristic (reject a combiner
lambda whose top-level operator is not one of `+ * & | ^ && ||`) for
whoever implements it there.

**Alternative**: require an explicit `Associative` marker on `combine`.
Rejected — nothing else in Zirk's type system has an operator-property
marker, and 5.3 already settled on a syntactic heuristic instead.

### D3: `first`/`last`/`pop` are nullable, not exceptions

`first()`/`last()` on an empty `List<T>`/`Array<T>`/`Range<T>` return `T?`
(`null` on empty); `pop()` (`List<T>` only — `Array<T>` is fixed-size)
likewise returns `T?`. This matches `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`'s
own precedent (`get_or_null` "MAY deliberately collapse absence with
nullable value") rather than the *direct*-access precedent (`zirk-collections`:
"Direct missing index/key access SHALL produce a typed controlled
exception") — `first`/`last`/`pop` are explicitly asking "is there one?",
not asserting one exists the way `list[i]` does.

**Alternative**: throw an exception on empty, matching direct indexing.
Rejected — `first`/`last`/`pop` read as a query, not an assertion; forcing a
`try`/`catch` (or a length check immediately before every call) for the
overwhelmingly common "process while there's something left" loop shape
would fight the caller rather than help them.

### D4: `sort`/`sort_by` mutate in place; `reverse` too

Matching `add`/`insert`/`remove`'s own in-place style on `List<T>` (and
`Array<T>`, which already supports in-place element writes via
indexing). `sort()` requires the element type to have a natural ordering
(numeric, `String`, `Char`, `Boolean` — the same widening/ordering rules
comparison operators already use); `sort_by(cmp: (T, T): Int32)` takes an
explicit comparator for anything else. Both return `Void`; call `.clone()`
first for a sorted copy, matching the existing `List.clone()`/`Array.clone()`
precedent for "I want an independent copy first."

**Alternative**: return a new sorted collection, leaving the receiver
untouched (a "functional" style). Rejected — inconsistent with every other
`List<T>`/`Array<T>` mutator this compiler already has, all in-place.

## Risks / Trade-offs

- **Scope creep into `Map<K,V>`/`Set<T>`.** → Mitigation: explicitly a
  non-goal (D1's own alternative note); a follow-up change can adopt the same
  table shape once this one is landed and reviewed.
- **`sort`'s ordering rules for a mixed/generic element type** could reveal
  gaps in the comparison-operator widening rules this reuses. → Mitigation:
  reject (compile-time) any element type without a natural ordering when
  `sort()` (no comparator) is called, directing the caller to `sort_by`.
- **`reduce`'s call site existing but its associativity check not landing
  here** means `parallel-cpu-regions` 5.3 is still a second, separate piece
  of work after this lands, not automatically finished by it. → Mitigation:
  explicit in this design's Non-Goals and in the proposal's own "What
  Changes"; `parallel-cpu-regions`'s `tasks.md` records the two-step
  dependency so it is not mistaken for done once this change closes.
- **Public status/website drift** if `ZIRK_FEATURE_STATUS.md` is updated
  without the matching `../zirk-lang-site` sync. → Mitigation: same
  `./scripts/sync-website-content.sh --audit-date` step every other change
  in this codebase already uses; call it out in this change's own closeout
  task.

## Migration Plan

1. Extend `check_array_list_method_call` with the sequence pipeline and
   companions on `List<T>`/`Array<T>`; add `check_range_method_call` and
   `check_tuple_method_call`.
2. IR lowering for each: `map`/`filter`/`for_each` as a loop calling a
   lambda per element into a fresh (for `map`/`filter`) or discarded (for
   `for_each`) result; `reduce`/`sum`/`count` as an accumulating loop;
   `collect` reusing the existing collection-construction lowering;
   `sort`/`sort_by`/`reverse`/`contains`/`first`/`last`/`pop` against the
   existing `List<T>`/`Array<T>` buffer primitives.
3. Codegen for whatever the IR step needs that does not already exist
   (expected: nothing new — these all lower to existing loop/call shapes).
4. Runtime: only if sorting is implemented as a native helper rather than
   lowered inline (open question below) — a `zirk_rt_list_sort`-style
   C-ABI entry point alongside `crates/zirk-runtime/src/list.rs`'s existing
   ones.
5. Fixtures (valid/invalid corpus) + handbook chapters 12/13 + feature
   status + website sync.
6. `parallel-cpu-regions`: resume tasks 5.2/5.3 against this.

## Open Questions

- Does `sort`/`sort_by` lower as an inline generated loop (an
  insertion/quicksort emitted by codegen) or call into a native runtime
  helper? Proposed: a native helper (`zirk_rt_list_sort`) — consistent with
  `add`/`insert`/`remove` already being native calls, and avoids duplicating
  a sort algorithm's code at every call site.
- Does `Range<T>.map`/`.filter`/etc. return `List<T>` unconditionally, or
  follow the same "contextual collection literal selection" `range-collection-expansion`
  already gives literals (defaulting to `Array` unless context selects
  `List`)? Proposed: always `List<T>` — a `Range`'s own size is not always
  known cheaply relative to a fixed-size `Array`'s allocation, so `List<T>`'s
  growable storage is the safer default; revisit if profiling shows it
  matters.
