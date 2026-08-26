## Context

`Checker::register_class` (`checker.rs:2127`) mints a class's own type-parameter ids and pushes a placeholder `ClassType` into `self.classes` immediately, ahead of any field resolution — "a field elsewhere in the file may write `Box<Int32>` before `Box` itself is declared" (`checker.rs:2151-2155`). `Checker::declare_class_members` (`checker.rs:2504`) resolves actual field types in a later pass, by which point every class (including a self-referencing one) is already a resolvable name. `Checker::declare_enum` (`checker.rs:3261`) has no equivalent split: it mints type-parameter ids, then resolves every variant's associated field types (`checker.rs:3338`), all in one pass, only pushing the finished `EnumType` into `self.enums` at the very end — so nothing inside `declare_enum` can ever name the enum currently being declared.

## Goals / Non-Goals

**Goals:**
- A self-referencing enum declaration, generic or not, can be declared: `enum IntList { Nil, Cons(head: Int32, tail: IntList) }` and `enum Tree<T> { Leaf, Node(value: T, left: Tree<T>, right: Tree<T>) }` both resolve.
- Declaration order between an enum and a class (or another enum) that reference each other does not matter, mirroring how class-to-class forward references already don't matter today.
- Once declared, a recursive (generic or plain) enum specializes and lowers correctly through the existing `specialize_enum`/IR pipeline — this change's own closing verification, not deferred.

**Non-Goals:** nested generic instantiation in a non-recursive payload (`fase-3-generic-substitution-recursion`'s own concern).

## Decisions

### D1: Split `declare_enum` into `register_enum` (name, arity, type params) and `declare_enum_variants` (variant field resolution), run in the same two passes classes' own registration/member-resolution already run in

`register_enum` mints type-parameter ids the same way `declare_enum` already does today (this part is unchanged, `fase-3-generic-enums`'s own fix already made this correct — `mint_type_param_ids` inline, not `enter_type_params`) and pushes a placeholder `EnumType` (name, arity, type params, empty `variants`) into `self.enums` immediately. `declare_enum_variants` — called in the same later top-level pass `declare_class_members` already runs in, confirmed against the actual current top-level declaration-pass driver during implementation rather than assumed — resolves each variant's associated field types and populates the placeholder's `variants` field, by which point every class's and every enum's name (including the one currently being resolved) is already registered.

Alternative considered: give `declare_enum` a lighter-weight "just register the name, resolve variants lazily on first reference" scheme instead of a dedicated second pass. Rejected — classes already establish the two-clean-passes pattern for exactly this problem; introducing a second, different resolution strategy (lazy-on-first-use) for enums specifically would be inconsistent with the existing precedent and harder to reason about (lazy resolution order depends on reference order, which is exactly the ordering-independence this change is trying to guarantee).

### D2: `specialize_enum` itself needs no change for the direct self-reference case; verify this rather than assume it

`Tree<T>`'s own recursive field is exactly `Tree<T>` — the *same* instantiation, not a nested *different* one — so `fase-3-generic-substitution-recursion`'s own gap (nested-instantiation substitution) is not required for this case to work: `specialize_enum`'s existing memoization (confirmed by `fase-3-generic-enums`'s own IR tests, "one `EnumLayout` per distinct instantiation") should already terminate correctly once `Tree<T>` can even be *declared*, since a recursive field's own type args are identical to the instantiation already being built, not a new one requiring further substitution. This must be confirmed with a real end-to-end test (construct and pattern-match a small `Tree<T>`, at least 2 levels deep) rather than assumed correct from this reasoning alone — this was `fase-3-generic-enums`'s own originally-planned closing test, never reached because declaration itself blocked it first.

## Risks / Trade-offs

- **[Risk] Moving variant-field resolution to a later pass could interact with some other checker pass that currently assumes `declare_enum` fully populates an enum in one call** (flagged explicitly by `fase-3-generic-enums`'s own investigation as the reason this was judged out of that change's scope). → Mitigation: audit every caller/reader of `self.enums` between the old single-pass `declare_enum` call site and wherever member resolution now runs, confirming nothing reads a still-empty `variants` list during the gap between the two new phases. This is the change's own central risk and must be treated as such, not glossed over.
- **[Risk] A pathological mutually-recursive class/enum pair** (a class field naming an enum that has a variant naming the class back) might expose an ordering issue neither the class two-phase split nor this change's own mirrored split individually anticipated. → Mitigation: an explicit test for a class-and-enum mutual reference, not just enum self-reference alone.
