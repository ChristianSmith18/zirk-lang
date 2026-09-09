## Context

The repository already contains a `RangeExpr`, a `Range<T>` semantic type, LLVM lowering for range loops and range values, and a collector-tracked runtime representation. Those pieces currently implement a different contract: the parser accepts `start..end..step`, omitted steps lower to `+1`, and `.reverse()` remains a special method. Collection construction is also split between constructor calls and an intended but incomplete `[...]` literal surface.

The target contract must be consistent across source grammar, AST, semantic checking, IR, runtime, tests, and public documentation. Ranges are integer arithmetic sequences for this feature. Existing duration-range behavior must either be migrated deliberately or remain a separately specified compatible extension; it must not be silently changed by the integer-bound validation.

## Goals / Non-Goals

**Goals:**

- Define `start..end`, `start..=end`, `start..end:step`, and `start..=end:step` as the only range construction forms.
- Require braces around every non-literal bound or step expression and type-check those expressions as compatible integers.
- Infer the omitted step from bound direction and support explicit positive and negative steps with checked zero-step failure.
- Make range loops, range values, array literals, list literals, and `Array(...)`/`List(...)` calls share one expansion and evaluation model.
- Make `[...]` default to `Array<T>` and use destination context to select `List<T>`.
- Support fixed-size declarations such as `Int32[6]` without conflating allocation with initializer-driven length.
- Remove obsolete range builder methods and update all normative and public status material.
- Preserve deterministic left-to-right evaluation and ensure each dynamic expression is evaluated once.

**Non-Goals:**

- Nested collection syntax such as `[[range]]` as a special range feature.
- Changing slice syntax `[start:end:step]`; slice steps remain independent from range steps.
- Introducing a general spread operator for arbitrary iterables.
- Changing unrelated collection mutation, ownership, iterator invalidation, or `Iterable<T>` rules.
- Adding a new runtime dependency.

## Decisions

### Canonical AST representation

Keep one `RangeExpr` node with `start`, `end`, optional `step`, and `inclusive`. The parser consumes a single colon for an optional step after the end expression. Braced operands are represented as their inner expression plus source metadata indicating that the operand was explicitly braced, allowing diagnostics to distinguish a missing brace from a type error.

Alternatives considered: retaining a second `..` delimiter would preserve compatibility but perpetuate ambiguity with nested ranges and contradict the requested syntax; storing raw text would move validation out of the parser and weaken diagnostics.

### Direction and step normalization

When `step` is omitted, lowering computes `+1` when `end > start` and `-1` when `end < start`; equal bounds produce an empty exclusive range and a one-element inclusive range. When `step` is explicit, its sign controls the comparison direction and must be compatible with the bounds. A non-zero step whose sign cannot reach the end is a compile-time error for constant operands and a controlled runtime range-direction error for dynamic operands. Zero always raises `InvalidStepError` before iteration or collection construction.

Alternatives considered: always defaulting to `+1` is the current bug; silently reversing an explicitly contradictory step hides mistakes and makes the sign meaningless.

### Contextual collection literals

Parse `[...]` as a collection literal with a sequence of scalar elements and range-expansion elements. Semantic analysis chooses `Array<T>` when no expected collection type exists and chooses `List<T>` when the expected type is `List<T>`. An `Array<T>` initializer has fixed length equal to the fully expanded element count; a `List<T>` initializer creates a resizable list. A range element is always expanded in place, preserving source order. No nested-range escape hatch is introduced.

`Array(...)` and `List(...)` calls use the same expansion pass for arguments. This avoids separate semantics for literal and constructor forms and guarantees `Array(0..2)` is equivalent to `Array(0, 1)`.

Alternatives considered: making `[...]` array-only would make list initialization needlessly different; treating a range as one element would contradict the requested API and require a separate spread syntax.

### Fixed-array declarations

`T[n]` declarations without an initializer reserve exactly `n` slots and retain fixed length. The implementation must use the language's existing default-initialization policy for `T`; reading a slot before initialization must not become undefined behavior. The design records this as an explicit semantic/test point rather than assuming a value in the parser.

### Removal of range methods

Delete range-specific `.step(...)` and `.reverse()` dispatch from checker and lowering, remove their runtime ABI exports once no supported caller remains, and migrate every fixture/documentation example to colon-step syntax. The `step` data member is retained only if the final `Range<T>` value API still requires read-only introspection; it is not a builder method.

### Evaluation and lowering

Literal loops may lower directly to a counter loop. Range values and collection expansion lower through a shared normalized range sequence helper. Start, end, step, and each non-range collection element are evaluated once, left to right, before allocation/iteration begins. Expansion computes capacity with checked arithmetic and reports an allocation/length error instead of overflowing.

## Risks / Trade-offs

- **[Risk]** Existing source using `..step` or `.reverse()` breaks. → **Mitigation:** provide precise migration diagnostics and update all repository fixtures and handbook examples in the same change.
- **[Risk]** Dynamic bounds can make array capacity unknown until runtime. → **Mitigation:** allocate after expansion with checked length arithmetic; keep `List` growth semantics separate.
- **[Risk]** Sign-conflicting dynamic steps could create empty or non-terminating loops. → **Mitigation:** validate direction before entering the loop and reject zero/contradictory steps deterministically.
- **[Risk]** Braces conflict with block syntax in parser recovery. → **Mitigation:** add a dedicated operand parser with balanced-brace diagnostics and focused parser tests.
- **[Risk]** Removing runtime symbols may break generated code from older compiler versions. → **Mitigation:** treat the ABI change as a compiler/runtime lockstep migration and verify no current lowering path references removed symbols.
- **[Risk]** Public status may claim delivery before end-to-end LLVM verification. → **Mitigation:** update feature status only after parser, sema, IR, runtime, and CLI corpus tests pass; record LLVM/toolchain limitations honestly and synchronize the companion site.

## Migration Plan

1. Land parser/AST and semantic diagnostics with compatibility tests that identify legacy forms.
2. Land lowering/runtime normalization and collection expansion behind the new grammar.
3. Migrate fixtures and public examples; remove obsolete method tests.
4. Run component tests and LLVM-backed CLI tests, then update status/roadmap documentation.
5. Synchronize `../zirk-lang-site` from the committed source revision.

Rollback is a source-level revert of the change commit; no persistent data migration is involved.

## Open Questions

- Should a dynamic explicit step whose sign conflicts with its bounds be a dedicated `InvalidRangeDirectionError`, or should it produce an empty range?
- What exact default value/read-before-write behavior should apply to an allocation-only `T[n]` array for every supported element family?
- Does the final integer-only range contract intentionally remove `Range<Duration>`, or should duration ranges retain the same colon-step and direction rules as a compatible extension?
