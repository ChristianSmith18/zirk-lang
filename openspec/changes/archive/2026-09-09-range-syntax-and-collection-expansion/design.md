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

## Resolved Decisions (Task 1.1)

The three open questions below are resolved as follows and are now normative for this change.

### RD1 — Contradictory explicit step direction

A non-zero explicit step whose sign cannot reach `end` from `start` is an error, never a silently-empty range.

- **Constant operands:** compile-time error (`codes` entry `E04xx`, message "the step of this range moves away from its end"), reusing the constant sign/bound contradiction check in task 3.1.
- **Dynamic operands (any braced non-literal bound or step):** a dedicated controlled runtime failure `InvalidRangeDirectionError`, raised by the shared range-normalization helper (task 4.1) *before* any iteration or collection allocation begins, alongside the existing `InvalidStepError` for a zero step.

`InvalidRangeDirectionError` is a new sibling of `InvalidStepError` in the runtime's controlled-error set; it is catchable with the same machinery and carries the evaluated `start`, `end`, and `step`.

### RD2 — Allocation-only `T[n]` initialization

An allocation-only `inmut buffer: T[n];` zero-initializes every slot to `T`'s zero/default representation (universal zero-init):

- scalar integer / float / boolean / char families → numeric zero / `false` / `\0`;
- `String` → the empty string;
- `record` / class types → their all-defaults value when every field has a default, otherwise a compile-time error naming the first field with no default;
- reference/nullable types → `null`.

Reads of an unwritten slot are therefore always well-defined; there is no read-before-write analysis for fixed arrays. The zero value is materialized by the same runtime array-construction path used for expanded literals (task 4.5), not decided in the parser.

### RD3 — `Range<Duration>` retention

`Range<Duration>` is retained as a compatible extension, not removed. The "integer-compatible operand" rule generalizes to "valid range element type", whose members are the integer scalar families and `Duration`. Duration ranges obey the same colon-step spelling, inferred direction, zero-step rejection, and `InvalidRangeDirectionError` rules; only the legacy `..step` spelling and `.step(...)` / `.reverse()` methods are removed for them too. Duration bounds/steps written as non-literals still require braces.

## Compatibility Inventory (Task 1.2)

Every current reference to a form this change replaces or removes:

### Legacy `start..end..step` (second `..` step)

- `crates/zirk-parser/src/parser.rs:3283` — `parse_range` consumes a second `..` as the step. Replace with optional `:step`; a second `..` becomes a migration diagnostic.
- `crates/zirk-sema/src/checker.rs:10256-10312` — `check_range` doc and logic mention `start..end..step`; step already type-checked generically, only the doc/AST source changes.
- `crates/zirk-ir/src/lower.rs:5268`, `:6975`, `:2820` — range value/loop lowering reads `start`/`end`/`step` as written; direction inference for an omitted step must be added (task 4.1/4.2).
- Fixtures using the legacy spelling: `crates/zirk-cli/tests/corpus/valid/range_ops.zrk:6,14`, `range_duration.zrk:2,6`, `range_types.zrk:9`. Migrated in task 6.3 / 5.4.

### `.reverse()` range-builder method

- `crates/zirk-sema/src/checker.rs:15465` (dispatch), `:11643-11659` (member diagnostic listing `reverse()`).
- `crates/zirk-ir/src/lower.rs:12995-13015` (`lower_range_reverse`, `is_range_reverse`), `:1056` (extern decl `zirk_range_reverse`).
- `crates/zirk-runtime/src/range.rs:153-173` (`zirk_range_reverse`), `crates/zirk-runtime/src/lib.rs:90` (re-export).
- Fixture: `crates/zirk-cli/tests/corpus/valid/range_ops.zrk:22`.
- Docs: `docs/01_plantilla_zirk.md:6369`, `docs/handbook/02-handbook/05-operators-and-expressions/10-ranges.md:29,34`, `docs/handbook/02-handbook/12-collections/06-ranges.md:12,20`, `docs/handbook/04-standard-library/06-std-collections.md:25`, `docs/handbook/11-reference/05-grammar-summary.md:41`, `docs/init/ZIRK_ROADMAP.md:463`.

### `.step(...)` range-builder method

- Not currently dispatched in checker/lower (no `zirk_range_*` symbol); only referenced in prose `docs/01_plantilla_zirk.md:6368,6374`. Task 3.5 adds an explicit rejection with migration guidance so the documented form fails cleanly.

### Range interpolation / braced dynamic operands

- No current support: `parse_range` calls `parse_binary(0)` for each operand and any bare identifier bound already parses today (e.g. `range_types.zrk` uses `1 as Int64 .. 5 as Int64`). The new contract *requires* braces for non-literals — task 2.3 adds the balanced-brace operand parser and task 3.1 enforces it. Existing brace-free non-literal bounds in fixtures/docs must be rewritten to `{ ... }` (tasks 6.1/6.3).
- `crates/zirk-ast/src/lib.rs:1057-1067` (`RangeExpr`) — needs explicit-brace metadata per operand (task 2.1).

### Range value member surface (retained)

- `start` / `end` / `step` read-only members: `crates/zirk-sema/src/checker.rs:11651`, `crates/zirk-ir/src/lower.rs:14677-14679`, `crates/zirk-runtime/src/range.rs:88-120`. Retained as read-only introspection (design "Removal of range methods").
- Range slicing `r[start:end:step]`: `crates/zirk-ir/src/lower.rs:16518`, `crates/zirk-runtime/src/range.rs:187` (`zirk_range_slice`). Retained unchanged (non-goal: slice syntax).

### Collection literal `[...]` / `Array(...)` / `List(...)`

- No AST node or parser path exists (`[` in `parser.rs:3921` is postfix index/slice only). `Array<T>` / `List<T>` are not checker types today (only `NativeSlice<T>` from Phase 4e, plus incipient `crates/zirk-runtime/src/array.rs` / `range.rs`). Tasks 2.4, 3.2-3.4, 4.3-4.5, 5.x build these.

### Public status / roadmap

- `docs/init/ZIRK_ROADMAP.md:463` states range `start`/`end`/`step`, `.reverse()` and slicing are still pending — reword in task 6.2.

## Open Questions

*(All resolved — see "Resolved Decisions" above.)*
