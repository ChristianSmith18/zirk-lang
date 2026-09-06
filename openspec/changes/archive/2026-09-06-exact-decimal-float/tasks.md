## 1. Decisions locked before coding

- [x] 1.1 256-bit intermediate: hand-rolled `u256` (four `u64` limbs, `mul` + `div_rem`) in `decimal.rs`, no new crate
- [x] 1.2 Normative constants locked: `MAX_SIGNIFICANT_DIGITS = 28`, `MIN_DIV_SCALE = 10`, coefficient hard limit 38 digits (recorded in `design.md`)
- [x] 1.3 Binary-float literal suffix locked: `b` / `b16` / `b32` / `b64` / `b128`
- [x] 1.4 `Float.scale` is a public member
- [x] 1.5 `format(spec)` deferred — stays "specified, not implemented"; scope is `to_string` + `round(places[, mode])`

## 2. Runtime: exact-decimal core (`zirk-runtime`)

- [x] 2.1 Create `crates/zirk-runtime/src/decimal.rs` with `#[repr(C)] struct Decimal { coef: i128, scale: u8 }` and internal normalization (strip trailing zeros, clamp scale)
- [x] 2.2 Implement the 256-bit intermediate helper — hand-rolled `U256` (`mul_u128`, `divrem_u256`, `u256_mul10`, `scale_u256`, `add_one_u256`)
- [x] 2.3 Implement `zirk_rt_decimal_add` / `sub` with scale alignment + checked combine + normalize
- [x] 2.4 Implement `zirk_rt_decimal_mul` with 256-bit intermediate and half-to-even reduction to the digit budget; `ArithmeticOverflowError` on irrecoverable overflow
- [x] 2.5 Implement `zirk_rt_decimal_div` (default: quotient carries ~`MAX_SIGNIFICANT_DIGITS` significant digits, half-to-even) and `zirk_rt_decimal_div_ex` (explicit mode + places); zero divisor handled by the lowering guard
- [x] 2.6 Implement `zirk_rt_decimal_rem` (exact, dividend sign), `neg`, `abs`, `cmp`, plus `min`/`max`/`clamp`/`sign`/`scale`
- [x] 2.7 Implement `zirk_rt_decimal_pow_i` (exact repeated multiply, exact-square path) and `zirk_rt_decimal_pow` / `zirk_rt_decimal_sqrt` (f64-precision result; negative `sqrt` -> `zirk_rt_decimal_domain`)
- [x] 2.8 Implement `zirk_rt_decimal_round(value, places, mode)` and the `RoundingMode` ABI encoding (0..6 half-even/half-up/half-down/up/down/ceil/floor)
- [x] 2.9 Implement `zirk_str_from_decimal` — exact digit-string formatting with the point inserted `scale` from the right, no `ryu`, no binary intermediate
- [x] 2.10 Implement `zirk_rt_decimal_parse_ok` / `zirk_rt_decimal_parse_value` for `Float.parse`, plus `zirk_rt_decimal_from_literal` for `ConstDecimal`
- [x] 2.11 Implement conversions: `zirk_rt_decimal_from_i128`, `zirk_rt_decimal_to_i128_checked`, `zirk_rt_decimal_from_f64` (checked non-finite), `zirk_rt_decimal_to_f64`
- [x] 2.12 Route overflow / bad-conversion / domain failures through `failure.rs` (`zirk_rt_overflow`, `zirk_rt_invalid_cast`, `zirk_rt_division_by_zero`, new `zirk_rt_decimal_domain`)
- [x] 2.13 `decimal` module added to `lib.rs` (no-mangle symbols are emitted from the compiled module, as `duration` does; `pub use` not required)
- [x] 2.14 Unit tests in `decimal.rs` (14): `0.1 + 0.2 == 0.3`, scale alignment, normalization, half-to-even boundaries, wide multiply, formatting exactness, parse rejection, conversions, sqrt

## 3. Runtime: rename binary float surface

- [x] 3.1 Keep `zirk_str_from_f32` / `zirk_str_from_f64` and the `zirk_float_*` helpers in `scalar.rs` unchanged in behavior; add doc notes that they now back `BinaryFloat*`
- [x] 3.2 Exact-`Float` member helpers implemented in `decimal.rs` (`abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `is_negative`, `is_integer`, `floor`, `ceil`, `truncate`, `fraction`, `scale`)
- [x] 3.3 Runtime spec update: add the decimal helpers section to `docs/ZIRK_RUNTIME_SPEC.md`

## 4. Lexer + AST + parser

- [x] 4.1 `zirk-lexer`: lex a suffix-less fractional/scientific literal as an exact-decimal `Float` token (text preserved); lex `b`/`b16`/`b32`/`b64`/`b128` as a `BinaryFloat` token with width
- [x] 4.2 `zirk-lexer`: reject a suffix-less fractional literal whose exact form needs more than the coefficient digit budget, with a diagnostic suggesting a `b` suffix
- [x] 4.3 `zirk-ast`: add an exact-decimal literal node; the existing float-literal node keeps its Rust name and now carries the `b`-suffix width (surface `BinaryFloat`)
- [x] 4.4 `zirk-parser`: parse both literal kinds; retain the operator tree for `Float(expr)` and add `BinaryFloat(expr)` contextual constructor
- [x] 4.5 Parser + lexer tests: literal inference, suffix widths, oversized-literal diagnostic, contextual constructor trees

## 5. Type system (`zirk-sema`)

- [x] 5.1 `types.rs`: add `Base::Decimal` (exact `Float`, no width); add `Type::FLOAT` (= `Decimal`). `Base::Float(FloatWidth)` keeps its Rust name (surface name `BinaryFloat*`) — doc note only (per design D2)
- [x] 5.2 `types.rs` `from_name`: `Float` -> `Base::Decimal`; `BinaryFloat`/`BinaryFloat16..128` -> `Base::Float(width)`; `Float16/32/64/128` -> `None`; `FloatWidth::name()` returns `BinaryFloat*`
- [x] 5.3 `types.rs`: remove `Float*` from the pending list; keep `Decimal*`/`Dec` in the "not a type" list; update the `accepts` rules — `Int -> Decimal` implicit exact, `Decimal <-> BinaryFloat` explicit only, no `Decimal`/`BinaryFloat` mixing
- [x] 5.4 `checker.rs`: infer suffix-less fractional literal as exact `Float`; mixed `Int`/`Float` arithmetic -> exact `Float`; reject `Float`/`BinaryFloat` mixed arithmetic with a fix-it naming the conversion
- [x] 5.5 `checker.rs`: redirect diagnostic for `Float16/32/64/128` -> `BinaryFloat*` and `Float`
- [x] 5.6 `checker.rs`: member-surface resolution — exact `Float` members per spec (no `EPSILON`/infinity/`is_finite`/`is_infinite`; add `scale`, `is_integer`, `round(places[,mode])`, `div(other,mode,places)`); `BinaryFloat` members = today's `Float` members
- [x] 5.7 `checker.rs`: `Float(expr)` establishes exact-decimal deep context; `BinaryFloat(expr)` establishes binary deep context
- [x] 5.8 `checker.rs`: scope the `NaN`-would-be error to `BinaryFloat`; add coefficient-overflow and checked-conversion error paths for `Float`
- [x] 5.9 `zirk-sema` tests (`typing.rs`): every rule above, including `0.1 + 0.2` typing, redirect diagnostics, conversion matrix, member surfaces

## 6. IR + lowering (`zirk-ir`)

- [x] 6.1 `ir.rs`: add `IrType::Decimal` and `InstKind::ConstDecimal(String)`. `IrType::Float` / `InstKind::ConstFloat` keep their Rust names (surface `BinaryFloat`) — doc note only (per design D2)
- [x] 6.2 `ir.rs`: add conversion instructions `IntToDecimal`, `DecimalToInt`, `DecimalToFloat` (decimal->binary), `FloatToDecimal` (binary->decimal)
- [x] 6.3 `lower.rs`: lower `Decimal` arithmetic/comparison/rounding/parse/format to `zirk_rt_decimal_*` calls
- [x] 6.4 `lower.rs`: emit the zero-divisor guard before `Decimal` `/` and `%` (mirror `Duration` division lowering)
- [x] 6.5 `lower.rs`: lower integer operand -> exact `Float` at scale 0 in mixed arithmetic; lower the two contextual-constructor domains
- [x] 6.6 `lower.rs`: keep `BinaryFloat` lowering identical to today's `Float` lowering, including the `NaN` guard
- [x] 6.7 `zirk-ir` tests (`lowering.rs`): decimal op lowering, divide-by-zero guard, no `NaN` check on `Decimal`, conversions, both contextual domains

## 7. Codegen (`zirk-codegen-llvm`)

- [x] 7.1 `emit.rs`: emit `IrType::Decimal` as `{ i128, i8 }` aggregate; pass/return by pointer across the runtime boundary (mirror `STR_FROM_I128`)
- [x] 7.2 `emit.rs`: emit `ConstDecimal` from the parsed `{coef, scale}` directly, never via a binary float
- [x] 7.3 `emit.rs`: emit calls for every decimal runtime helper and the four conversion instructions
- [x] 7.4 `emit.rs`: `IrType::Float` (surface `BinaryFloat`) emission unchanged (LLVM `f16/f32/f64/f128`, explicit `NaN` guard)
- [x] 7.5 decimal helpers registered as `ExternFn`s in `zirk-ir` lowering; `declare_extern_fn` emits their `ptr`-shaped C signature (out-pointer for a `Decimal`/`i128` result) — no `runtime.rs` `Runtime` struct entry needed
- [x] 7.6 `zirk-codegen-llvm` tests (`emission.rs`): decimal constant, decimal arithmetic dispatch, by-pointer ABI, binary-width mapping unchanged

## 8. CLI + diagnostics

- [x] 8.1 `zirk-diagnostics` / `zirk-cli`: wording for the `Float64`->`BinaryFloat64` redirect, the oversized-literal error, and the `Float`/`BinaryFloat` mixing error
- [x] 8.2 `zirk-cli` help / examples text: replace `Float64` mentions

## 9. Documentation

- [x] 9.1 `docs/ZIRK_LANGUAGE_SPEC.md`: rewrite the numeric-families section (§3) — `Float` exact, `BinaryFloat*` binary, literal forms, `NaN`/infinity scoping
- [x] 9.2 `docs/ZIRK_STDLIB_SPEC.md`: rewrite the float section — exact `Float` member surface + `BinaryFloat` member surface
- [x] 9.3 `docs/ZIRK_COMPILER_SPEC.md` and `docs/CORE_LANGUAGE_SEMANTICS.md`: pipeline + semantics notes for the exact type and the rename
- [x] 9.4 `docs/ZIRK_RUNTIME_SPEC.md`: decimal runtime helpers (also covered by 3.3)
- [x] 9.5 Handbook: rewrite `02-handbook/03-everyday-types/04-decimals.md` as "Exact decimals (`Float`)"; add a `BinaryFloat` page; update `05-numeric-literals.md`, that chapter's `README.md`
- [x] 9.6 Handbook: update `11-reference/03-built-in-types.md`, `11-reference/13-type-member-index.md`, `SUMMARY.md`, and any `12-explanations` page discussing float precision
- [x] 9.7 Handbook: add a "Float is now exact — migration" note (rename table + `b` suffix)
- [x] 9.8 Roadmap/status: `docs/init/ZIRK_ROADMAP.md`, Feature Status, Current Limitations, handbook roadmap — move the `Float128` truncation + Windows-verification limitation onto `BinaryFloat128`
- [x] 9.9 Examples: handbook `.zrk` code blocks, spec docs, `docs/init/*` status/roadmap/agent-prompt updated. Repo-root scratch files (`main.zrk` etc.) are untracked and out of scope. Historical docs left as-is: `docs/01_plantilla_zirk.md` (design template), `docs/decisions/ADR-015`, `docs/decisions/2026-08-20-auditoria-*` (ADR / audit record)

## 10. End-to-end test battery

- [x] 10.1 Fixture: `0.1 + 0.2 == 0.3` prints `true`; `(0.1).to_string()` is `"0.1"`; sum prints `"0.3"`
- [x] 10.2 Fixture: exact `+ - *` across differing scales; integer `**`; normalization (`1.0 == 1.00`, `2.50 == 2.5`)
- [x] 10.3 Fixture: `1.0 / 3.0` rounds half-to-even at `MAX_SIGNIFICANT_DIGITS`; `a.div(b, mode, places)` variants; `round(places[, mode])` at tie boundaries
- [x] 10.4 Fixture: `%` sign, `sqrt` exact + negative error, `pow` fractional rounding
- [x] 10.5 Fixture: coefficient overflow raises catchable `ArithmeticOverflowError`; `Float / 0.0` raises catchable `DivisionByZeroError`
- [x] 10.6 Fixture: `Int -> Float` implicit; `Float -> Int` checked cast error on fractional; `Float(binaryValue)` and `x as BinaryFloat64` explicit; `Float + BinaryFloat64` is a type error
- [x] 10.7 Fixture: `1.5b`, `1.5b32`, `0.1b128` type and behave as the old `Float*`; `0.0b / 0.0b` catchable `FloatNanError`; non-zero `/ 0.0b` is infinity
- [x] 10.8 Fixture: `Float64` annotation emits the redirect diagnostic; oversized suffix-less literal emits the digit-budget diagnostic
- [x] 10.9 Fixture: `Float.parse` success and failure; `format(spec)` per 1.5
- [x] 10.10 Wire all fixtures into the CLI end-to-end test suite with expected stdout/diagnostics

## 11. Companion website — deferred to the merge/release workstream

These run against the sibling repo `../zirk-lang-site` (not present in this
worktree) and need a human-reviewed `--audit-date`. Do them after this branch is
merged to `develop`/`main` and committed.

- [ ] 11.1 After the zirk-lang changes are committed, run `./scripts/sync-website-content.sh`
- [ ] 11.2 Review `../zirk-lang-site` site-owned status catalog for the changed float semantics and limitations; re-run with `--audit-date YYYY-MM-DD`
- [ ] 11.3 Verify the website no longer describes `Float` as IEEE 754 binary and describes `BinaryFloat*` correctly

## 12. Close-out

- [x] 12.1 `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test` green across all crates
- [x] 12.2 `cargo clippy --all-targets` clean
- [x] 12.3 `openspec validate exact-decimal-float` passes; run `/opsx:archive` after review
