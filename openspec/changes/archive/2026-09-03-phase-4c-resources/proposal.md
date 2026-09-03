## Why

Phase 4c of the Zirk roadmap — the resource-management foundation — is the remaining piece that must be closed before the runtime and type system can support structured concurrency and advanced memory primitives. The current implementation covers single-resource `match with`, but it lacks grouped acquisition, surfaced close failures, ownership transfer, dependent-resource lifetime tracking, and cancellation-aware cleanup. Closing Phase 4c in a dedicated change delivers a complete resource model that later phases can rely on without also taking on the unrelated 4b, 4d, and 4e work.

## What Changes

- **Grouped `match with` acquisition**: extend the grammar, type checker, and lowering to accept `match r1 with ..., r2 with ...`; acquire resources left-to-right and unwind already-acquired resources right-to-left on failure.
- **Close-failure merging**: introduce `ResourceFailure<BodyError, CloseError>` with `Body`, `Close`, and `BodyAndClose` variants so the managed body and the cleanup path can both fail without loss of information.
- **`TransferableResource` transfer**: add `transfer(r)` for resources that implement `TransferableResource`; the source binding is invalidated and the receiver assumes ownership.
- **Dependent-resource lifetime**: track the parent slot of every dependent resource and reject any use that could outlive the parent through returns, field stores, closure captures, or task spawns.
- **Cancellation-aware cleanup**: make resource close paths observe the active cancellation token and either skip blocking cleanup or surface a cancellation-specific error.

## Capabilities

### New Capabilities

- *None.* This change delivers existing, planned `zirk-resources` behavior that was previously partial or unimplemented.

### Modified Capabilities

- `zirk-resources`: complete grouped acquisition, surfaced/merged close failures, `TransferableResource` transfer, cancellation-aware cleanup, and dependent-resource lifetime enforcement.

## Impact

- `crates/zirk-parser/src/` — grammar and AST support for grouped `match with`.
- `crates/zirk-sema/src/checker.rs` — type rules for grouped resources, `TransferableResource` transfer validation, and dependent-resource lifetime analysis.
- `crates/zirk-ir/src/lower.rs` and `crates/zirk-ir/src/ir.rs` — lowering and new instructions for grouped resource cleanup, `ResourceFailure` construction, and `ResourceTransfer`.
- `crates/zirk-codegen-llvm/src/emit.rs` and `crates/zirk-codegen-llvm/src/runtime.rs` — LLVM emission and runtime symbols for grouped cleanup and transfer.
- `crates/zirk-runtime/src/` — runtime helpers for resource close failure composition, transfer invalidation, and cancellation checks.
- `crates/zirk-cli/tests/corpus/` and per-crate tests — new fixtures and assertions for grouped acquisition, `ResourceFailure` variants, transfer, dependency rejection, and cancellation.
