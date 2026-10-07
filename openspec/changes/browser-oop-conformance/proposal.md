## Why

The browser interpreter already executes the object model at the IR level (objects, inherited layouts, virtual and contract dispatch, closures, records, enums, exceptions), but the public claim has not caught up. `docs/browser-interpreter-support-matrix.md` still says objects, callables, records and enums are "planned runtime-core coverage" that "do not become public browser claims until differential fixtures pass", and only three browser fixtures touch classes (`downcast`, `class-construction-sugar`, `void-method-call`). Interfaces, traits with default implementations, abstract classes, static members, visibility, nested/inner/local classes, object identity, generic classes, and polymorphic collections have no differential evidence. Until that evidence exists, the playground and any browser-hosted learning surface cannot honestly teach object-oriented Zirk.

## What Changes

- Audit the whole object-oriented surface documented in `docs/handbook/02-handbook/08-classes-and-objects/` and `09-interfaces-and-traits/` against the browser pipeline and record, per construct, whether it runs, diverges from native, or fails preflight.
- Add browser conformance fixtures (run through both the interpreter and the native `zirk run` runner) for: constructors and `construct` sugar, fields and properties, `this`, visibility, single inheritance with `super`, `#override`, abstract classes and abstract dispatch, interfaces, traits and default implementations, static members, object identity (`is`) versus structural equality, static nested / `inner` / local classes, generic classes and generic methods, records and enums with payloads inside class hierarchies, polymorphic dispatch through `List<T>`/`Map<K,V>` of supertypes, and uncaught/typed exceptions thrown from methods.
- Fix every divergence the audit finds in lowering, the verifier, or the interpreter, each with a regression test. Divergences that require language-level decisions are recorded and escalated instead of patched silently.
- Replace the stale object-model wording in `docs/browser-interpreter-support-matrix.md` with reviewed, fixture-backed claims, and update Feature Status, the browser handoff document, and the fixtures README accordingly.
- Publish a checked-in object-model catalog (`crates/zirk-browser/oop-catalog.json`) and republish its verdicts in the browser package manifest, so a consumer can learn which OOP constructs are covered without inferring it from source presence.
- Run the entire native end-to-end corpus through the browser `run` operation as a standing differential test, with an explicit, exhaustive exclusion list.
- Report excluded instructions by Zirk feature name (for example deep `clone()`) instead of an IR debug dump.
- No change to the source language and no change to `apiVersion: 1`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `zirk-ir-interpreter`: adds a requirement that the delivered object-oriented surface executes with native-equivalent observable behavior, backed by shared fixtures, and that unsupported OOP constructs fail with a structured capability diagnostic instead of an internal fault.

## Impact

- `crates/zirk-browser/fixtures/` and the native/browser fixture runners (`scripts/run-native-browser-fixtures.mjs`): a new family of OOP fixtures with expected output.
- `crates/zirk-interpreter/src/lib.rs`, `crates/zirk-ir/src/lower.rs`, `crates/zirk-ir/src/verify.rs`: fixes for whatever the audit finds; no new instruction kinds are planned by this change.
- `docs/browser-interpreter-support-matrix.md`, `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/browser-package-handoff.md`, `crates/zirk-browser/fixtures/README.md`: status and claim updates. This change affects implementation status and limitations, so public content changes.
- Companion repository `../zirk-lang-site` is impacted: the browser package it pins must be rebuilt from the committed compiler revision, its playground example catalog may gain OOP examples, and its site-owned status catalog needs human review before any new public claim. Run `./scripts/sync-website-content.sh` with a reviewed `--audit-date` after the compiler commit.
- Related in-flight work: `browser-deep-clone` (deep `clone()` is part of the object model and is excluded today) and `browser-execution-trace` (object-state observation for teaching). Neither blocks this change.
