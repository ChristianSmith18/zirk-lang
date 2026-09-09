## 1. Normative contract and compatibility inventory

- [ ] 1.1 Resolve the open semantic decisions for contradictory step direction, allocation-only array initialization, and `Range<Duration>` support; record the decisions in `design.md` and the affected specs.
- [ ] 1.2 Inventory every parser, AST, checker, lowering, runtime, fixture, handbook, roadmap, status, and website reference to `..step`, `.step(...)`, `.reverse()`, and range interpolation.
- [ ] 1.3 Add migration diagnostics and negative tests for legacy `start..end..step`, `.step(...)`, `.reverse()`, unbraced dynamic operands, and zero steps.

## 2. Lexer, parser, and AST

- [ ] 2.1 Extend the AST range node to retain colon-step syntax and explicit-brace metadata for non-literal operands while preserving inclusive information and source spans.
- [ ] 2.2 Update range parsing to consume `..`, `..=`, and an optional `:step` without conflicting with ternary expressions, type annotations, or slice colons.
- [ ] 2.3 Add a balanced-brace operand parser for range bounds and steps, including nested expressions and precise diagnostics for missing or unmatched braces.
- [ ] 2.4 Add primary collection-literal parsing for `[...]`, including comma-separated scalar/range elements and empty literals, while preserving postfix indexing and slicing grammar.
- [ ] 2.5 Add parser/lexer tests for all ascending, descending, inclusive, negative-step, braced-expression, collection-literal, and constructor-argument forms.

## 3. Semantic checking and type inference

- [ ] 3.1 Refactor range checking to require compatible integer operands, enforce braces for non-literals, reject zero steps, and validate constant sign/bound contradictions.
- [ ] 3.2 Implement contextual collection typing: unannotated `[...]` infers `Array<T>`, `List<T>` context selects a list, and all expanded elements participate in element-type unification.
- [ ] 3.3 Type-check range expansion in `Array(...)` and `List(...)` calls, including mixed scalar/range arguments and left-to-right evaluation constraints.
- [ ] 3.4 Implement fixed-array declaration typing for `T[n]`, enforce constant non-negative lengths, and apply the chosen default-initialization/read-before-write policy.
- [ ] 3.5 Remove range-builder method dispatch from the checker and update member diagnostics so `.step(...)` and `.reverse()` are rejected with migration guidance.
- [ ] 3.6 Add semantic tests for inferred direction, dynamic bounds, incompatible types, collection context, mixed expansion, fixed lengths, and invalid legacy forms.

## 4. IR lowering and runtime behavior

- [ ] 4.1 Normalize range operands in one lowering path that infers omitted step direction and evaluates start/end/step exactly once.
- [ ] 4.2 Update literal and value range loops to select forward/backward comparisons from normalized step sign and preserve `..` versus `..=` endpoint behavior.
- [ ] 4.3 Add checked range expansion lowering for arrays and lists, including capacity/length overflow handling and deterministic left-to-right element materialization.
- [ ] 4.4 Lower `Array(...)`/`List(...)` range arguments through the same expansion helper used by `[...]` literals.
- [ ] 4.5 Lower allocation-only fixed arrays with exactly the declared number of slots and no implicit resizing.
- [ ] 4.6 Remove `zirk_range_reverse` and obsolete range-builder lowering/ABI references once no supported path uses them; retain only explicitly approved read-only range properties.
- [ ] 4.7 Add IR and runtime tests for empty/equal bounds, inclusive singleton ranges, descending defaults, positive/negative steps, zero-step failures, and expansion length.

## 5. Collection integration

- [ ] 5.1 Implement array literal construction from expanded elements and verify fixed-length storage, indexing, iteration, and mutation rules.
- [ ] 5.2 Implement list literal construction from expanded elements and verify resizable list operations and iteration order.
- [ ] 5.3 Verify mixed literals and constructor calls preserve source order and do not create nested range elements.
- [ ] 5.4 Add CLI corpus fixtures covering `[0..100]`, `List<Int32> = [0..100]`, `Array(0..2)`, `List(0..=2)`, mixed values, and `Int32[6]` declarations.

## 6. Documentation and public alignment

- [ ] 6.1 Update `docs/ZIRK_SPEC_FINAL.md`, consolidated semantic documents, `docs/ZIRK_LANGUAGE_SPEC.md`, and handbook range/collection chapters with the finalized contract and examples.
- [ ] 6.2 Update `openspec/specs` main requirements, feature status, current limitations, and roadmap text to remove obsolete APIs and accurately state delivery status.
- [ ] 6.3 Replace stale repository fixtures and the new `examples/range_feature_examples.zrk` comments if any finalized decision changes their expected output.
- [ ] 6.4 Commit the zirk-lang source/documentation changes, run `./scripts/sync-website-content.sh` from a clean companion checkout, and review the generated `../zirk-lang-site` diff.
- [ ] 6.5 If status evidence changed, obtain human review of the website status catalog with an explicit `--audit-date YYYY-MM-DD`, then commit the website synchronization separately.

## 7. Verification and release readiness

- [ ] 7.1 Run `cargo test -p zirk-lexer`, `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-runtime`, and `zirk-cli` with the required LLVM toolchain available.
- [ ] 7.2 Run `zirk check`, `zirk run`, and relevant corpus tests for every example in `examples/range_feature_examples.zrk`; record any toolchain limitation explicitly.
- [ ] 7.3 Validate OpenSpec artifacts and confirm no active source, generated documentation, or website content still advertises removed range syntax or methods.
- [ ] 7.4 Report separate zirk-lang and zirk-lang-site revisions and the final verification matrix.
