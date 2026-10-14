## 1. Failing tests and fixtures first

- [x] 1.1 Cover cloning a flat object, a nested tree, a shared-child graph, and a cyclic graph; verify the native corpus programs `clone_record` and `callable_clone` failed with the excluded-capability result before the change.
- [x] 1.2 Cover cloning through a supertype reference and a closure capturing an object; verify the same failure before the change.
- [x] 1.3 Add fixtures `clone-tree`, `clone-shared-child`, `clone-cycle`, `clone-polymorphic`, `clone-callable` with expected output taken from native execution; verify `node scripts/run-native-browser-fixtures.mjs` accepts them.

## 2. Interpreter implementation

- [x] 2.1 Implement the worklist graph copy with a source-to-destination identifier map recorded at allocation; verify the tree, shared-child, and cycle fixtures pass.
- [x] 2.2 Copy the runtime entry (object, record, enum, closure, container) so subclass layouts survive a supertype receiver; verify `clone-polymorphic` passes.
- [x] 2.3 Share immutable strings, chars, regexes, ranges, and runtime errors; verify `clone-tree` string output and `is` results match native.
- [x] 2.4 Charge instruction cost and weighted heap accounting before each allocation; verify `deep_clone_work_is_charged_against_the_instruction_budget`.
- [x] 2.5 Reclassify `InstKind::Clone` as browser-supported and keep `Weak*`, pointer, native-slice and journal instructions excluded; verify the exhaustive support-matrix tests and the exclusion tests pass.

## 3. Differential verification

- [x] 3.1 Run the clone fixtures and the `clone_record`/`callable_clone` corpus programs through the browser and native paths; verify output matches.
- [x] 3.2 Add a heap-limit stress test with a very wide graph at the maximum default limits; verify it ends in `limit-exceeded` naming `heap`.
- [x] 3.3 Run `cargo test --workspace`, formatting, clippy, and `scripts/audit-browser-dependencies.sh`; verify only the pre-existing `zirk-parser` failures remain.

## 4. Documentation, status, and companion-site handoff

- [x] 4.1 Update the support matrix, Feature Status, and the object-model coverage document; verify they cite the clone fixtures.
- [x] 4.2 Rebuild and verify the browser package from the committed revision, run `./scripts/sync-website-content.sh`, and obtain human review with an explicit `--audit-date YYYY-MM-DD` if status evidence changed.
- [x] 4.3 In `../zirk-lang-site`, adopt the rebuilt package, review the diff, and commit it separately with both revisions recorded.
