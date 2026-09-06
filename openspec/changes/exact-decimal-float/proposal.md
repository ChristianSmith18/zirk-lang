## Why

Zirk's everyday fractional type, `Float` (an alias of `Float64`), is IEEE 754
binary floating point. Binary floating point cannot represent most base-ten
fractions exactly, so ordinary programs behave surprisingly:

```zirk
inmut a, b: Float64 = 0.1, 0.2;
(a + b) == 0.3;   // false, today
```

This is the wrong default for a language that already chose correctness over
raw hardware behavior elsewhere (no valid `NaN`, division by zero as a
controlled error, checked integer arithmetic). Money, tax, invoicing,
measurement, and configuration values — the bulk of what fractional numbers are
used for outside numerical computing — need exact base-ten arithmetic. Raku and
Groovy already make the exact type the default; binary is opt-in for the code
that genuinely needs it (graphics, ML, DSP, C ABI interop).

This change flips the default: `Float` becomes an exact base-ten decimal type
(integer coefficient plus a decimal scale), and the current IEEE 754 binary
behavior moves to an explicit `BinaryFloat` family for technical use.

## What Changes

- **BREAKING**: `Float` is redefined from "alias of `Float64` (IEEE 754 binary)"
  to an **exact base-ten decimal** scalar. Its value model is a signed 128-bit
  integer coefficient with a non-negative decimal `scale` (value =
  `coefficient x 10^-scale`). `Float` is no longer a member of a "width" family;
  it is a single type.
- **BREAKING**: `Float64` as a spelling is withdrawn as the everyday type name.
  IEEE 754 binary floating point is provided by a **new** family:
  `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, with
  `BinaryFloat` aliasing `BinaryFloat64`. This family inherits, unchanged, every
  semantic the current `Float16..Float128` family has today: exact IEEE widths,
  explicit `POSITIVE_INFINITY`/`NEGATIVE_INFINITY`, no valid `NaN`, `NaN`-would-be
  operations as controlled runtime errors, `Float128` formatting truncation, and
  the Windows verification gap.
- **BREAKING**: an unannotated fractional or scientific literal (`0.1`, `6.25e-2`)
  now infers type `Float` (exact decimal), not `Float64` (binary).
- New literal suffix for binary: `0.1b` / `1.5b` (and `1.5b32`, `1.5b128` for a
  specific width) produce `BinaryFloat` literals. The bare form stays exact.
- Exact `Float` arithmetic: `+ - *` and integer `**` are exact. `/`, `%`, and
  irrational operations (`sqrt`, fractional `pow`) round **half-to-even** to a
  documented maximum significant-digit budget; the rounding mode and target
  scale are selectable through explicit method forms.
- Exact `Float` has **no** `NaN` and **no** infinity. Coefficient overflow past
  128 bits, and division by zero, are controlled runtime errors, consistent with
  integer overflow and current `Float` division-by-zero.
- `Float <-> BinaryFloat` conversions are always explicit and checked. `Int ->
  Float` stays implicit (every integer is an exact decimal); `Float -> Int`
  stays an explicit checked cast.
- `Float.to_string()` produces the exact decimal rendering (no shortest-round-trip
  heuristic needed). Equality is exact after scale alignment, with trailing-zero
  normalization so `1.0 == 1.00`.
- The withdrawn `Decimal*`/`Dec` family stays withdrawn — this change delivers
  its intent under the `Float` name rather than reviving the family.
- Full compiler pipeline support: lexer, AST, parser, checker/type system, IR
  and lowering, LLVM codegen, and a new runtime module for decimal arithmetic
  and formatting.
- Exhaustive documentation update: `docs/ZIRK_LANGUAGE_SPEC.md`,
  `docs/ZIRK_STDLIB_SPEC.md`, `docs/ZIRK_RUNTIME_SPEC.md`,
  `docs/ZIRK_COMPILER_SPEC.md`, `docs/CORE_LANGUAGE_SEMANTICS.md`, the handbook
  `02-handbook/03-everyday-types/` pages, `11-reference` type pages, and every
  `.zrk` example and roadmap mention.
- Exhaustive test battery across parser, checker, IR, codegen, and runtime
  crates proving: exact arithmetic (`0.1 + 0.2 == 0.3`), scale alignment,
  rounding behavior, overflow errors, `Float`/`BinaryFloat` separation and
  conversion, literal inference and the `b` suffix, and `to_string` exactness.

## Capabilities

### New Capabilities
- `exact-decimal-arithmetic`: the value model, arithmetic, rounding policy,
  error conditions, conversions, member surface, and runtime representation of
  the exact base-ten `Float` type, plus the `BinaryFloat` family definition, its
  member surface, and its relationship to the former `Float16..Float128` family.

### Modified Capabilities
- `zirk-scalars`: the floating family requirement is replaced — `Float` is exact
  decimal; `BinaryFloat16..BinaryFloat128` carry the IEEE semantics; mixed
  integer/`Float` arithmetic yields exact `Float`; the unannotated fractional
  literal is `Float` (exact).
- `zirk-type-system`: "Float family replaces Decimal family" and "Fractional
  literal context and mixed arithmetic" requirements are rewritten for the exact
  default and the `BinaryFloat` family; new conversion rules for
  `Float <-> BinaryFloat`.
- `zirk-lexical-syntax`: fractional/scientific literals lex as exact `Float`
  literals; new `b`/`b32`/`b128` binary-float literal suffix.
- `zirk-grammar`: `Float` literal production renamed/retargeted to the exact
  type; binary-float literal grammar added; `Float(expr)` contextual-conversion
  wording updated.
- `zirk-ir-lowering`: exact `Float` lowers to a decimal value plus runtime
  calls; `BinaryFloat` inherits the current `Float` lowering including the
  `NaN`-guard rule; new integer<->decimal and decimal<->binary conversion
  lowering.
- `zirk-native-codegen`: exact `Float` emits a `{ i128, i32 }` value passed by
  pointer and runtime calls; `BinaryFloat` emits the LLVM native `FloatType`
  exactly as `Float` does today.
- `zirk-errors`: the catchable-error catalog gains decimal coefficient overflow
  and inexact-without-rounding conditions; the `NaN`-producing-operation error
  is scoped to `BinaryFloat`.

## Impact

- **Affected crates**: `zirk-lexer`, `zirk-ast`, `zirk-parser`, `zirk-sema`,
  `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime` (new `decimal.rs` module),
  `zirk-cli` (diagnostics/help text), `zirk-diagnostics`.
- **New runtime dependency** (candidate): a fixed-precision decimal crate
  (`rust_decimal`) or a hand-rolled `i128`-coefficient implementation; decided in
  design.
- **Affected public documentation**: yes — normative semantics, syntax examples,
  the everyday-types handbook chapter, the type reference, roadmap wording, and
  multiple `.zrk` examples all change. This is a breaking language-semantics
  change.
- **Companion repository `../zirk-lang-site`**: impacted. After the zirk-lang
  changes land and are committed, run `./scripts/sync-website-content.sh`, review
  the site-owned status catalog for the changed float semantics and limitations,
  and pass `--audit-date YYYY-MM-DD`. The website must not keep describing
  `Float` as IEEE 754 binary.
- **Migration**: every existing `.zrk` file using `Float64`/`Float16`/`Float32`/
  `Float128` names, and any program relying on binary semantics for the bare
  fractional literal, must move to the `BinaryFloat*` names or the `b` suffix.
  A migration note and a checker diagnostic that recognizes the old spellings
  and points at the replacement are in scope.
