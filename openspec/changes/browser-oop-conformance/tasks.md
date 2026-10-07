## 1. Audit of the object-oriented surface

- [ ] 1.1 Enumerate every construct in `docs/handbook/02-handbook/08-classes-and-objects/` and `09-interfaces-and-traits/` and check it into `docs/browser-oop-coverage.md` as a table with columns construct, handbook source, native result, browser result, verdict; verify every handbook file in both directories appears at least once.
- [ ] 1.2 Write one throwaway probe program per construct and run it through the native driver and the browser pipeline; verify each table row records both observed outputs and no row is left blank.
- [ ] 1.3 Triage each divergence as must-fix, defer, or escalate and record the decision in the coverage document; verify every non-matching row has a triage entry.

## 2. Conformance fixtures

- [ ] 2.1 Add fixtures for constructors, `construct` sugar, fields and properties, `this`, and visibility; verify `node scripts/run-native-browser-fixtures.mjs` and the browser fixture runner agree on each.
- [ ] 2.2 Add fixtures for single inheritance, `super`, `#override`, and abstract classes with abstract dispatch; verify both runners agree.
- [ ] 2.3 Add fixtures for interfaces, traits, and default implementations including an override of a default; verify both runners agree.
- [ ] 2.4 Add fixtures for static members, static nested classes, `inner` classes, and local classes; verify both runners agree.
- [ ] 2.5 Add fixtures for generic classes and generic methods at two instantiations each; verify both runners agree.
- [ ] 2.6 Add fixtures for polymorphic `List<Supertype>` and `Map<K, Supertype>` dispatch, object identity versus equality, and records/enums inside class hierarchies; verify both runners agree.
- [ ] 2.7 Add fixtures for typed exceptions thrown from methods, caught by supertype, and an uncaught method exception with its stack frames; verify completion and failure categories match native.

## 3. Fix divergences

- [ ] 3.1 For each must-fix row, write a failing regression test in the owning crate (`zirk-ir`, `zirk-interpreter`, or `zirk-browser`) before changing production code; verify the test fails first and passes after the fix.
- [ ] 3.2 Make object-model constructs that lower to excluded instructions fail preflight with a named capability diagnostic; verify a test asserts the construct name, source span, and that no entrypoint instruction runs.
- [ ] 3.3 Re-run all pre-existing browser fixtures and `cargo test -p zirk-ir -p zirk-interpreter -p zirk-browser`; verify no regressions.

## 4. Catalog and verification

- [ ] 4.1 Generate the construct-to-fixture catalog from fixture metadata and add a check that fails when a catalog fixture is missing or failing; verify the check fails on a deliberately renamed fixture and passes after reverting.
- [ ] 4.2 Expose the catalog through the existing `contract` operation as additive discovery data without changing `apiVersion`; verify the TypeScript declarations and the contract test cover the new field.
- [ ] 4.3 Run `cargo test --workspace`, formatting, clippy, `scripts/audit-browser-dependencies.sh`, and the browser package size check; verify all pass.

## 5. Documentation, status, and companion-site handoff

- [ ] 5.1 Update `docs/browser-interpreter-support-matrix.md` so object, callable, record and enum wording matches the catalog and no longer says the model is merely planned; verify no claim lacks a fixture.
- [ ] 5.2 Update `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/browser-package-handoff.md`, and `crates/zirk-browser/fixtures/README.md`; verify the three documents agree with the catalog.
- [ ] 5.3 Commit the `zirk-lang` changes, then build and verify the browser package from that clean revision; verify `scripts/verify-browser-package.sh` passes.
- [ ] 5.4 Run `./scripts/sync-website-content.sh`; if project-status evidence changed, obtain human review of the `zirk-lang-site` status catalog and pass an explicit `--audit-date YYYY-MM-DD`.
- [ ] 5.5 In `../zirk-lang-site`, adopt the rebuilt package and review the diff for contradictory availability or target claims; commit it separately and record both revisions.
