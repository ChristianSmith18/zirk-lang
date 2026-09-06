## Why

The language defines an infix exponentiation operator `**` (and its compound
form `**=`), and the grammar spec already fixes its precedence and
associativity. The lexer recognizes the tokens but defers them to roadmap
Phase 3b, so no program can use `**` today: the only way to raise a value to a
power is the `pow(exponent)` method, which every numeric family already
implements end to end (`zirk_rt_decimal_pow` / `zirk_rt_decimal_pow_i` for
exact `Float`, the binary helpers for `BinaryFloat`, checked repeated multiply
for the integer widths). Closing the gap is now a thin desugaring layer over
machinery that already exists and is tested, and it is the last missing
arithmetic operator after the exact-`Float` / `BinaryFloat` split.

## What Changes

- The lexer stops deferring `**` and `**=`; they tokenize as ordinary
  operators available in the implemented phase.
- The parser accepts `**` as a right-associative infix operator that binds
  tighter than unary `-`/`!` and looser than postfix calls, so `-2 ** 2`
  parses as `-(2 ** 2)` and `2 ** 3 ** 2` parses as `2 ** (3 ** 2)`.
- The parser accepts `**=` as a compound assignment, desugared like the other
  compound assignments (`a **= b` is `a = a ** b`).
- `a ** b` is defined as `a.pow(b)` and lowers to the same runtime path the
  method call already uses. No new runtime symbol is added.
- Type rules for `**`:
  - `Int ** Int` with a non-negative exponent is `Int` (checked repeated
    multiply, overflow is a controlled error like every other integer op).
  - `Int ** Int` with a negative exponent is exact `Float` — `2 ** -1` is
    `0.5` — matching the long-standing lexer note that exponentiation "is
    defined as the mathematical result converted back".
  - `Float ** Int` is exact `Float`; `Float ** Float` is exact `Float`
    (fractional exponent uses the documented f64-precision path).
  - `BinaryFloatN ** _` is `BinaryFloatN` (f64-precision result), mirroring
    `BinaryFloatN.pow`.
  - Mixing exact `Float` and `BinaryFloat` operands is the same type error the
    other arithmetic operators raise, naming the explicit conversion.
- `zirk-feature-phasing`: `**` is removed from the set of operators the
  phase diagnostic defers; the deferred-operator table and the Phase 3b
  entry are updated.
- Public documentation and the companion website are updated to present `**`
  as available and to show it alongside `pow`.

No behavior of `pow` changes. This is **not BREAKING**: `**` was previously a
compile error (phase diagnostic), so no valid program relied on its absence.

## Capabilities

### New Capabilities
- `exponentiation-operator`: the surface, precedence, associativity, result
  typing, and lowering of the infix `**` and compound `**=` operators across
  the integer, exact-`Float`, and `BinaryFloat` families.

### Modified Capabilities
- `zirk-lexical-syntax`: `**` and `**=` are tokenized as operators of the
  implemented phase, not deferred.
- `zirk-grammar`: the exponentiation row of the precedence table becomes
  normative parser behavior rather than a documented-but-rejected form;
  `**=` joins the compound-assignment set.
- `zirk-feature-phasing`: `**` is no longer listed among the operators the
  phase diagnostic defers.

## Impact

- Code: `crates/zirk-lexer` (phase gate), `crates/zirk-ast` (new `BinaryOp`
  variant or desugar node), `crates/zirk-parser` (infix + compound
  assignment), `crates/zirk-sema` (result typing, `Float`/`BinaryFloat`
  mixing error, negative-exponent widening), `crates/zirk-ir` (lower `**` to
  the existing `pow` calls), `crates/zirk-codegen-llvm` (reuses the existing
  decimal/binary pow emission — no new path expected).
- Tests: lexer, parser, `zirk-sema` typing corpus, `zirk-ir` lowering,
  `zirk-codegen-llvm` emission, and CLI end-to-end fixtures.
- Docs: `docs/ZIRK_LANGUAGE_SPEC.md` operator table and numeric-families
  section, `docs/ZIRK_STDLIB_SPEC.md`, the handbook numeric-operators and
  numeric-literals pages, `11-reference` operator index, Feature Status,
  Current Limitations, `docs/init/ZIRK_ROADMAP.md`, and the handbook roadmap.
- Companion repository: `../zirk-lang-site` must be re-synced with
  `./scripts/sync-website-content.sh` (with a reviewed `--audit-date`,
  because the operator's implementation status changes) so the website no
  longer implies `**` is unavailable.
