## Why

A browser-hosted learning surface must show learners what their object-oriented code does: an object being created, a field changing, a method being called on one instance and not another. Today the only observable result of a browser run is ordered `stdout` text, so a consumer has to ask learners to `println` their own state and then parse text to animate it. That is fragile, teaches the wrong habit, and cannot show hidden state such as private fields or object identity. A bounded, opt-in, read-only trace of interpreter events gives such consumers real data without granting the program any new authority.

## What Changes

- Add an opt-in execution trace to the interpreter: an ordered, bounded stream of read-only events — object allocation (class name and stable per-run object id), field write (object id, field name, rendered value), method call and return (callee name, receiver object id, source span), throw, and a sequence number that interleaves exactly with output events.
- Expose it through the browser `run` operation as an additive request option and an additive response field, keeping `apiVersion: 1` and leaving the field absent unless requested.
- Bound the trace by new limits (maximum events, maximum rendered value bytes, maximum bytes in total) that callers may only tighten, and mark truncation explicitly.
- Record the decision in a new ADR that amends the ADR-020 host-effect statement: tracing is an output-direction observation channel; it grants the program no input, no clock, no DOM, and no way to read the trace.
- Keep identifiers deterministic: object ids are assigned per run in allocation order, never derived from heap addresses.
- Out of scope here: a game-specific host API such as `game.move()` that lets the program cause effects (a separate, larger decision), single-step debugging, and breakpoints.

## Capabilities

### New Capabilities

### Modified Capabilities

- `zirk-ir-interpreter`: adds the bounded, read-only, opt-in execution trace requirement.
- `zirk-browser-compiler`: adds the additive request option and response field for the trace, and the matching discoverability data.

## Impact

- `crates/zirk-interpreter/src/lib.rs`: an optional trace sink next to the output sink, with emission points at allocation, field store, call and return, and throw.
- `crates/zirk-browser/src/contract.rs`, `crates/zirk-browser/src/lib.rs`, `crates/zirk-browser/browser-api.d.ts`: additive request option, response field, limits, and generated declarations.
- `docs/decisions/ADR-022-browser-execution-trace.md` (new), `docs/browser-interpreter-support-matrix.md`, `docs/browser-package-handoff.md`, `docs/init/ZIRK_FEATURE_STATUS.md`: documentation and status. This change affects the browser package contract and implementation status, so public content changes.
- Companion repository `../zirk-lang-site` is impacted: its worker and TypeScript contract consume the new field, and its playground may show the trace; any public wording about it goes through the reviewed status catalog and `./scripts/sync-website-content.sh`.
- Depends on nothing, but is most useful after `browser-oop-conformance` (so traced OOP constructs are known to be native-equivalent) and `browser-deep-clone`.
