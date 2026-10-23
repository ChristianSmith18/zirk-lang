## 1. Audit of the object-oriented surface

- [x] 1.1 Catalogue every object-model construct from `docs/handbook/02-handbook/08-classes-and-objects/` and `09-interfaces-and-traits/` in `crates/zirk-browser/oop-catalog.json` and `docs/browser-oop-coverage.md`; verify every handbook chapter in both directories maps to a construct id.
- [x] 1.2 Run the whole native corpus (218 programs with recorded output) and one probe program per uncovered construct through the native driver and the browser pipeline; verify the results are recorded in the coverage document (184 agree, 34 are capability exclusions).
- [x] 1.3 Triage each divergence as must-fix, defer, or escalate and record it in the coverage document; verify every non-agreeing program is either in the exclusion list or in the findings section.

## 2. Conformance fixtures

- [x] 2.1 Add fixtures for constructors, `construct` sugar, fields, `this`, and visibility; verify `node scripts/run-native-browser-fixtures.mjs` and the browser fixture runners agree on each.
- [x] 2.2 Add fixtures for single inheritance, `super`, `#override`, `final`, and abstract classes with abstract dispatch; verify both runners agree.
- [x] 2.3 Add fixtures for interfaces, traits, and default implementations including an override of a default; verify both runners agree.
- [x] 2.4 Add fixtures for static members and static nested, `inner`, and local classes; verify both runners agree.
- [x] 2.5 Add a fixture for generic classes at several instantiations; verify both runners agree.
- [x] 2.6 Add fixtures for polymorphic `List<Supertype>` and `Map<K, Supertype>` dispatch, object identity versus equality, and records/enums inside class hierarchies; verify both runners agree.
- [x] 2.7 Add a fixture for typed exceptions raised from calls inside methods with `finally`; verify completion and output match native.

## 3. Fix divergences

- [x] 3.1 For the must-fix row (a subclass method resolved with an inherited method's signature), add the failing corpus program `subclass_adds_method` before the fix; verify it failed with `compiler produced invalid IR` first and passes in both engines after the fix.
- [x] 3.2 Make excluded instructions fail preflight with the Zirk feature name; verify `corpus_parity` asserts the named feature, the capability kind, and that no program output is produced.
- [x] 3.3 Re-run `cargo test -p zirk-ir -p zirk-sema -p zirk-interpreter -p zirk-browser` and the end-to-end suite; verify no regressions.

## 4. Catalog and verification

- [x] 4.1 Add the checks that fail when a catalog fixture or corpus program is missing, when a supported construct cites a browser exclusion, or when a fixture file is not listed in `FIXTURES`; verify the catalog check fails on a deliberately renamed fixture and passes after reverting.
- [x] 4.2 Republish the catalog verdicts as `objectModel` in the package manifest and require it in `scripts/verify-browser-package.mjs`; verify by writing and verifying a manifest for a scratch package directory.
- [x] 4.3 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `scripts/audit-browser-dependencies.sh`; verify everything passes except the five `zirk-parser` grammar tests that already fail on the base revision for unrelated `std.io` reasons.

## 5. Documentation, status, and companion-site handoff

- [x] 5.1 Update `docs/browser-interpreter-support-matrix.md` so object, callable, record and enum wording matches the catalog; verify no claim lacks a fixture or corpus program.
- [x] 5.2 Update `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/browser-package-handoff.md`, and `crates/zirk-browser/fixtures/README.md`; verify the documents agree with the catalog.
- [x] 5.3 Commit the `zirk-lang` changes, then build and verify the browser package from that clean revision; verify `scripts/verify-browser-package.sh` passes.
- [x] 5.4 Run `./scripts/sync-website-content.sh`; if project-status evidence changed, obtain human review of the `zirk-lang-site` status catalog and pass an explicit `--audit-date YYYY-MM-DD`.
- [x] 5.5 In `../zirk-lang-site`, adopt the rebuilt package and review the diff for contradictory availability or target claims; commit it separately and record both revisions.
