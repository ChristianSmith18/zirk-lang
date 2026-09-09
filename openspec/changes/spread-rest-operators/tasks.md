## 1. Contract inventory and compatibility

- [x] 1.1 Inventory all `...` uses in parser, AST, checker, lowering, runtime, fixtures, handbook, and status documents.
- [x] 1.2 Confirm spread fixed-arity policy, supported destructuring sources, named-argument restrictions, and reference/transfer behavior; record decisions in `design.md`.
- [x] 1.3 Add migration diagnostics for unsupported spread contexts and document the distinction between explicit spread and implicit range expansion.

## 2. AST, lexer, and parser

- [x] 2.1 Add explicit AST nodes/flags for spread call arguments, spread collection elements, and rest-pattern bindings while preserving existing variadic parameter nodes.
- [x] 2.2 Parse `...expression` in positional call arguments with source spans and reject named spread forms unless explicitly approved.
- [x] 2.3 Parse `...expression` in array/list literals and preserve scalar/spread source order.
- [x] 2.4 Parse `[prefix, ...rest]` and equivalent approved ordered patterns, enforcing one final rest binding.
- [x] 2.5 Parse `{ ...source, field: value }` object spread and `{ field, ...rest }` record rest patterns, disambiguating them from blocks.
- [x] 2.6 Add parser and lexer tests proving `DotDotDot` is interpreted correctly in declaration, call, collection, object, and pattern contexts.

## 3. Semantic checking

- [x] 3.1 Require every spread source to implement `Iterable<T>` and propagate `T` into call-argument and collection-element checking.
- [x] 3.2 Expand spread arguments before fixed/variadic parameter matching while preserving named/default rules and statically provable arity constraints.
- [x] 3.3 Type-check spread collection literals contextually for `Array<T>` and `List<T>`, including mixed scalar/spread elements and empty sources.
- [x] 3.4 Type-check rest destructuring, suffix element types, source compatibility, and non-mutating projection semantics.
- [x] 3.5 Verify variadic parameters remain ordered read-only `Iterable<T>` values and can themselves be spread.
- [x] 3.6 Type-check object spread/rest against nominal record shapes, field overrides, duplicate/unknown fields, and projection-copy rules.
- [x] 3.7 Add semantic tests for valid, empty, nested, mixed, object, non-iterable, fixed-arity, named-order, and invalid-rest cases.

## 4. IR, runtime, and memory behavior

- [x] 4.1 Implement one lowering helper that evaluates scalar/spread expressions left to right and consumes each iterable exactly once.
- [x] 4.2 Lower spread calls into fixed argument vectors or variadic sequence values with checked count/arity handling.
- [x] 4.3 Lower spread arrays/lists with checked capacity arithmetic, exact array length, list append behavior, and no partial construction on failure.
- [x] 4.4 Lower rest destructuring bindings using iterator/collection operations without mutating the source.
- [x] 4.5 Reuse `Iterable<T>` iteration copy, invalidation, transfer, and sharing rules; add runtime tests for reference-backed elements.
- [x] 4.6 Verify explicit spread of ranges and interaction with the separate implicit range expansion forms.
- [x] 4.7 Lower object spread/rest with nominal field layouts, left-to-right evaluation, and source immutability.

## 5. Fixtures and documentation

- [x] 5.1 Add and maintain `examples/spread_rest_examples.zrk` with expected outputs for rest parameters, spread calls, collection literals, constructors, ranges, array/list destructuring, record spread, and object rest destructuring.
- [x] 5.2 Add CLI corpus fixtures for all valid and invalid spread/rest scenarios.
- [x] 5.3 Update authoritative language/spec documents, handbook examples, feature status, current limitations, and roadmap wording to distinguish delivered variadics from new spread/rest behavior.
- [x] 5.4 Update OpenSpec main requirements and remove contradictory statements that rest destructuring or spread are unsupported once implementation is verified.
- [x] 5.5 Commit zirk-lang documentation/source changes, run `./scripts/sync-website-content.sh`, review `../zirk-lang-site`, and commit the website synchronization separately.
- [x] 5.6 If status evidence changes, require human review of the website status catalog with an explicit `--audit-date YYYY-MM-DD`.

## 6. Verification and release readiness

- [x] 6.1 Run parser, semantic, IR, runtime, and CLI test suites with the required LLVM toolchain.
- [x] 6.2 Run `zirk check` and `zirk run` for the example file and record exact toolchain results.
- [x] 6.3 Validate the OpenSpec change in strict mode and confirm no stale spread/rest claims remain in source or website content.
- [x] 6.4 Report separate zirk-lang and zirk-lang-site revisions with the final verification matrix.
