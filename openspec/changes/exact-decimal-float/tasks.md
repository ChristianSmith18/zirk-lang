## 1. Decisions locked before coding

- [x] 1.1 256-bit intermediate: hand-rolled `u256` (four `u64` limbs, `mul` + `div_rem`) in `decimal.rs`, no new crate
- [x] 1.2 Normative constants locked: `MAX_SIGNIFICANT_DIGITS = 28`, `MIN_DIV_SCALE = 10`, coefficient hard limit 38 digits (recorded in `design.md`)
- [x] 1.3 Binary-float literal suffix locked: `b` / `b16` / `b32` / `b64` / `b128`
- [x] 1.4 `Float.scale` is a public member
- [x] 1.5 `format(spec)` deferred — stays "specified, not implemented"; scope is `to_string` + `round(places[, mode])`

## 2. Runtime: exact-decimal core (`zirk-runtime`)

- [ ] 2.1 Create `crates/zirk-runtime/src/decimal.rs` with `#[repr(C)] struct Decimal { coef: i128, scale: u8 }` and internal normalization (strip trailing zeros, clamp scale)
- [ ] 2.2 Implement the 256-bit intermediate helper (per 1.1) for multiply/divide
- [ ] 2.3 Implement `zirk_rt_decimal_add` / `sub` with scale alignment + checked combine + normalize
- [ ] 2.4 Implement `zirk_rt_decimal_mul` with `i256` intermediate and half-to-even reduction to the digit budget; `ArithmeticOverflowError` on irrecoverable overflow
- [ ] 2.5 Implement `zirk_rt_decimal_div` (default half-to-even, `MIN_DIV_SCALE`..`MAX_SIGNIFICANT_DIGITS`) and `zirk_rt_decimal_div_ex` (explicit mode + places); zero divisor handled by the lowering guard
- [ ] 2.6 Implement `zirk_rt_decimal_rem` (exact, dividend sign), `neg`, `abs`, `cmp`
- [ ] 2.7 Implement `zirk_rt_decimal_pow_i` (exact repeated multiply) and `zirk_rt_decimal_pow` / `zirk_rt_decimal_sqrt` (correctly rounded to `MAX_SIGNIFICANT_DIGITS`; negative `sqrt` -> controlled float-domain error)
- [ ] 2.8 Implement `zirk_rt_decimal_round(value, places, mode)` and the `RoundingMode` enum encoding
- [ ] 2.9 Implement `zirk_str_from_decimal` — exact digit-string formatting with the point inserted `scale` from the right, no `ryu`, no binary intermediate
- [ ] 2.10 Implement `zirk_rt_decimal_parse_ok` / `zirk_rt_decimal_parse_value` for `Float.parse`
- [ ] 2.11 Implement conversions: `zirk_rt_decimal_from_i128`, `zirk_rt_decimal_to_i128_checked`, `zirk_rt_decimal_from_f64` (checked non-finite), `zirk_rt_decimal_to_f64`
- [ ] 2.12 Route overflow / bad-conversion failures through `crates/zirk-runtime/src/failure.rs` (as `zirk_rt_float_nan` does), raising `ArithmeticOverflowError` / conversion error
- [ ] 2.13 Re-export every new symbol from `crates/zirk-runtime/src/lib.rs`
- [ ] 2.14 Unit tests in `decimal.rs`: `0.1 + 0.2 == 0.3`, scale alignment, normalization, half-to-even at boundaries, overflow error, formatting exactness, parse round-trip, all conversions

## 3. Runtime: rename binary float surface

- [ ] 3.1 Keep `zirk_str_from_f32` / `zirk_str_from_f64` and the `zirk_float_*` helpers in `scalar.rs` unchanged in behavior; add doc notes that they now back `BinaryFloat*`
- [ ] 3.2 Split the exact-`Float` member helpers (`abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `is_negative`, `is_integer`, `floor`, `ceil`, `truncate`, `fraction`) into decimal versions in `decimal.rs`
- [ ] 3.3 Runtime spec update: add the decimal helpers section to `docs/ZIRK_RUNTIME_SPEC.md`

## 4. Lexer + AST + parser

- [ ] 4.1 `zirk-lexer`: lex a suffix-less fractional/scientific literal as an exact-decimal `Float` token (text preserved); lex `b`/`b16`/`b32`/`b64`/`b128` as a `BinaryFloat` token with width
- [ ] 4.2 `zirk-lexer`: reject a suffix-less fractional literal whose exact form needs more than the coefficient digit budget, with a diagnostic suggesting a `b` suffix
- [ ] 4.3 `zirk-ast`: add `DecimalLit { text }`; rename `FloatLit` usage to represent `BinaryFloatLit { text, width }`
- [ ] 4.4 `zirk-parser`: parse both literal kinds; retain the operator tree for `Float(expr)` and add `BinaryFloat(expr)` contextual constructor
- [ ] 4.5 Parser + lexer tests: literal inference, suffix widths, oversized-literal diagnostic, contextual constructor trees

## 5. Type system (`zirk-sema`)

- [ ] 5.1 `types.rs`: rename `Base::Float(FloatWidth)` -> `Base::BinaryFloat(BinaryFloatWidth)`; add `Base::Decimal` for exact `Float`; add `Type::FLOAT` (= `Decimal`) and `Type::BINARY_FLOAT64`
- [ ] 5.2 `types.rs` `from_name`: `Float` -> exact decimal; `BinaryFloat`/`BinaryFloat16..128` -> binary; `Float16/32/64/128` -> `None`
- [ ] 5.3 `types.rs`: remove `Float*` from the pending list; keep `Decimal*`/`Dec` in the "not a type" list; update the `accepts` rules — `Int -> Decimal` implicit exact, `Decimal <-> BinaryFloat` explicit only, no `Decimal`/`BinaryFloat` mixing
- [ ] 5.4 `checker.rs`: infer suffix-less fractional literal as exact `Float`; mixed `Int`/`Float` arithmetic -> exact `Float`; reject `Float`/`BinaryFloat` mixed arithmetic with a fix-it naming the conversion
- [ ] 5.5 `checker.rs`: redirect diagnostic for `Float16/32/64/128` -> `BinaryFloat*` and `Float`
- [ ] 5.6 `checker.rs`: member-surface resolution — exact `Float` members per spec (no `EPSILON`/infinity/`is_finite`/`is_infinite`; add `scale`, `is_integer`, `round(places[,mode])`, `div(other,mode,places)`); `BinaryFloat` members = today's `Float` members
- [ ] 5.7 `checker.rs`: `Float(expr)` establishes exact-decimal deep context; `BinaryFloat(expr)` establishes binary deep context
- [ ] 5.8 `checker.rs`: scope the `NaN`-would-be error to `BinaryFloat`; add coefficient-overflow and checked-conversion error paths for `Float`
- [ ] 5.9 `zirk-sema` tests (`typing.rs`): every rule above, including `0.1 + 0.2` typing, redirect diagnostics, conversion matrix, member surfaces

## 6. IR + lowering (`zirk-ir`)

- [ ] 6.1 `ir.rs`: rename `IrType::Float` -> `IrType::BinaryFloat`, `IrExpr::ConstFloat` -> `ConstBinaryFloat`; add `IrType::Decimal` and `IrExpr::ConstDecimal(String)`
- [ ] 6.2 `ir.rs`: add conversion instructions `IntToDecimal`, `DecimalToInt`, `DecimalToBinaryFloat`, `BinaryFloatToDecimal`
- [ ] 6.3 `lower.rs`: lower `Decimal` arithmetic/comparison/rounding/parse/format to `zirk_rt_decimal_*` calls
- [ ] 6.4 `lower.rs`: emit the zero-divisor guard before `Decimal` `/` and `%` (mirror `Duration` division lowering)
- [ ] 6.5 `lower.rs`: lower integer operand -> exact `Float` at scale 0 in mixed arithmetic; lower the two contextual-constructor domains
- [ ] 6.6 `lower.rs`: keep `BinaryFloat` lowering identical to today's `Float` lowering, including the `NaN` guard
- [ ] 6.7 `zirk-ir` tests (`lowering.rs`): decimal op lowering, divide-by-zero guard, no `NaN` check on `Decimal`, conversions, both contextual domains

## 7. Codegen (`zirk-codegen-llvm`)

- [ ] 7.1 `emit.rs`: emit `IrType::Decimal` as `{ i128, i8 }` aggregate; pass/return by pointer across the runtime boundary (mirror `STR_FROM_I128`)
- [ ] 7.2 `emit.rs`: emit `ConstDecimal` from the parsed `{coef, scale}` directly, never via a binary float
- [ ] 7.3 `emit.rs`: emit calls for every decimal runtime helper and the four conversion instructions
- [ ] 7.4 `emit.rs`: keep `IrType::BinaryFloat` emission identical to today's `Float` (LLVM `f16/f32/f64/f128`, explicit `NaN` guard)
- [ ] 7.5 `runtime.rs`: register every `zirk_rt_decimal_*`, `zirk_str_from_decimal`, `zirk_rt_decimal_parse_*`, and conversion symbol
- [ ] 7.6 `zirk-codegen-llvm` tests (`emission.rs`): decimal constant, decimal arithmetic dispatch, by-pointer ABI, binary-width mapping unchanged

## 8. CLI + diagnostics

- [ ] 8.1 `zirk-diagnostics` / `zirk-cli`: wording for the `Float64`->`BinaryFloat64` redirect, the oversized-literal error, and the `Float`/`BinaryFloat` mixing error
- [ ] 8.2 `zirk-cli` help / examples text: replace `Float64` mentions

## 9. Documentation

- [ ] 9.1 `docs/ZIRK_LANGUAGE_SPEC.md`: rewrite the numeric-families section (§3) — `Float` exact, `BinaryFloat*` binary, literal forms, `NaN`/infinity scoping
- [ ] 9.2 `docs/ZIRK_STDLIB_SPEC.md`: rewrite the float section — exact `Float` member surface + `BinaryFloat` member surface
- [ ] 9.3 `docs/ZIRK_COMPILER_SPEC.md` and `docs/CORE_LANGUAGE_SEMANTICS.md`: pipeline + semantics notes for the exact type and the rename
- [ ] 9.4 `docs/ZIRK_RUNTIME_SPEC.md`: decimal runtime helpers (also covered by 3.3)
- [ ] 9.5 Handbook: rewrite `02-handbook/03-everyday-types/04-decimals.md` as "Exact decimals (`Float`)"; add a `BinaryFloat` page; update `05-numeric-literals.md`, that chapter's `README.md`
- [ ] 9.6 Handbook: update `11-reference/03-built-in-types.md`, `11-reference/13-type-member-index.md`, `SUMMARY.md`, and any `12-explanations` page discussing float precision
- [ ] 9.7 Handbook: add a "Float is now exact — migration" note (rename table + `b` suffix)
- [ ] 9.8 Roadmap/status: `docs/init/ZIRK_ROADMAP.md`, Feature Status, Current Limitations, handbook roadmap — move the `Float128` truncation + Windows-verification limitation onto `BinaryFloat128`
- [ ] 9.9 Examples: update `main.zrk`, `main2.zrk`, `main3.zrk`, `numerics.zrk`, `hello.zrk` and every `.zrk` code block under `docs/` to the new names/semantics

## 10. End-to-end test battery

- [ ] 10.1 Fixture: `0.1 + 0.2 == 0.3` prints `true`; `(0.1).to_string()` is `"0.1"`; sum prints `"0.3"`
- [ ] 10.2 Fixture: exact `+ - *` across differing scales; integer `**`; normalization (`1.0 == 1.00`, `2.50 == 2.5`)
- [ ] 10.3 Fixture: `1.0 / 3.0` rounds half-to-even at `MAX_SIGNIFICANT_DIGITS`; `a.div(b, mode, places)` variants; `round(places[, mode])` at tie boundaries
- [ ] 10.4 Fixture: `%` sign, `sqrt` exact + negative error, `pow` fractional rounding
- [ ] 10.5 Fixture: coefficient overflow raises catchable `ArithmeticOverflowError`; `Float / 0.0` raises catchable `DivisionByZeroError`
- [ ] 10.6 Fixture: `Int -> Float` implicit; `Float -> Int` checked cast error on fractional; `Float(binaryValue)` and `x as BinaryFloat64` explicit; `Float + BinaryFloat64` is a type error
- [ ] 10.7 Fixture: `1.5b`, `1.5b32`, `0.1b128` type and behave as the old `Float*`; `0.0b / 0.0b` catchable `FloatNanError`; non-zero `/ 0.0b` is infinity
- [ ] 10.8 Fixture: `Float64` annotation emits the redirect diagnostic; oversized suffix-less literal emits the digit-budget diagnostic
- [ ] 10.9 Fixture: `Float.parse` success and failure; `format(spec)` per 1.5
- [ ] 10.10 Wire all fixtures into the CLI end-to-end test suite with expected stdout/diagnostics

## 11. Companion website

- [ ] 11.1 After the zirk-lang changes are committed, run `./scripts/sync-website-content.sh`
- [ ] 11.2 Review `../zirk-lang-site` site-owned status catalog for the changed float semantics and limitations; re-run with `--audit-date YYYY-MM-DD`
- [ ] 11.3 Verify the website no longer describes `Float` as IEEE 754 binary and describes `BinaryFloat*` correctly

## 12. Close-out

- [ ] 12.1 `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test` green across all crates
- [ ] 12.2 `cargo clippy --all-targets` clean
- [ ] 12.3 `openspec validate exact-decimal-float` passes; run `/opsx:archive` after review
