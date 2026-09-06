# Proposal: fase-3-dispatch-generico

## Why

Generic contract dispatch is the last open segment of the Phase-3 object
system: `docs/init/ZIRK_FEATURE_STATUS.md` marks "Generic contract lowering
(`class`/`record` implements `Contract<T>`)" as parsed and partially checked
but not lowered, and the checker emits `E0423 NOT_LOWERED` for user-defined
generic contracts whose methods name their own type parameter. Generic enum
and contract dispatch are likewise pending. Until this lands, `implements
Contract<T>` is accepted by the frontend but cannot reach code generation.

## What Changes

- Lower user-defined `class`/`record` types implementing generic contracts
  (`implements Contract<T>`): build the contract dispatch tables for generic
  instantiations, lower trait default bodies in the generic context, and emit
  `CallContract`/`SafeDispatch::Contract` for calls through a generic contract
  reference.
- Remove the `E0423 NOT_LOWERED` rejection path for the cases this change
  covers; any remaining unimplemented generic-dispatch shape keeps a precise
  diagnostic.
- Extend contract-table construction in `zirk-ir` and itable lookup in
  `zirk-codegen-llvm` so generic instantiations get consistent method indices
  (as already done for two adopters of the same abstract class).
- Preserve source maps through generic lowering so diagnostics and the IR
  verifier keep working.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `zirk-generics`: generic contract conformance SHALL lower and dispatch for
  user-defined `implements Contract<T>` within the scope defined here.
- `zirk-contracts`: contract dispatch SHALL cover generic contract
  instantiations adopted by `class` and `record` types.
- `zirk-ir-lowering`: portable IR SHALL retain the contract tables needed for
  generic dispatch.

## Impact

- `crates/zirk-sema/src/checker.rs`: remove the `E0423` gate for the covered
  shapes; record generic conformance data needed by lowering.
- `crates/zirk-ir/src/lower.rs`: generic contract tables, trait default
  lowering under substitution, contract call emission.
- `crates/zirk-codegen-llvm/src/emit.rs`: itable lookup for generic
  instantiations if the current scheme does not cover them.
- Corpus fixtures: valid/invalid end-to-end cases for generic contract
  dispatch (extending `generic_contracts.zrk` and siblings).
- `docs/init/ZIRK_FEATURE_STATUS.md`: the generic-contract row moves to
  implemented for the covered scope.
- This change affects normative semantics and implementation status; the
  companion repository `../zirk-lang-site` must be synced via
  `./scripts/sync-website-content.sh` after the code changes land.
- Assumption: this change consumes the constraint model as it exists on
  `develop` today (`from` recorded but not yet verified); constraint
  verification itself is owned by `fase-3-verificacion-constraints` and both
  changes merge independently.
