# Browser conformance fixtures

Each JSON fixture is a single browser-compatible source unit with these fields:

- `name`: stable fixture identifier.
- `source` and `virtualName`: the in-memory program and diagnostic-only name.
- `expectedOutput`: exact ordered output events.
- `completion`: `completed`, `runtime`, `capability`, or `limit`.
- `exclusions`: explicit native-only features the browser runner must not try
  to execute.

Runners compare only observable output and completion category. They never
read source paths from a fixture or grant filesystem, network, clock, DOM, or
native-runtime authority to the program.

Fixtures named `oop-<concept>` exercise the object model and are the evidence
cited by `crates/zirk-browser/oop-catalog.json`. Their expected output is taken
from native `zirk run`, never written to match the interpreter; check one with
`node scripts/run-native-browser-fixtures.mjs crates/zirk-browser/fixtures/oop-*.json`.
Every fixture file must be listed in `FIXTURES` in `src/lib.rs`; a test fails
when one is missing. Beyond these single-file fixtures,
`tests/corpus_parity.rs` runs the whole native corpus through the browser.
