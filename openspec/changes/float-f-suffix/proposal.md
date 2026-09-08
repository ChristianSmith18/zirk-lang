# Proposal: float-f-suffix

## Why

The `decimal-and-float-type-rename` established `Decimal` as the exact base-ten
fractional default and `Float`/`Float16`…`Float128` as the IEEE 754 binary
family, with `f`/`fN` as the binary literal suffix. The lexer, however, still
implements the previous scheme: it accepts `b`/`b16`/`b32`/`b64`/`b128` and
rejects `f`/`fN`, so every documented example (`1.5f32`) fails to compile. At
the same time, the language decided that `Decimal` — being the default — takes
no literal suffix at all, so the specified `d`/`dN` decimal suffix must be
dropped rather than implemented.

## What Changes

- The lexer accepts `f`, `f16`, `f32`, `f64`, and `f128` on fractional and
  scientific literals, producing the corresponding `Float`/`FloatN` literal.
  **BREAKING** for the implemented surface: `b`/`bN` stops resolving.
- `b` and `bN` suffixes are rejected with a targeted diagnostic that points to
  `f`/`fN` as the binary-float suffix.
- `d` and `dN` suffixes are rejected as invalid numeric suffixes; `Decimal`
  literals are always written without a suffix (there is no explicit decimal
  suffix).
- The invalid-suffix diagnostic no longer mentions `b` as the valid
  binary-float spelling, and checker diagnostics that suggest a binary suffix
  now say `f` instead of `b`.
- Specs and internal docs that still describe `b`/`d` suffixes or the
  `BinaryFloat*` spellings are aligned to the `f`/`Float*` naming.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `zirk-lexical-syntax`: the fractional-suffix requirement drops `d`/`dN`
  entirely, keeps `f`/`fN` → `FloatN`, and keeps the `b`/`bN` rejection but
  pointing only to `f`/`fN` (no decimal suffix exists to point at).
- `zirk-grammar`: the integer-width/fractional-literal requirement drops the
  `d`/`dN` clause and the `d` pointer in the `b`/`bN` rejection diagnostic.
- `exact-decimal-arithmetic`: the binary-literal wording still specifies the
  old `b` suffix and `BinaryFloat*` spellings; it is updated to `f`/`fN` and
  `Float*` names.

## Impact

- `crates/zirk-lexer`: `FLOAT_WIDTHS`, suffix classification, and the
  invalid-suffix diagnostic text; lexer tests move from `b` to `f` spellings.
- `crates/zirk-sema`: comments and diagnostics that suggest a `b` suffix, and
  any literal-width handling keyed on the old `b*` strings.
- `docs/`: handbook and `ZIRK_LANGUAGE_SPEC.md` already document `f` and the
  absence of `d`; `docs/init/*` and remaining spec documents still mention the
  `b` suffix and are updated in the same change.
- Tests: `zirk-lexer` lexical tests and any sema/lowering tests that use `b*`
  literals switch to `f*` spellings; workspace `cargo test` must pass.
