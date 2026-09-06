# Proposal: pending-types-and-string-repeat

## Why

Three small but visible gaps remain in the scalar/native surface after the
`Decimal`/`Float` rename landed:

1. `UInt` is a registered type name that the checker rejects with "arrives in
   Phase 3b". The handbook lists it as the unsigned alias for `UInt32`, the
   same role `Int`/`Integer` play for `Int32`. The alias is trivially
   implementable and the checker error is now user-facing noise.
2. `String * Int` lowers to invalid IR whenever the repetition appears as an
   operand inside a larger expression (`"x: " + "ab" * 3`). The repeat guard
   (`checked_repeat`) opens a `fail`/`cont` branch, but `opens_blocks` does not
   recognize `Mul` over `String` as block-opening, so a sibling operand emitted
   before the branch is stranded in an earlier block. This is a verified
   compiler bug, not a missing feature.
3. `unsafe`/`Pointer` are already implemented end to end (see the
   `pointer_from_*`, `native_slice_*` and `unsafe_journal_*` corpus fixtures)
   but the feature has no handbook coverage and no example program, so readers
   cannot discover it.

`Option<T>` and the `Date`/`Time` temporal family are deliberately out of
scope: `T?` already covers absence for application code, and the temporal
family is explicitly deferred to Phase 7 by `zirk-temporal-types`.

## What Changes

- Add `UInt` (and `UInteger` if the project decides to keep paired aliases) as
  a transparent alias for `UInt32`, mirroring how `Int`/`Integer` alias
  `Int32`. No new semantics: same ranges, same overflow rules, same member
  surface.
- Fix `String * Int` lowering so the repetition is recognized as a
  block-opening expression in `opens_blocks`; any operand evaluated before it
  is held through a slot exactly like operands held across `if`/`match`/`?.`.
- Add a `Pointer`/`unsafe` handbook page plus a corpus example exercising
  `Pointer.from`, `read`/`write`, `NativeSlice` bounds and `unsafe` blocks, and
  document the feature in the feature-status sources.
- Update `hello.zrk` (the "everything Zirk can do" example) with `UInt` and an
  `unsafe`/`Pointer` section.

## Capabilities

### New Capabilities

None — every change lands inside an existing capability.

### Modified Capabilities

- `zirk-scalars`: "Complete family of integer widths" gains the `UInt`/
  `UInteger` alias for `UInt32`.
- `zirk-ir-lowering`: add a requirement that `String * Int` is a
  block-opening expression, so earlier operands are held through slots.

(The unsafe/Pointer documentation gap is already a normative requirement —
`language-documentation-information-architecture`'s "Complete memory and
unsafe path" — so no spec delta is needed; only the missing page and the
feature-status/example updates are implementation work.)

## Impact

- `crates/zirk-sema/src/types.rs`, `checker.rs`: `UInt` alias resolution.
- `crates/zirk-ir/src/lower.rs`: `opens_blocks` learns the `String * Int`
  branch; no other lowering changes.
- Tests: `zirk-sema` typing tests for `UInt`; IR lowering test for
  `"x: " + "ab" * n`; CLI corpus fixture for the unsafe/Pointer example.
- Docs: new handbook page under `17-memory-and-safety/` or extension of the
  existing pointers page; `ZIRK_FEATURE_STATUS.md` and the roadmap entry.
- Website: `../zirk-lang-site` must be re-synced after the docs land.

This change affects normative semantics (a new alias is part of the language),
public documentation, and implementation status — the website sync is
mandatory before the change is marked complete.
