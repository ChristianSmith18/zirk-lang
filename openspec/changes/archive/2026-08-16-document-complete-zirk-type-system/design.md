## Context

The handbook already has chapters for everyday types, nullability, domain data types, generics, collections, iteration, and a compact built-in-type reference. Their present depth is uneven: primitive chapters are often shorter than twenty lines, operator behavior is distributed across unrelated pages, and the temporal model is limited to brief duration mentions. Meanwhile the authorial review has now settled semantics that affect the public language contract: a primitive/native/reference taxonomy, contract-based operators, `Float*` naming, deep contextual conversions, grapheme `Char`, mutable reference `String`, strict alias propagation, string repetition, and an eight-type temporal family.

This is a documentation implementation, but it cannot treat those decisions as prose-only. Canonical specs, OpenSpec requirements, the active Phase 3 change, reference tables, and teaching chapters must say the same thing. The handbook remains English-language public documentation, ordered by `SUMMARY.md`, with previous/next links on every published page.

## Goals / Non-Goals

**Goals:**

- Make the type system understandable progressively before individual APIs are introduced.
- Give every built-in type an authoritative, appropriately sized chapter covering semantics, operators, properties, methods, conversions, errors, and examples.
- Separate exact timeline arithmetic (`Duration`) from calendar arithmetic (`Period`) and separate calendar, local-time, instant, and zoned representations.
- Make mutability, reference aliasing, strictness, cloning, equality, and identity consistent across all type chapters.
- Provide compact reference matrices without forcing readers to infer behavior from them.
- Keep canonical documents, handbook prose, and OpenSpec capabilities synchronized.

**Non-Goals:**

- Implement the compiler, runtime, Unicode engine, time-zone database, or standard-library APIs.
- Promise performance characteristics that the compiler/runtime architecture has not established.
- Fully design collection APIs beyond the interactions required to explain type taxonomy, iteration, indexing, mutability, and operator availability.
- Replace tutorials with API catalogs or make every chapter the same length.

## Decisions

### D1 — Teach the model before cataloging the members

The everyday-types unit will begin with several conceptual chapters rather than one undersized hierarchy page: the type tree, primitive versus native versus user-defined types, value versus reference semantics, contracts and capabilities, conversions, and a native-operator overview. Individual type chapters then apply that vocabulary. This mirrors how mature language handbooks establish a mental model before exhaustive reference material.

### D2 — Keep one canonical chapter per semantic owner

Every rule has one primary handbook owner and other pages link to it. Numeric representation and operations belong to everyday types; reference mutability belongs to bindings and is applied by String/collection/class chapters; records/enums/unions stay in data types; the new temporal unit owns all temporal types; operator reference pages summarize rather than redefine. This prevents duplicated prose from drifting.

### D3 — Expand by semantic complexity, not a fixed template length

Every type chapter answers a common checklist—role, construction/literals, inference, representation category, mutability, conversions, operators, API, errors, examples, related types—but sections MAY be omitted when genuinely inapplicable. `Boolean` can remain compact; `String`, `Float`, `ZonedDateTime`, and `Duration` require substantially more explanation and edge cases.

### D4 — Temporal is a sealed capability family, not a license for arbitrary arithmetic

`Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period` are distinct immutable value types grouped by compiler-known `Temporal` capabilities. Documentation will show a composition matrix and reject meaningless combinations. Exact nanosecond timeline quantities use signed `Duration`; calendar years/months/weeks/days use `Period`; time zones use IANA identities and explicit DST ambiguity policies.

### D5 — Examples state both value and type

Operator and conversion examples will identify the resulting value and type whenever inference is not obvious. Deep contextual conversion receives desugaring-style explanations: `Float(a / b)` evaluates the contained arithmetic in the Float domain, and `String("value=" + n)` evaluates the contained concatenation after contextual string conversion. The boundary rules—arithmetic/concatenation tree only, no function-body traversal, no mutation of operands—are always stated.

### D6 — Native operator behavior has both teaching and reference representations

Each type chapter explains its operators with examples and edge cases. A generated-by-hand authoritative matrix in the reference unit provides a fast lookup across arithmetic, comparison, equality, identity, logical, indexing, temporal, and repetition operations. String repetition supports both `String * Integer` and `Integer * String`, rejects negative/non-integer counts, returns empty for zero, and performs checked allocation.

### D7 — Strictness is documented as an alias invariant

The binding chapters will define `mut`, `inmut`, and `inmut::strict` separately for reassignment and referent mutation. Reference-type chapters will show that a strict reference cannot yield a mutable alias and cannot be acquired while a mutable alias remains accessible; `clone()` creates an independent value when its contract is available. Temporal and other immutable value types return new values instead of exposing internal mutation.

### D8 — Canonical specs change before prose claims completion

Implementation begins by updating `ZIRK_LANGUAGE_SPEC.md`, the stdlib spec, and relevant OpenSpec main/change artifacts. Handbook chapters are then authored against that checkpoint. The final audit searches for superseded `Decimal*`, code-point `Char`, immutable-String, copy-on-write String, unsigned-only Duration, and operator statements before navigation and strict validation run.

## Risks / Trade-offs

- **The documentation can promise a very large API before implementation exists** → Label feature status independently from semantic contract and avoid invented complexity guarantees.
- **Deep contextual conversion can surprise readers from other languages** → Give it a dedicated chapter, formal boundaries, desugared examples, and invalid counterexamples.
- **Reference String plus strict aliasing implies future ownership/alias analysis** → Document the semantic invariant without prematurely fixing the compiler algorithm.
- **Temporal APIs can become another ambiguous JavaScript `Date`** → Preserve separate types and publish operator/composition matrices with DST examples.
- **Unicode grapheme behavior is complex** → Distinguish graphemes, code points, and bytes consistently and avoid claiming constant-time indexing.
- **Large API tables can drift from teaching prose** → Assign canonical owners and include cross-document contradiction checks in completion tasks.

## Migration Plan

1. Establish the canonical type and temporal semantics in language, stdlib, and OpenSpec documents.
2. Reorganize the everyday-type introduction and add the temporal unit to `SUMMARY.md` without breaking existing links.
3. Expand type chapters in coherent families, updating adjacent bindings, operator, nullability, collection, data-type, and reference pages as needed.
4. Regenerate previous/next navigation from the final order and audit all Markdown links, fences, examples, terminology, and feature-status labels.
5. Validate the OpenSpec change strictly and archive it only after every documentation task is complete.

Rollback is a documentation revert; no runtime migration occurs.

## Open Questions

- The precise complete method catalogs for collections remain a later focused design, though their interaction with the type model must be documented here.
- Locale data packaging and time-zone database update policy belong to future stdlib/runtime design; this change documents public behavior without choosing distribution mechanics.
