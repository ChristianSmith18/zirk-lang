## 1. Parser and AST for grouped match-with

- [x] 1.1 Extend the grammar in `crates/zirk-parser/src/grammar.rs` (or equivalent) to accept `match r1 with ..., r2 with ...` with one or more acquisition clauses separated by `,`.
- [x] 1.2 Update `crates/zirk-parser/src/ast.rs` so `MatchWith` carries a list of `acquisitions`, each with an `expr` and an optional `close` action, plus a single `body` block and a single `error` branch.
- [ ] 1.3 Add parser unit tests for one, two, and three acquisition clauses, with and without explicit close actions.

## 2. Semantic analysis for grouped resources

- [x] 2.1 In `crates/zirk-sema/src/checker.rs`, ensure every acquisition expression returns `Resource<E>` for some error type `E`.
- [x] 2.2 Verify that all acquisition error types are compatible with the `error` branch's expected pattern type; otherwise produce a type mismatch diagnostic.
- [ ] 2.3 Enforce left-to-right acquisition ordering in the semantic model: an acquisition may not use a later resource, and the `error` branch is only reachable after all attempted acquisitions.
- [ ] 2.4 Extend the resource scope/lifetime table to hold multiple owned slots for the duration of the `match with` body.

## 3. IR lowering for grouped acquisition and cleanup

- [x] 3.1 In `crates/zirk-ir/src/lower.rs`, lower a grouped `match with` to a left-to-right acquisition sequence.
- [x] 3.2 Emit a right-to-left cleanup block that closes every already-acquired resource when a later acquisition fails.
- [x] 3.3 Emit the same right-to-left cleanup block on every normal and exceptional exit from the body.
- [ ] 3.4 Add or reuse an IR instruction (e.g., `ResourceClose` or `CleanupGroup`) that captures the list of acquired resource slots and the close action for each.

## 4. `ResourceFailure` type and error merge

- [x] 4.1 Define `ResourceFailure<BodyError, CloseError>` in the stdlib type registry (`crates/zirk-sema/src/types.rs` or stdlib metadata) with the variants `Body(BodyError)`, `Close(CloseError)`, and `BodyAndClose(BodyError, CloseError)`.
- [x] 4.2 In `crates/zirk-ir/src/lower.rs`, construct `ResourceFailure.Body` when only the body fails and no close is attempted.
- [x] 4.3 Construct `ResourceFailure.Close` when the body succeeds but close fails.
- [x] 4.4 Construct `ResourceFailure.BodyAndClose` when both body and close produce an error.
- [ ] 4.5 Add lowering tests for each of the three `ResourceFailure` variants.

## 5. `TransferableResource` and `transfer(r)`

- [x] 5.1 Add a `TransferableResource` contract to the resource capability set recognized by `crates/zirk-sema/src/checker.rs`.
- [x] 5.2 Add `transfer(r)` as an expression form; the operand must implement `TransferableResource`.
- [x] 5.3 Mark the source binding as moved/invalid after a statically visible `transfer`; reject any subsequent use of `r` with a compile-time diagnostic.
- [x] 5.4 Define `InstKind::ResourceTransfer { source, dest }` in `crates/zirk-ir/src/ir.rs` and lower `transfer(r)` to it.
- [ ] 5.5 Implement runtime `moved`-flag support in `crates/zirk-runtime/src/resource.rs` so debug builds can report `use after transfer` cleanly.

## 6. Dependent-resource lifetime analysis

- [ ] 6.1 Track a `parent_slot` in the resource descriptor for every `Dependent<T>` or dependent resource value.
- [ ] 6.2 In `crates/zirk-sema/src/checker.rs`, reject returning a dependent resource from a function.
- [ ] 6.3 Reject storing a dependent resource in a field of an object that may outlive the parent.
- [ ] 6.4 Reject capturing a dependent resource in a closure that may outlive the parent.
- [ ] 6.5 Reject passing a dependent resource to a task spawn or any other construct that may outlive the parent (where available).
- [ ] 6.6 Emit a clear "dependent resource outlives parent" diagnostic that names the parent and the offending use.

## 7. Cancellation-aware cleanup

- [x] 7.1 Expose `zirk_rt_is_cancelled` or an equivalent cancellation token accessor in `crates/zirk-runtime/src/`.
- [x] 7.2 In the resource close path in `crates/zirk-ir/src/lower.rs`, read the active cancellation token before invoking the user close action.
- [~] 7.3 If cancelled, lower to a `skip close` branch (a real `CancellationError` needs Phase 5 cancellation tokens; `zirk_rt_is_cancelled` currently always returns `false`).
- [x] 7.4 Ensure the cancellation check is ordered correctly with respect to the right-to-left cleanup of a grouped `match with`.

## 8. Codegen and runtime support

- [x] 8.1 Add C-ABI runtime symbols for `zirk_rt_resource_transfer`, `zirk_rt_resource_close_group`, and `zirk_rt_is_cancelled` (or equivalent naming) to `crates/zirk-codegen-llvm/src/runtime.rs`.
- [x] 8.2 Implement the corresponding helpers in `crates/zirk-runtime/src/resource.rs`.
- [x] 8.3 Update `crates/zirk-codegen-llvm/src/emit.rs` to emit grouped resource cleanup, `ResourceTransfer`, and cancellation-token checks.

## 9. Tests and validation

- [x] 9.1 Add CLI fixtures in `crates/zirk-cli/tests/corpus/` for grouped `match with` success and failure paths.
- [x] 9.2 Add CLI fixtures for `ResourceFailure.Body`, `.Close`, and `.BodyAndClose` patterns.
- [x] 9.3 Add CLI fixtures for valid `transfer(r)`, use-after-transfer rejection, and non-`TransferableResource` rejection.
- [ ] 9.4 Add CLI fixtures for dependent-resource return, field-store, and closure-capture rejections.
- [ ] 9.5 Add CLI fixtures for cancellation-aware cleanup (cancelled close is skipped or returns a cancellation error).
- [ ] 9.6 Add IR lowering tests in `crates/zirk-ir/tests/lowering.rs` for grouped acquisition ordering, close-failure merge, and `ResourceTransfer`.
- [x] 9.7 Run `cargo test -p zirk-parser`, `cargo test -p zirk-sema`, `cargo test -p zirk-ir`, and `cargo test -p zirk-cli` for the new fixtures.

## 10. OpenSpec and documentation

- [x] 10.1 Ensure `proposal.md`, `design.md`, `specs/zirk-resources/spec.md`, and `tasks.md` are consistent with the implementation scope.
- [x] 10.2 Run `openspec validate phase-4c-resources --strict` and fix any reported issues.
