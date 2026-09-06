## Context

Zirk's fractional scalar family today is IEEE 754 binary: `FloatWidth::{F16,F32,F64,F128}`
in `zirk-sema` (`crates/zirk-sema/src/types.rs`) and `zirk-ir`
(`crates/zirk-ir/src/ir.rs`), lowered to LLVM `f16/f32/f64/f128` in
`crates/zirk-codegen-llvm/src/emit.rs`, formatted through `zirk_str_from_f32/f64`
in `crates/zirk-runtime/src/string.rs`, with a member surface in
`crates/zirk-runtime/src/scalar.rs`. Literals are carried as text end to end
(`IrExpr::ConstFloat(FloatWidth, String)`, `ir.rs:988`) and parsed only at
codegen. The `Decimal*` family was deliberately removed from the language
(`types.rs:963`, spec `zirk-type-system` "Float family replaces Decimal family").

The base `Float` work is complete and frozen per the requester; this change
adapts all of it rather than building alongside it.

Existing precedent for exact scaled arithmetic in this codebase: `Duration` is
an `i64` nanosecond count whose cheap operations lower to checked integer IR and
whose non-integer operations (`to_string`, `Float` ratio, scalar cross-type) go
to `zirk_rt_duration_*` runtime helpers (`crates/zirk-runtime/src/duration.rs`).
The exact `Float` type follows the same split.

## Goals / Non-Goals

**Goals:**
- `Float` is an exact base-ten decimal type and the default for unannotated
  fractional literals, so `0.1 + 0.2 == 0.3` holds with no ceremony.
- Preserve every current IEEE 754 binary capability verbatim under a new,
  explicit `BinaryFloat` family — no behavior of the frozen float work is lost,
  only renamed and made opt-in.
- Keep the pipeline shape unchanged: text-carried literals, checker-driven
  conversions, an IR type plus lowering, codegen, and a runtime module.
- Deterministic semantics: no `NaN`, no infinity on `Float`; overflow and
  division by zero are controlled runtime errors already modeled by the error
  system.
- Exhaustive, layered tests that a reader can run to see exactness.

**Non-Goals:**
- Reviving a multi-width `Decimal16..Decimal128` family. There is one exact
  type, `Float`.
- Arbitrary-precision decimal (unbounded coefficient). The coefficient is
  128-bit; exceeding it is a controlled error, not an automatic promotion.
- Exact irrational results. `sqrt`, fractional `pow`, and future transcendental
  functions on `Float` are correctly-rounded approximations to a fixed budget.
- Decimal128 IEEE 754-2008 interchange format or hardware decimal FP.
- Changing integer, `Boolean`, `Char`, `String`, or temporal semantics.
- An automatic source rewriter. Migration is a documented rename plus a checker
  diagnostic that names the replacement.

## Decisions

### D1: Value model — 128-bit signed coefficient + non-negative scale

`Float` is represented as `{ coefficient: i128, scale: u8 }` with value
`coefficient x 10^-scale`. `scale` is clamped to `0..=38` (an `i128` holds at
most 38 decimal digits). `0.1` is `{1, 1}`; `12.340` normalizes to `{1234, 2}`.

- **Range**: ~38 significant decimal digits; largest magnitude ~1.7 x 10^38 at
  scale 0, smallest step 10^-38.
- **Copy, no heap**, does not touch the garbage collector (`collector.rs`),
  matching `Duration`.
- **Normalization**: trailing decimal zeros are stripped after every operation
  (`{1200, 3}` -> `{12, 1}`), so equality can be a direct `(coef, scale)` compare
  after aligning scales, and `1.0 == 1.00`. Rationale: user-visible equality
  must not depend on how a literal was written. Java `BigDecimal` keeps trailing
  zeros in `equals`; that trips people constantly, so we do not.

_Alternatives considered:_
- **`rust_decimal` (96-bit mantissa, scale 0..28)**: fewer digits, an external
  dependency with its own rounding surface, and a `Decimal` type whose API we
  would still wrap. Rejected for control and for matching the `i128` idiom
  already in the runtime (`i128` scalars, `Duration`).
- **Arbitrary-precision (`BigInt` coefficient)**: heap allocation, GC
  interaction, slower, unbounded blow-up on repeated multiplication. Deferred;
  the value model is forward-compatible if it is ever wanted.
- **Rational `num/den`**: exact division too, but `1/3` retained exactly leads
  to denominator blow-up and expensive comparison, and it is not "decimal". Out
  of scope.

### D2: `BinaryFloat` family = today's `Float` family, re-surfaced

At the **language surface** the IEEE 754 binary family is `BinaryFloat16`,
`BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, with `BinaryFloat` aliasing
`BinaryFloat64`. All current semantics carry over: exact IEEE widths,
`POSITIVE_INFINITY`/`NEGATIVE_INFINITY`, no valid `NaN`, `NaN`-would-be
operations as controlled runtime errors, `Float128` text truncation to `f64`
precision, and the pending Windows verification for `Float128`.

**Implementation choice — the internal Rust identifiers keep the name `Float`.**
`zirk-sema`'s `Base::Float(FloatWidth)`, `zirk-ir`'s `IrType::Float` /
`InstKind::ConstFloat` / `IntToFloat` / `FloatCast` / `FloatToInt`, and the
`zirk-codegen-llvm` / `zirk-runtime` `zirk_*_f32/f64` symbols are unchanged: they
now denote the binary family, whose surface name is `BinaryFloat*`. A wholesale
rename of ~30 files and every golden test was judged pure churn against the
behavioural goal; OpenSpec specs govern language behaviour, not Rust
identifiers. Each of those types/modules gains a doc line stating "surface name:
`BinaryFloat*`". The exact type is the genuinely new one and is named
`Decimal` internally (`Base::Decimal`, `IrType::Decimal`,
`InstKind::ConstDecimal`), surface name `Float`.

The names `Float16`, `Float32`, `Float64`, `Float128` no longer resolve:
`Type::from_name` returns `None` and the checker emits a redirect diagnostic —
_"`Float64` is now `BinaryFloat64`; the exact base-ten type is `Float`"_.
`"BinaryFloat"` and `"BinaryFloat16..128"` resolve to `Base::Float(width)`;
`"Float"` resolves to `Base::Decimal`.

_Alternative considered:_ the full internal rename per the original task list.
Deferred to a separate mechanical pass — it is behaviour-neutral and safest done
on its own, not folded into this change.

### D3: Literals

- Bare fractional / scientific literal -> `Float` (exact). The lexer keeps
  producing a text-valued token; only the token's kind/typing changes.
- Binary-float literal suffix: `b` (default width 64), `b16`, `b32`, `b64`,
  `b128`. `1.5b`, `1.5b32`, `0.1b128`. Chosen over `f` because `f` collides with
  the reading "the exact one is the real float". `1.5b` reads as "binary".
- Scientific literals: `6.25e-2` is exact `Float` `{625, 4}`. A literal whose
  exact form needs more than 38 digits of coefficient (`1e300`, `1.23e-40`) is a
  compile error suggesting a `b` suffix. Rationale: silently falling back to
  binary would defeat the "bare literal is exact" guarantee.
- `Float(expr)` contextual constructor keeps its meaning (establish a conversion
  domain over the contained operator tree), now converting operands to exact
  `Float`. `BinaryFloat(expr)` is the binary counterpart.

### D4: Arithmetic and rounding

- `+`, `-`: align to `max(scaleA, scaleB)` by multiplying the smaller-scale
  coefficient by `10^diff` (checked; overflow -> `ArithmeticOverflowError`), then
  add/subtract coefficients (checked), then normalize.
- `*`: `coef = coefA * coefB` (checked `i128`; on overflow, retry via an `i256`
  intermediate and round the result back into 38 digits half-to-even; if the
  rounded magnitude still overflows, `ArithmeticOverflowError`), `scale =
  scaleA + scaleB`, then normalize.
- `/`: result scale is `max(scaleA, scaleB, MIN_DIV_SCALE)` capped at
  `MAX_SIGNIFICANT_DIGITS`; compute `(coefA * 10^k) / coefB` with the remainder feeding
  half-to-even rounding. Divisor zero -> `DivisionByZeroError` (guard emitted in
  lowering, as `Duration` does).
- `%`: exact (`a - (a / b).truncate() * b` computed on aligned coefficients).
- `**` with a non-negative integer exponent: exact repeated multiply. Negative
  or fractional exponent: rounded like `/`.
- `sqrt`, fractional `pow`: correctly-rounded to `MAX_SIGNIFICANT_DIGITS` half-to-even;
  negative `sqrt` input is a controlled error (unchanged from today).
- Rounding constants: `MAX_SIGNIFICANT_DIGITS = 28` (the `.NET decimal` /
  `rust_decimal` budget, proven for money/base-ten domains; leaves headroom
  below the 38-digit `i128` ceiling), `MIN_DIV_SCALE = 10`. Both are documented
  normative values. Exact `+ - *` may use the full 38-digit coefficient; only
  inexact results are rounded into the 28-digit budget.
- Explicit control: `a.div(b, mode, places)`, `a.round(places, mode)`,
  `RoundingMode` = `HALF_EVEN | HALF_UP | HALF_DOWN | UP | DOWN | CEIL | FLOOR`.
  Default mode everywhere is `HALF_EVEN` (banker's rounding — no systematic
  bias, matches SQL `NUMERIC` and IEEE decimal default).

_Alternative considered:_ make `/` a controlled error when the quotient does not
terminate. Rejected — too hostile for everyday use; half-to-even to a generous
budget is the SQL/`.NET` norm and is what people expect.

### D5: No `NaN`, no infinity on `Float`

Consistent with the language's existing stance. There is no `Float.NaN` and no
`Float.POSITIVE_INFINITY`. Operations that would need them are controlled runtime
errors:
- coefficient overflow past `i128` after rounding -> `ArithmeticOverflowError`
  (reuse the integer-overflow error).
- division / modulo by zero -> `DivisionByZeroError` (reuse).
- `sqrt` of a negative -> the existing float-domain error.
Infinity remains available, unchanged, on `BinaryFloat`.

### D6: Conversions

| From → To | Rule |
| --- | --- |
| `Int* → Float` | implicit, always exact (`{value, 0}`, widening `i128` first) |
| `Float → Int*` | explicit `as` cast, checked: non-integer or out-of-range -> controlled error |
| `Float → BinaryFloat*` | explicit `as` / `BinaryFloat(x)`, may lose precision; documented |
| `BinaryFloat* → Float` | explicit `as` / `Float(x)`, checked: non-finite input -> controlled error; value is the exact decimal of the binary bits, then rounded to `MAX_SIGNIFICANT_DIGITS` |
| `Float ↔ String` | `Float.parse(text) -> Result<Float, ParseError>`, `x.to_string()` exact |

No implicit `Float`/`BinaryFloat` mixing in arithmetic — a binary operand in a
`Float` expression is a type error naming the explicit conversion. Rationale:
silently importing binary imprecision into an exact computation is exactly the
bug this change removes.

### D7: IR and lowering (`zirk-ir`)

- New `IrType::Decimal` (spelled `Float` at the surface). `IrExpr::ConstDecimal(String)`
  carries the literal text, parsed to `{i128,u8}` in the runtime/codegen, same
  rationale as `ConstFloat` today.
- Arithmetic on `Decimal` lowers to `zirk_rt_decimal_*` calls (add, sub, mul,
  div, rem, pow_i, neg, abs, cmp, sqrt, pow, round, div_ex). Cheap ops are not
  inlined as `i128` IR because scale alignment + normalization + overflow retry
  is too much to open-code per site; centralizing it in the runtime matches
  `Duration`'s non-integer ops.
- Division/modulo by zero: guard instruction emitted in lowering that raises
  `DivisionByZeroError` before the call, mirroring `Duration`.
- New conversion instructions: `IntToDecimal`, `DecimalToInt` (checked),
  `DecimalToBinaryFloat`, `BinaryFloatToDecimal` (checked).
- The `Float(expr)` deep-context rule and the `NaN`-guard rule are retargeted:
  the guard now applies to `BinaryFloat` only.

### D8: Codegen ABI (`zirk-codegen-llvm`)

- `Float` is the LLVM struct `{ i128, i8 }` (`iN` padding as the target
  requires). Passed and returned **by pointer** in the C ABI for the runtime
  calls, matching how `Int128`/`UInt128` already cross the boundary
  (`STR_FROM_I128`, "no stable cross-target ABI for a by-value 128-bit").
- Constants: emit the parsed `{coef, scale}` as a struct constant; do not route
  through any binary float.
- New runtime symbols registered in `crates/zirk-codegen-llvm/src/runtime.rs`:
  `zirk_rt_decimal_add/sub/mul/div/rem/pow_i/neg/abs/cmp/sqrt/pow/round/div_ex`,
  `zirk_str_from_decimal`, `zirk_rt_decimal_parse_ok/value`,
  `zirk_rt_decimal_from_i128`, `zirk_rt_decimal_to_i128_checked`,
  `zirk_rt_decimal_from_f64`, `zirk_rt_decimal_to_f64`.
- `BinaryFloat` codegen is the current `Float` codegen, unchanged.

### D9: Runtime (`zirk-runtime`)

- New module `crates/zirk-runtime/src/decimal.rs`, sibling of `duration.rs`.
  `#[repr(C)] pub struct Decimal { coef: i128, scale: u8 }`.
- Implementation: hand-rolled over `i128`, with a small internal hand-rolled
  `u256` (four `u64` limbs, `mul` + `div_rem`, no new crate) only for the
  multiply/divide intermediates.
- Constants: `MAX_SIGNIFICANT_DIGITS = 28` (rounding budget for inexact
  results), `MIN_DIV_SCALE = 10`, coefficient hard limit 38 decimal digits.
- `zirk_str_from_decimal`: format `coef.abs()` as digits, insert the point
  `scale` from the right with left zero-fill, prepend `-` if negative. No `ryu`.
- Errors raised through the existing failure path (`crates/zirk-runtime/src/failure.rs`,
  as `zirk_rt_float_nan` does) for overflow / bad conversion.
- `zirk-runtime/src/lib.rs` re-exports the new symbols.

### D10: Member surface (`zirk-standard-library` spec + `scalar.rs`)

Exact `Float` surface: `abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `floor`,
`ceil`, `round` / `round(places[, mode])`, `truncate`, `fraction`, `pow(int)`,
`pow(Float[, mode])`, `sqrt`, `div(other, mode, places)`, `scale` (property, the
decimal scale), `is_integer`, `parse`, `to_string`. `format(spec)` stays
"specified, not implemented", as it is for `Float` today. Removed vs binary:
`is_finite`, `is_infinite`, `POSITIVE_INFINITY`, `NEGATIVE_INFINITY`, `EPSILON`
(no meaning for exact decimal). `is_negative` kept.

`BinaryFloat` surface is exactly today's `Float` surface (including `EPSILON`,
infinities, `is_finite`/`is_infinite`).

### D11: Documentation

- Normative: `docs/ZIRK_LANGUAGE_SPEC.md` §3 numeric families and §on `NaN`,
  `docs/ZIRK_STDLIB_SPEC.md` float section, `docs/ZIRK_RUNTIME_SPEC.md` (new
  decimal runtime helpers), `docs/ZIRK_COMPILER_SPEC.md` pipeline notes,
  `docs/CORE_LANGUAGE_SEMANTICS.md`.
- Handbook: rewrite `docs/handbook/02-handbook/03-everyday-types/04-decimals.md`
  as "Exact decimals (`Float`)", add a `BinaryFloat` page, update
  `05-numeric-literals.md`, `README.md` of that chapter, `SUMMARY.md`,
  `11-reference/03-built-in-types.md`, `11-reference/13-type-member-index.md`,
  `12-explanations` if it discusses float precision.
- Examples: `main.zrk`, `main2.zrk`, `main3.zrk`, `numerics.zrk`, `hello.zrk`,
  every handbook code block, every `.zrk` under `docs/`.
- Roadmap / status: `docs/init/ZIRK_ROADMAP.md`, Feature Status, Current
  Limitations, handbook roadmap.
- Companion site: run `./scripts/sync-website-content.sh` after commit, review
  the status catalog, pass `--audit-date`.

## Risks / Trade-offs

- **[Large breaking surface — every float program changes]** → single migration
  section in the handbook; checker diagnostic maps `Float64`→`BinaryFloat64` and
  bare-literal binary assumptions to the `b` suffix; land as one atomic change so
  no interim state has both meanings.
- **[Performance regression for numeric-heavy code that used the bare literal]** →
  `BinaryFloat` is a one-suffix opt-in with identical performance to today;
  documented prominently ("use `BinaryFloat` for graphics/ML/DSP"). A checker
  hint on hot patterns (`sqrt`/`pow` loops on `Float`) is a stretch goal.
- **[`i128` coefficient overflow surprises users mid-expression]** → normalize
  aggressively (strip trailing zeros) to reclaim digits; `*` uses an `i256`
  intermediate before declaring overflow; the error message states the digit
  budget and suggests `BinaryFloat` or explicit `round`.
- **[Rounding-mode default is a semantic commitment]** → `HALF_EVEN` is the
  SQL/IEEE-decimal norm and unbiased; explicit `mode` args on `div`/`round`/`pow`
  give an escape hatch; documented as normative.
- **[Two decimal-arithmetic implementations if `rust_decimal` is used somewhere
  else later]** → decide the dependency in tasks; default to hand-rolled `i128`
  for one code path and zero new crates.
- **[`Float128` LLVM `f128` support and Windows gap inherited]** → unchanged; it
  now rides on `BinaryFloat128` and the existing limitation text moves with it.
- **[Companion website drifts]** → explicit task, gated on `--audit-date`, must
  be done before the change is archived.

## Migration Plan

1. Land the whole change atomically on a branch: compiler + runtime + specs +
   docs + examples + website sync in one workstream.
2. `Float64`/`Float16`/`Float32`/`Float128` stop resolving; the checker emits a
   fix-it diagnostic naming `BinaryFloat*`.
3. Programs that assumed binary behavior from a bare `0.1` literal and depended
   on it (rare, and usually a latent bug) add the `b` suffix.
4. Update in-repo `.zrk` examples and handbook code blocks in the same change.
5. Publish a "Float is now exact" migration note in the handbook and the
   changelog.
6. **Rollback**: revert the branch. Because the change is atomic and there is no
   data/state involved (compiler + docs only), rollback is a git revert plus a
   website re-sync to the prior commit.

## Open Questions (resolved)

- **256-bit intermediate**: hand-rolled `u256` (four `u64` limbs) `mul` and
  `div_rem` in `decimal.rs`, no new crate. Rationale: Zirk keeps a deliberately
  tiny dependency tree (`Cargo.toml` ~1KB); the intermediate is only needed for
  `mul` overflow recovery and the `div` scaled numerator, ~60 lines, fully
  testable. `ethnum`/`rust_decimal` rejected — external surface for a contained
  need.
- **Literal suffix**: `b`, `b16`, `b32`, `b64`, `b128`. No hex-float form exists
  in Zirk, so `b` is unambiguous; a float literal always carries a `.` or `e`,
  so `1e2b` parses cleanly. Confirmed.
- **`MAX_SIGNIFICANT_DIGITS` = 28, `MIN_DIV_SCALE` = 10.** 28 significant digits
  is the `.NET decimal` / `rust_decimal` budget, proven for money and base-ten
  domains — the exact use the handbook names. Exact `+ - *` results may use the
  full 38-digit coefficient; only inexact results (`/`, `sqrt`, fractional
  `pow`) are rounded into the 28-digit budget. `MIN_DIV_SCALE = 10` guarantees
  a division computes at least 10 fractional digits before rounding (covers
  currency plus rate math).
- **`Float.scale` is public** — a well-defined exact property, useful for
  formatting and tests.
- **No early literal-size warning** for v1. A suffix-less literal that does not
  fit 38 digits of coefficient is a hard error suggesting `b`; nothing between.
- **`format(spec)` is deferred.** It is already "specified, not implemented" for
  the current `Float`; this change keeps that status. Scope here is `to_string`
  (exact) and `round(places[, mode])`. A follow-up change defines the format
  mini-language for both `Float` and `BinaryFloat` together.
