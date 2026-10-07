## 1. Failing tests and fixtures first

- [ ] 1.1 Add interpreter tests for cloning a flat object, a nested tree, a shared-child graph, and a cyclic graph; verify they fail with the current excluded-capability result.
- [ ] 1.2 Add interpreter tests for cloning through a supertype reference, a `record`, a graph holding `List`/`Map`/`Set` members, and a closure capturing an object; verify they fail for the same reason.
- [ ] 1.3 Add browser conformance fixtures `clone-tree`, `clone-shared-child`, `clone-cycle`, `clone-polymorphic`, `clone-collections`, `clone-callable` with expected output taken from native execution; verify the native fixture runner passes them.

## 2. Interpreter implementation

- [ ] 2.1 Implement the iterative graph copy with a source-to-destination identifier map that records each destination before populating its fields; verify the tree, shared-child, and cycle tests pass.
- [ ] 2.2 Drive the field walk from the module's object layouts and the runtime class of each object; verify the supertype-reference test passes.
- [ ] 2.3 Clone `List`, `Array`, `Map`, `Set` and closure environments through the same map; verify the collection and callable tests pass.
- [ ] 2.4 Charge instruction cost and weighted heap accounting before each allocation and discard partial clones on failure; verify tests for heap exhaustion and instruction exhaustion leave the source graph intact.
- [ ] 2.5 Reclassify `InstKind::Clone` as browser-supported and keep `WeakFrom`, `WeakUpgrade`, `WeakIsAlive`, pointer, native-slice and journal instructions excluded; verify the exhaustive support-matrix test and the exclusion tests pass.

## 3. Differential verification

- [ ] 3.1 Run the six clone fixtures through both the interpreter and the native runner; verify output, completion, and failure category match.
- [ ] 3.2 Add a stress test with a wide and a deep graph at the maximum default limits; verify it ends in the expected result without host stack overflow.
- [ ] 3.3 Run `cargo test -p zirk-interpreter -p zirk-browser`, `cargo test --workspace`, formatting, clippy, and `scripts/audit-browser-dependencies.sh`; verify all pass.

## 4. Documentation, status, and companion-site handoff

- [ ] 4.1 Update `docs/browser-interpreter-support-matrix.md` to move `Clone` out of the excluded list and `docs/init/ZIRK_FEATURE_STATUS.md` to record the reviewed browser coverage; verify both cite the clone fixtures.
- [ ] 4.2 Commit the `zirk-lang` changes, rebuild and verify the browser package from that clean revision, and run `./scripts/sync-website-content.sh`; if project-status evidence changed, obtain human review and pass an explicit `--audit-date YYYY-MM-DD`.
- [ ] 4.3 In `../zirk-lang-site`, adopt the rebuilt package, review the diff for contradictory claims, and commit it separately with both revisions recorded.
