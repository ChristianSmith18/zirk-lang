# Proposal: fase-3-verificacion-constraints

## Why

Generic constraints declared with `from` are parsed and recorded but never
verified: `crates/zirk-sema/src/checker.rs` states that "verifying `from` at
the use site and inside the body is not implemented yet: `constraints` is only
recorded". As a result, `fun f<T from Serializable>(x: T)` accepts any
concrete `T`, and a generic body may call methods its constraints do not
declare. This is a silent correctness hole: the compiler accepts programs the
specification forbids.

At the same time, declared variance (`in T` / `out T`) is in a contradictory
state: `zirk-generics` both requires covariant/contravariant checking and
declares it out of scope, while the checker only emits `PENDING_FEATURE`
without verifying anything. Finally, `value class` was removed from the data
model but several specs still mention it. All three items are Phase-3 type
system closure work that must land before Phase 5.

## What Changes

- Verify `from` constraints at the use site: a concrete argument for
  `T from A & B` must satisfy every listed contract, with diagnostics naming
  each missing constraint.
- Check each generic body exactly once against its declared constraints: a
  body may only use members that some constraint guarantees; other member
  accesses are rejected with help text pointing at the constraint.
- Decide declared variance: verify `in`/`out` positions per the spec
  (covariant output-only, contravariant input-only, mutable attribute use
  requires invariance, `Fn` retains intrinsic variance) **or** reject the
  syntax with a stable diagnostic. The chosen direction reconciles the two
  contradictory requirements in `zirk-generics`.
- Remove remaining `value class` mentions from specs that still reference it
  (`zirk-type-system`, `zirk-grammar`, `zirk-lexical-syntax`,
  `zirk-collections`), consistent with the `zirk-data-types` decision.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `zirk-generics`: constraints `from` SHALL be verified at the use site and
  generic bodies SHALL be checked once against their constraints; the
  declared-variance requirement is reconciled (either enforced or explicitly
  rejected, removing the internal contradiction).
- `zirk-type-system`: remove `value class` mentions, keep `record` wording.
- `zirk-grammar`: remove `value class` mention.
- `zirk-lexical-syntax`: remove `value class` mention.
- `zirk-data-types`: remove the `Value classes` requirement and the
  `value class` mention in `Pointer.from`.
- `zirk-contracts`: remove `value class` from the derivation requirement.
- `zirk-native-codegen`: replace `value class` scenarios/wording with
  `record`.

## Impact

- `crates/zirk-sema/src/checker.rs`: constraint verification at
  instantiation sites and inside generic bodies; variance decision
  implemented.
- `crates/zirk-sema/src/types.rs`: constraint satisfaction queries.
- Corpus fixtures: new valid/invalid cases for constraint satisfaction,
  constraint-insufficient member access, and the variance decision.
- `docs/init/ZIRK_FEATURE_STATUS.md`: status updates for generics rows.
- This change affects normative semantics and implementation status; the
  companion repository `../zirk-lang-site` must be synced via
  `./scripts/sync-website-content.sh` after the code changes land.
- Related but out of scope: lowering user-defined `implements Contract<T>`
  and generic dispatch belong to `fase-3-dispatch-generico`, which assumes
  the constraint model defined here.
