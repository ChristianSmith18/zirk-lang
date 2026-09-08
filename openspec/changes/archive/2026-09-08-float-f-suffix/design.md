# Design: float-f-suffix

## Context

The `decimal-and-float-type-rename` change renamed the type surface: `Decimal`
is the exact base-ten fractional default and `Float`/`Float16`…`Float128` is
the IEEE 754 binary family, with `f`/`fN` as the documented literal suffix. The
implementation was never updated: the lexer still accepts `b`, `b16`, `b32`,
`b64`, `b128` (`FLOAT_WIDTHS` in `crates/zirk-lexer/src/token.rs`), and
`check_float_literal` in `crates/zirk-sema/src/checker.rs` matches on the `b*`
strings. Every documented `f` literal currently fails to lex.

Separately, the language decision is that `Decimal` — the default fractional
type — takes no literal suffix at all. The specs previously required a `d`/`dN`
decimal suffix; that requirement is removed, and `d` must be rejected like any
other invalid numeric suffix (it already is, since no `d*` entry exists in
`FLOAT_WIDTHS`, but the diagnostic text must not suggest `d` as valid).

## Goals / Non-Goals

**Goals:**

- `f`/`f16`/`f32`/`f64`/`f128` lex as binary-float literals and resolve to the
  corresponding `Float`/`FloatN` type.
- `b`/`bN` literals produce a targeted diagnostic pointing to `f`/`fN`.
- `d`/`dN` is never a decimal suffix: a trailing `d` remains the `Duration`
  days unit (`1.5d` is 1.5 days), and `dN` spellings fall through to the
  generic invalid-suffix path.
- No reference to `b` or `d` as a valid numeric suffix remains in code,
  diagnostics, or docs.

**Non-Goals:**

- Implementing `to_decimal()` on `JSONNumber` or elsewhere (documented as
  specified-not-implemented only).
- The residual `Float`-means-exact naming drift inside
  `openspec/specs/exact-decimal-arithmetic/spec.md` outside the literal-suffix
  requirement; this change updates only the binary-family requirement it owns.
- Any semantic change to `Decimal` arithmetic, `Duration`, or integer literals.

## Decisions

- **Suffix list is `["f", "f16", "f32", "f64", "f128"]`.** `f` and `f64` both
  map to `Float64`, mirroring the type alias `Float = Float64`. Alternative
  considered: omitting `f64`; rejected because the width suffixes are meant to
  be predictable (`fN` ⇔ `FloatN`).
- **Reject `b*` explicitly, not generically.** A `b` suffix is a migration
  case, not an arbitrary typo: the lexer recognizes `b`/`b16`/`b32`/`b64`/`b128`
  and emits a diagnostic naming `f`/`fN` as the replacement, consistent with
  how the checker already redirects removed `BinaryFloat*` type spellings.
- **`d` is not rejected**: it is already the `Duration` days unit and must keep
  lexing as such (`1.5d` is 1.5 days). "No decimal suffix" therefore means no
  `d` entry ever maps to `Decimal`; non-unit `dN` spellings reach the ordinary
  invalid-suffix path. No dedicated `d` diagnostic: `d` as a decimal marker
  never existed in any shipped implementation, so there is nothing to migrate
  from.
- **The width string is carried on the token as today** (`NumberLit.width`),
  so only the accepted set and the `check_float_literal` match arms change;
  lowering and codegen are untouched.

## Risks / Trade-offs

- [Existing `.zrk` sources or tests still use `b*` literals] → grep the
  workspace corpus and tests; switch them to `f*` in the same change.
- [Diagnostic text elsewhere recommends `b`] → audit the "add a `b` suffix"
  style hints in `checker.rs` and update to `f`.
- [`1d`-style Duration suffixes could be confused with a decimal `d`] → `d`
  stays a valid duration unit; the delta spec now documents `1.5d` as a
  `Duration`, not a rejection.
- [The `Float`/`f64` Rust identifiers inside the compiler keep old names] →
  out of scope; only the surface language changes.
