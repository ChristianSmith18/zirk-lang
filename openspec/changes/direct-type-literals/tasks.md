## 1. Type-position grammar and AST

- [x] 1.1 Remove the `const(...)` type-atom production, allow only fixed Date/Time/DateTime constructors with direct integer components, and add a focused diagnostic for arbitrary calls or operator expressions in singleton type position; verify parser grammar tests accept direct literals and reject wrappers/calls.
- [x] 1.2 Require owning Date, Time, and local DateTime constructors for temporal singleton values, reject bare ISO spellings in type position, and validate fixed integer components with precise spans; verify valid, invalid, boundary, adjacent, and whitespace-separated cases in parser tests.
- [x] 1.3 Preserve direct duration, null, primitive, and enum-case literal forms while keeping tuple and object values in their type-expression grammar; verify parse-format-parse and targeted rejection cases.

## 2. Semantic identity and type checking

- [x] 2.1 Represent ISO temporal atoms and whitelisted temporal constructors as owning-type values and canonicalize equivalent spellings to the same identity; verify equivalent spellings share identity and invalid dates/times produce source-located diagnostics.
- [x] 2.2 Restrict singleton annotation resolution to supported direct type/literal atoms and whitelisted temporal constructor forms while preserving exact literal widening, union discrimination, and match narrowing; verify focused checker tests for every supported value family.
- [x] 2.3 Remove or migrate source-level `const(...)` singleton evaluator tests and stale AST/formatter expectations; verify the evaluator suite still covers any retained internal constant-evaluation behavior.

## 3. Compiler, IDE, and formatter integration

- [x] 3.1 Update parser, sema, IR, codegen, CLI, browser, formatter, and IDE fixtures to direct syntax; verify relevant crate tests and end-to-end checks accept the same cases.
- [x] 3.2 Ensure editor parsing and diagnostics recognize temporal constructor type atoms, reject bare ISO temporal type values, and report removed wrappers consistently; verify LSP/IDE tests and bundled extension JSON/JavaScript validation.
- [x] 3.3 Replace `const(...)` in demos 36–41 and add direct Date/Time/DateTime examples; verify every runnable demo with `zirk check` and run applicable executable demos.

## 4. Normative and public documentation

- [x] 4.1 Sync the delta specs into the four canonical specs and validate the change with strict OpenSpec validation; verify no active normative spec presents `const(...)` as singleton type syntax.
- [x] 4.2 Update the handbook, language reference, type-algebra examples, and current feature/limitation documents to document direct non-temporal literals and required temporal constructors; verify repository searches distinguish historical archives from active documents.
- [ ] 4.3 After the zirk-lang documentation changes are committed, run `./scripts/sync-website-content.sh`, review and validate the companion website diff, and report both repository revisions; if status evidence changed, review its catalog and pass an explicit audit date.

## 5. Integration verification

- [x] 5.1 Run focused lexer/parser/sema/IR/CLI/IDE tests and `cargo fmt --check`; verify direct scalar, enum, object/tuple, and temporal literal unions end to end.
- [x] 5.2 Run `git diff --check`, `openspec validate direct-type-literals --strict`, and `openspec validate --specs --strict`; all pass after replacing 14 archived-change placeholder Purpose summaries. Remaining active-spec occurrences of `const(...)` describe only rejected syntax; public docs and demos use direct literals and temporal constructors.
