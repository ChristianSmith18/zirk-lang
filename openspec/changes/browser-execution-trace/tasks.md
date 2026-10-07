## 1. Decision and groundwork

- [ ] 1.1 Write `docs/decisions/ADR-022-browser-execution-trace.md` stating that tracing is an observation channel within ADR-020's authority model and that program-driven host APIs are out of scope; verify the ADR index in `docs/decisions/README.md` lists it.
- [ ] 1.2 Verify that class and field names reach the interpreter through the module's layouts for plain, inherited, and generic classes; fix any gap and verify `cargo test -p zirk-ir -p zirk-codegen-llvm` still passes unchanged.
- [ ] 1.3 Define trace event, limit, and truncation types in the interpreter crate with documented semantics; verify unit tests reject invalid limit configurations.

## 2. Interpreter emission

- [ ] 2.1 Write failing tests for allocation and field-write events including sequence numbers and per-run object ids; verify they fail without tracing support.
- [ ] 2.2 Implement the optional trace sink and emit allocation and field-write events; verify the tests pass and an untraced run does no trace work.
- [ ] 2.3 Write failing tests for method call, method return, and throw events with callee name, receiver object id, and source span; verify they fail first.
- [ ] 2.4 Implement call, return, and throw emission, including closure and contract-dispatched calls; verify the tests pass.
- [ ] 2.5 Stamp output events with the shared sequence counter; verify a test asserts output events interleave correctly with field writes.
- [ ] 2.6 Render field values with a byte cap and render object-typed values as references; verify tests for truncation and object references.
- [ ] 2.7 Enforce event and byte limits without failing the program and set the truncation marker; verify the program's output and completion are identical to an untraced run.

## 3. Browser contract

- [ ] 3.1 Add the optional request field, response field, and trace limits to `crates/zirk-browser/src/contract.rs`, tighten-only like existing limits; verify contract tests cover default, tightened, and over-limit requests.
- [ ] 3.2 Reject unknown trace options deterministically before execution; verify a boundary test.
- [ ] 3.3 Regenerate `browser-api.d.ts` and verify the strict declaration test covers the new types and that responses without tracing are byte-identical to the previous contract.
- [ ] 3.4 Surface tracing and its event kinds and limits in the contract operation and package manifest; verify the manifest verification script covers the new data.

## 4. Behavior-preservation and stress verification

- [ ] 4.1 Run every existing browser fixture traced and untraced and assert identical output, completion, failure category, and limit boundary; verify the runner reports any difference by fixture name.
- [ ] 4.2 Add a determinism test running one program twice and asserting identical traces; verify ids and sequence numbers match.
- [ ] 4.3 Add a stress test with a long loop writing fields at maximum limits; verify truncation is reported and memory stays bounded.
- [ ] 4.4 Run `cargo test --workspace`, formatting, clippy, `scripts/audit-browser-dependencies.sh`, the package size check, and `scripts/verify-browser-package.sh`; verify all pass.

## 5. Documentation, status, and companion-site handoff

- [ ] 5.1 Update `docs/browser-interpreter-support-matrix.md`, `docs/browser-package-handoff.md`, and `docs/init/ZIRK_FEATURE_STATUS.md` for the trace feature and its limits; verify each states that tracing is observation, not a host effect.
- [ ] 5.2 Commit the `zirk-lang` changes, rebuild and verify the browser package from that clean revision, and run `./scripts/sync-website-content.sh`; if project-status evidence changed, obtain human review and pass an explicit `--audit-date YYYY-MM-DD`.
- [ ] 5.3 In `../zirk-lang-site`, update the worker and TypeScript contract for the new field, review the diff for contradictory claims, and commit it separately with both revisions recorded.
