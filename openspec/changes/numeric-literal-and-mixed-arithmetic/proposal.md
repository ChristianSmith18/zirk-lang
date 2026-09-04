## Why

Zirk supports ten integer widths, four float widths and `Char`/`String`, but its surface still behaves as if only `Int32` and `Float64` exist. Literals are always `Int32` or `Float64`, arithmetic between two `Int` widths fails, and `++`/`--` only work for `Int32`. This forces users to write `as` casts for the most basic numeric code, while the compiler is perfectly capable of inferring the right width or promoting operands safely. The gap is becoming a real ergonomic and correctness problem now that `String`/`Char` GC and overflow-checked increment are in place.

## What Changes

- **Contextual literal typing**: integer and float literals take the width of their expected type when one is known and the literal fits; e.g. `mut a: Int8 = 1;` and `mut a: Float16 = 1.0;` compile without `as`.
- **Integer ↔ float literal adaptation**: `1` assigned to a `Float` type becomes `1.0`; `1.0` assigned to an integer type becomes `1` when the fractional part is zero and the value fits.
- **Lossless implicit numeric conversions**: an assignment or initialization from one numeric type to another is allowed when the destination can represent every value of the source type (widening); narrowing is allowed only when the compiler can prove the value fits (constant propagation).
- **Mixed-width arithmetic promotion**: `Int8 + Int32`, `UInt16 + UInt32`, `Float16 + Float64`, etc. promote the narrower operand to the wider one and produce the wider type. Cross-family operands (`Int`/`UInt`/`Float`) use the smallest type that can hold both without loss, and produce a clear error when no such type exists (e.g. `UInt64 + Int64`).
- **Loss-safe `Int` to `Float` conversion**: converting an integer to a float is only implicit when the target float can represent every integer value at that width; otherwise `as` or an explicit wider target is required.
- **`++`/`--` for every width**: the implicit `+1`/`-1` uses the operand's exact type, so `Int8`/`UInt64`/`Float` increments work and are overflow-checked at the right width.
- **`String * n` for any integer**: the repeat count accepts any integer width, with implicit promotion to the runtime `i32` count argument.
- **Public status updates**: `docs/init/ZIRK_FEATURE_STATUS.md` and the relevant handbook entries are updated to mark this work as done. `../zirk-lang-site` is synchronized after the commits.

**BREAKING**: Some programs that previously failed with an `as` error will now compile, but no currently valid program becomes invalid. The only observable change for correct code is that numeric code is less verbose.

## Capabilities

### New Capabilities

*None.*

### Modified Capabilities

- `zirk-scalars`: add contextual literal typing, mixed-width and cross-family arithmetic promotion, loss-safe `Int`/`UInt`/`Float` conversion, `++`/`--` width inheritance, and `String * <any integer>`.
- `zirk-type-system`: allow lossless implicit numeric conversions and proven-fits narrowing; update "Absence of implicit conversions" to distinguish numeric value-preserving conversions from general implicit casts.
- `zirk-ir-lowering`: emit widening/promotion `Convert` or `Widen` instructions when binary operands have different widths; lower `++`/`--` with an operand-typed `1` literal.
- `zirk-grammar` **(documentation only)**: update language examples in the handbook to drop unnecessary `as` casts for numeric literals.

## Impact

- `crates/zirk-sema/src/checker.rs`: `check_int_literal`, `check_float_literal`, `check_assign`, `check_binary`, `check_increment` and the numeric conversion helpers.
- `crates/zirk-sema/src/types.rs`: possibly extend `Base`/`Type` helpers for numeric subtyping and promotion.
- `crates/zirk-ir/src/lower.rs`: `lower_binary` and `lower_increment` must insert widening conversions.
- `crates/zirk-codegen-llvm/src/emit.rs`: emit `SExt`, `ZExt`, `FPExt`, `SIToFP`, `UIToFP`, `FPToSI` with overflow/loss checks.
- `crates/zirk-runtime/src/...`: no new runtime work; the existing `zirk_*_checked_*` functions already cover every width, but the count for `zirk_str_repeat` may be widened in codegen.
- `crates/zirk-cli/tests/corpus/` and `crates/zirk-ir/tests/`: new fixtures covering every width combination and edge cases.
- `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/handbook/...`, and `../zirk-lang-site` after the implementation is committed.
