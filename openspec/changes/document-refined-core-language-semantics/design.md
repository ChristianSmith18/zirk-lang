## Context

The current handbook combines mature type-system documentation with phase-limited callable and object rules, shallow generic/collection chapters, and examples that predate the authorial decisions accepted in August 2026. Agents are already implementing Phase 3, so documentation must distinguish final language semantics from temporary delivery limits without forcing later memory or concurrency choices into the current compiler phase.

## Goals / Non-Goals

**Goals:**

- Establish one coherent final-language contract for callables, projections, objects, generics, algebraic data, collections, iteration, and matching.
- Make every surprising rule implementable through normative prose, testable OpenSpec scenarios, valid/invalid examples, API tables, and cross-links.
- Align active Phase 3 artifacts while preserving their implementation boundary: final syntax may be documented before its compiler phase arrives.
- Preserve the source-of-truth order: normative language spec, main OpenSpecs, standard-library contract, handbook, authorial inventory, archived history.

**Non-Goals:**

- Implement compiler, runtime, memory manager, or collection code.
- Finalize the unaccepted error/exception round or later resources, unsafe memory, concurrency, and decorators.
- Require the current Phase 3 implementation to deliver escaping closures, managed views, full collection APIs, or generators ahead of their roadmap phases.

## Decisions

### D1 — Writable callables use `Fn(...) => R`

`Function(...) => R` is the native long name and `Fn` its conventional alias. Function declarations, lambdas, bound/unbound methods, and explicit callable objects adapt structurally when the complete signature is compatible. Lambda expression identity remains nominal internally; parameters are contravariant and results covariant. Expected failure remains part of the return through `Result`, not a separate throws clause.

### D2 — Closures escape under compiler-managed representation

Closures may cross parameters, returns, attributes, and collections. Immutable captures are logical snapshots; a mutable binding written by one or more closures is lifted to one shared managed cell. Whole-reference capture shares the referent. Assignment shares a closure environment; `clone()` deep-clones it when all captures are cloneable. Escape analysis selects stack/inline or managed storage without observable differences.

### D3 — Projection reads copy; places retain storage identity

Reading an attribute, index, slice, destructured component, pattern binding, return projection, argument projection, or captured projection produces an independent logical value. Reference results therefore require a deep `Clone`. Passing, assigning, returning, or capturing a whole reference variable shares it. The same projection used as a place (`items[i].name = ...`) mutates original storage. This replaces accidental nested aliasing with a syntactic, auditable boundary.

### D4 — Objects expose attributes and methods, not properties

There is no property declaration or implicit getter/setter dispatch. APIs use ordinary `get_` and `set_` methods. Omitted attributes receive their type default; explicit initializers override it and construction may finalize `inmut` attributes. Concrete classes extend at most one concrete class. Abstract classes are nominal requirement sets adopted with `implements`; interfaces specify behavior; state-free traits may also provide behavior. `override` exists only as `override fn`.

### D5 — Generics remain explicit and monomorphized

`T from A & B` combines constraints. Inference uses arguments, receiver, expected result, callable context, and constraints but never guesses. Trailing defaults are allowed. Declarations are invariant unless marked `out` or `in`; `Fn` retains its intrinsic variance. Recursive constraints and managed `Box<T>` are allowed, while associated and higher-kinded types remain out of scope. Portable IR retains generic identity and final builds specialize safely.

### D6 — Algebraic data is value-oriented and behavior remains external for enums

Tuples use `Tuple(A, B)` and constant indexing `value[n]`; records are nominal immutable values with defaults and methods; enums are closed data declarations with no user methods. Traditional enums expose native name/mapping access without implicit conversion. Algebraic enum payloads are extracted only through `match`. Unions normalize duplicates/subsumed members and expose only compatible common capabilities.

### D7 — Matching is exhaustive and guard-free

Closed domains require exhaustive statement and expression matches. Patterns cover values, types, enums, unions, regex, alternatives, and nested structures; guards are excluded. Demonstrably unreachable branches are errors. Pattern/destructuring bindings follow projection-copy semantics, and enums are never destructured outside `match`.

### D8 — Collections share at the container boundary and copy on extraction

Array/List/Map/Set are shared native references; Range/Tuple are values. Ordered indexing accepts negative indexes. Direct access throws controlled bounds/key errors; safe APIs return typed `Result`. Slices use Python-style omitted component defaults but explicit out-of-range bounds remain errors, return deep independent collections, and require equal length for slice assignment. Iteration yields copies through `Iteration<T>`, structural mutation invalidates iterators deterministically, and explicit read-only views are the only sharing projections introduced here.

## Risks / Trade-offs

- **Implicit deep copy on projection can be expensive** → Document complexity, require `Clone`, permit elision only when observably equivalent, and reserve explicit views for sharing.
- **Final callable semantics supersede Phase 3 D9** → Mark D9 as a delivery limitation and prevent Phase 3 tasks from silently implementing later memory work.
- **Abstract classes adopted with `implements` differ from mainstream OOP** → Define them narrowly as nominal attribute/method requirement sets with no constructors, bodies, or layout contribution.
- **Variance increases checker complexity** → Keep invariance default and validate every `in`/`out` occurrence at declaration time.
- **Deep collection extraction conflicts with ordinary iterator-reference expectations** → Make ordinary iteration copy and defer mutable iteration until the memory/view model is specified.
- **Large cross-cutting documentation diff** → Use a canonical ownership map, repository-wide contradiction searches, link/fence audits, and strict OpenSpec validation.

## Migration Plan

1. Add delta requirements and mark accepted final-language semantics independently from implementation status.
2. Update canonical language and stdlib documents, then active Phase 3 planning constraints.
3. Rewrite handbook units and reference tables from canonical decisions.
4. Regenerate navigation and audit every published page, link, example, and superseded phrase.
5. Validate all OpenSpecs strictly. Archive/sync only after all documentation tasks complete.

Rollback is a documentation/spec revert; no runtime or persisted-data migration occurs.

## Open Questions

This change intentionally left error, `Result`, exception, resource,
memory/unsafe, concurrency, and decorator semantics to later rounds. Errors,
resources, and permissions were subsequently accepted in
`document-errors-resources-and-permissions` and compose with `Fn`,
projection-copy, `Clone`, views, and iterator cleanup defined here.
