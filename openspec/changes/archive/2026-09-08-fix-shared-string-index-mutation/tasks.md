## 1. Runtime representation and rollback

- [x] 1.1 Add a collector-traced stable String backing reference and resolve String content and ASCII metadata through it.
- [x] 1.2 Change `zirk_str_set` to update the stable handle's backing reference without replacing observable String identity.
- [x] 1.3 Add runtime journal support for backing-reference updates and unit tests for alias visibility, collector reachability, and rollback.

## 2. Checker and lowering

- [x] 2.1 Accept indexed String place mutation through `mut` and `inmut`, while rejecting roots reachable from `inmut::strict`.
- [x] 2.2 Lower indexed String writes as stable-handle effects, preserve bounds control flow, and journal them in active unsafe transactions.
- [x] 2.3 Update IR verification and focused unit tests for the mutation and journal call shapes.

## 3. Regression coverage and validation

- [x] 3.1 Add semantic tests for mutable, immutable, and strict String indexed-write behavior.
- [x] 3.2 Extend the CLI corpus with alias-visible indexed replacement and retain runtime unsafe rollback coverage.
- [x] 3.3 Run focused runtime, semantic, IR, and CLI tests; validate the OpenSpec change in strict mode with LLVM 20 exported through `LLVM_SYS_201_PREFIX`.
