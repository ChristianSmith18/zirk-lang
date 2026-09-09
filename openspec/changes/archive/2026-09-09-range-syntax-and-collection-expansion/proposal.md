## Why

Zirk currently has a partially implemented range model whose syntax, semantics, compiler lowering, runtime entry points, fixtures, and documentation disagree. The compiler accepts the legacy `start..end..step` form, defaults omitted steps to `+1` even for descending bounds, and still exposes `.step()`/`.reverse()`, while the intended language contract requires `start..end[:step]`, interpolated non-literal bounds, inferred descending iteration, and explicit collection expansion.

This change establishes one coherent range contract and makes ranges useful in both arrays and lists. It is needed now because ranges are already represented in the AST, semantic checker, IR, runtime, and public specifications; leaving the layers misaligned produces empty descending loops, unsupported examples, and different behavior depending on whether a range is used in a loop, as a value, or as a collection initializer.

## What Changes

- **BREAKING** Replace the legacy `start..end..step` spelling with `start..end:step` and `start..=end:step`.
- **BREAKING** Remove `.step(...)` and `.reverse()` as range-construction mechanisms from the checker, lowering, runtime surface, specifications, examples, and fixtures.
- Keep `..` exclusive and `..=` inclusive for ascending and descending ranges.
- Infer an omitted step from the relationship between `start` and `end`: `+1` for ascending ranges and `-1` for descending ranges.
- Support positive and negative integer steps, reject zero steps with the existing controlled `InvalidStepError`, and define behavior for a step whose sign conflicts with the bounds.
- Require every non-literal bound or step expression to be enclosed in `{}` and validate that it evaluates to a compatible integer type.
- Preserve single evaluation of dynamic bounds and steps before iteration or collection construction.
- Add collection literal syntax `[...]`, defaulting to `Array<T>` when no destination collection type is supplied.
- Make a destination annotation select `List<T>` for the same literal, for example `inmut values: List<Int32> = [0..100];`.
- Expand ranges used as collection elements or constructor arguments into individual values, including `Array(0..2)`, `List(0..=2)`, and mixed forms such as `[1, 5..8, 9]`.
- Ensure expansion works identically for arrays and lists; it is not an array-only feature.
- Support fixed-size allocation declarations such as `inmut buffer: Int32[6];`, reserving six slots without an initializer and preserving fixed length.
- Define type inference, capacity/length checks, initialization behavior, and errors for array/list construction from expanded ranges.
- Add executable-oriented examples with expected output comments for loops, dynamic bounds, arrays, lists, constructor expansion, mixed collection contents, and fixed arrays.
- Update normative specifications, compiler status, limitations, roadmap text, handbook examples, and the companion website metadata/content so no public source advertises the removed range API.

## Capabilities

### New Capabilities

- `range-collection-expansion`: Range expansion in array/list literals and `Array(...)`/`List(...)` constructor calls, including contextual collection selection and fixed-array allocation semantics.

### Modified Capabilities

- `zirk-collections`: Change `Range<T>` syntax, direction inference, step semantics, range API surface, and array/list construction requirements.
- `zirk-grammar`: Replace legacy range-step and method forms with colon steps, braced dynamic expressions, collection literals, and range expansion grammar.
- `zirk-lexical-syntax`: Align token and interpolation rules with braced range operands and the colon step delimiter.
- `zirk-type-system`: Require integer-compatible range operands, infer element and collection types, and validate fixed-array declarations and expanded elements.
- `zirk-contracts`: Keep range iteration compatible with `Iterable<T>` while removing compiler/runtime reliance on the obsolete range methods.

## Impact

- Rust parser and AST: `crates/zirk-parser`, `crates/zirk-ast`, and parser tests.
- Semantic checking: range operand validation, braced-expression enforcement, collection contextual typing, constructor expansion, and fixed-array declarations in `crates/zirk-sema`.
- IR lowering/code generation: colon-step ranges, inferred direction, dynamic range loops, array/list literal lowering, expansion, capacity allocation, and evaluation order in `crates/zirk-ir` and LLVM codegen integration.
- Runtime ABI: range construction/iteration helpers and array/list construction paths in `crates/zirk-runtime`; obsolete `.reverse()` support must be removed or made unreachable according to the final design.
- Tests and fixtures: parser, semantic, IR, runtime, and CLI corpus coverage for every syntax and output scenario.
- Public documentation and status: `docs/ZIRK_SPEC_FINAL.md`, consolidated semantic documents, `docs/ZIRK_LANGUAGE_SPEC.md`, handbook material, feature status, roadmap, and collection specifications.
- Companion repository: `../zirk-lang-site` must update imported handbook/status content and provenance descriptors after the source commit; public status claims must be reviewed with an explicit audit date.
