## Why

Zirk already supports variadic/rest parameter declarations such as `...values: Int32`, but it has no spread syntax for passing an existing iterable to a call, constructing an array/list, or destructuring a collection. Documentation describes variadic values as expandable while the parser, AST, checker, and lowering only support fixed argument expressions, leaving a visible gap between the language model and usable collection/function composition.

This change completes the spread/rest family as a coherent, type-safe feature and makes it interoperate with the range and collection expansion work without introducing nested-range special cases.

## What Changes

- Preserve and formalize rest parameters (`...values: T`) as ordered read-only sequences available inside the function body.
- Add spread arguments (`...iterable`) in calls, expanding an `Iterable<T>` into positional arguments in source order.
- Add spread elements (`...iterable`) in array/list literals, expanding values in place.
- Support spread of ranges and ordinary arrays/lists through the same `Iterable<T>` contract.
- Allow multiple spread and scalar elements in one call or collection literal, with deterministic left-to-right evaluation.
- Add rest destructuring for arrays/lists and other explicitly supported ordered iterables, binding a fixed prefix and collecting the remainder.
- Add object/record spread (`{ ...source, field: value }`) and object/record rest destructuring (`{ field, ...rest }`) with named-field preservation and explicit override rules.
- Define object spread for nominal records/classes separately from iterable spread; object spread copies fields, while collection spread consumes `Iterable<T>` elements.
- Require an expected record/object shape for object literals and rest results so `{ ...rest }` cannot be confused with a block or an untyped dictionary.
- Define contextual typing for spread collections and constructor calls, including `Array(...values)`, `List(...values)`, `[...values]`, and mixed elements.
- Enforce element/parameter type compatibility, argument arity, named-argument ordering, and variadic parameter rules after expansion.
- Reject spreading non-iterables, spreading into non-variadic fixed-arity positions when the expanded count is not statically compatible, and invalid rest placement.
- Preserve reference and transfer semantics: spread reads produce independent element values according to ordinary iteration rules; no implicit mutable alias crosses a call or collection boundary.
- Update parser, AST, semantic checker, IR lowering, runtime collection builders, tests, fixtures, normative documentation, feature status, roadmap, and companion website content.

## Capabilities

### New Capabilities

- `spread-rest-operators`: Explicit spread in calls and collection literals plus rest destructuring, integrated with existing variadic parameters and `Iterable<T>`.

### Modified Capabilities

- `zirk-grammar`: Add spread arguments/elements and rest destructuring grammar while preserving variadic parameter declarations.
- `zirk-type-system`: Type-check iterable spread, expanded argument matching, contextual collection inference, rest bindings, and nominal record/object field spread.
- `zirk-collections`: Define spread construction for arrays/lists and interaction with ranges and collection iteration.
- `zirk-contracts`: Use `Iterable<T>` as the source contract for spread values and preserve iteration copy/invalidating rules.
- `zirk-lexical-syntax`: Align `...` token use across variadic declarations, spread expressions, and rest patterns.
- `zirk-ir-lowering`: Lower spread expansion with single evaluation, checked capacity, and deterministic ordering.

## Impact

- AST/parser: `crates/zirk-ast` and `crates/zirk-parser` currently recognize `...` only in parameter declarations; calls and collection literals need explicit spread/rest nodes.
- Semantic checker: `crates/zirk-sema` must expand iterable element types before argument matching and collection element unification while enforcing mutability, cloning, and transfer rules.
- IR/backend: `crates/zirk-ir` and LLVM integration must lower expansion into fixed-arity calls, variadic sequence construction, array/list allocation, and destructuring bindings.
- Runtime: `crates/zirk-runtime` may need iterator-driven builders and checked capacity/length handling; no new external dependency is required.
- Tests/fixtures: parser, semantic, IR, runtime, and CLI corpus coverage for empty, single, multiple, mixed, nested-call, range, and invalid spread cases.
- Public documentation/status: update authoritative language/spec documents, handbook examples, feature status, limitations, roadmap, and generated website provenance in `../zirk-lang-site`.
