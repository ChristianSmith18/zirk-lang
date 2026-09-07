# Proposal: fase-3-cierre-contratos

## Why

Several contract/type-system features are already required by the specs but
the implementation never caught up — this is spec/code drift, not open
design. `zirk-contracts` requires `TraitName.super.method()` conflict
resolution and contract composition (`interface implements interface`,
`trait implements …`); `zirk-classes` requires `as?` nullable casts and
static-dispatch rules; `zirk-generics` allows trailing type-parameter
defaults. None of them parse today. Closing them completes the Phase-3
object surface before later phases build on it.

## What Changes

- Parse and check qualified trait selection `TraitName.super.method()` so a
  class adopting two traits with conflicting defaults can pick one.
- Allow `implements` on contract declarations: interfaces implementing
  interfaces, traits implementing interfaces/traits; cycles and
  incompatible same-name signatures fail with diagnostics.
- Add `as?` (nullable checked cast): `x as? T` evaluates to `T?` — the
  reference on success, `null` on failure; unrelated casts still fail at
  compile time.
- Add trailing type-parameter defaults `<T = Int32>` that satisfy the
  parameter's constraints.
- Add comparison operator contracts (`<`, `>`, `<=`, `>=`) mapped to
  reserved methods, consistent with the existing `_add`/`_equals` scheme.

## Capabilities

### New Capabilities

(none — every item is already required by existing specs)

### Modified Capabilities

- `zirk-contracts`: clarify/extend the conflict-resolution and
  operator-contract requirements only where implementation reveals a gap
  (e.g., naming the comparison reserved methods).
- `zirk-classes`: `as?` and `TraitName.super.method()` move from spec to
  implementation; no normative text change expected.
- `zirk-generics`: type-parameter defaults move from allowed-by-spec to
  implemented; no normative text change expected.

## Impact

- `crates/zirk-parser`: new syntax (`TraitName.super.m()`, contract
  `implements`, `as?`, `<T = …>`).
- `crates/zirk-ast`: nodes for qualified trait calls, nullable cast, type
  param defaults, contract `implements`.
- `crates/zirk-sema`: conformance graph for contract composition, cycle and
  signature-compatibility checks, nullable-cast typing, default
  substitution, comparison-contract checks.
- `crates/zirk-ir` / `crates/zirk-codegen-llvm`: qualified default
  selection lowers to the chosen trait body; `as?` lowers to checked cast +
  nullable; comparisons dispatch through contract calls.
- Corpus fixtures valid/invalid for every item.
- `docs/init/ZIRK_FEATURE_STATUS.md` rows updated.
- Normative semantics gain features already promised; implementation status
  changes → `../zirk-lang-site` sync required after merge.
