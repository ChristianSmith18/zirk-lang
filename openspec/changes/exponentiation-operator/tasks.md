## 1. Lexer: stop deferring `**`

- [ ] 1.1 `crates/zirk-lexer/src/token.rs`: remove the `StarStar | StarStarEq => Phase::THREE_B` arm from `TokenKind::phase`
- [ ] 1.2 Lexer test: `value ** 2` and `value **= 2` tokenize as power / compound-power with no phase attribution; still one token each, not two `*`

## 2. Parser: infix `**` and compound `**=`

- [ ] 2.1 `crates/zirk-parser/src/parser.rs`: add `parse_power_tail(base)` — if the next token is `**`, consume it (carrying `op_span`), parse the exponent with `parse_unary` (right-associative, exponent may be unary `-`), and build the desugared tree `base.pow(exponent)` (`Expr::Call` over `Expr::Field { name: "pow", safe: false }`, `op_span` on the `Field`)
- [ ] 2.2 Apply `parse_power_tail` to the postfix-complete operand in `parse_unary`, so a leading unary `-`/`!` wraps the whole power expression
- [ ] 2.3 Suppress the `-<integer literal>` fold when `peek_at(2)` is `**`, so `-2 ** 2` is `-(2 ** 2)`; keep the fold otherwise (`-2147483648` stays valid)
- [ ] 2.4 `compound_op` / statement parsing: expand `a **= b` to `a = a.pow(b)` (same desugaring as `**`); require an assignable, mutable place
- [ ] 2.5 Parser tests: right-associativity (`2 ** 3 ** 2`), `-2 ** 2` → `-(2 ** 2)`, `2 ** -1` (negated exponent operand), `3 * 2 ** 2` → `3 * (2 ** 2)`, `**=` expansion, `**=` on a non-place errors

## 3. Checker: integer `pow` and `**` result typing

- [ ] 3.1 `crates/zirk-sema/src/checker.rs`: add `pow` to the integer value-method surface — exponent checked as an integer; result is the receiver's integer type
- [ ] 3.2 When the exponent is a negative integer literal (`Expr::Int` with a negative value), type the `pow` call as exact `Float` (`Type::FLOAT`)
- [ ] 3.3 Confirm `Float ** _` types as exact `Float`, `BinaryFloatN ** _` as that width, and an exact-`Float` / `BinaryFloat` mix raises the existing mixed-family diagnostic (all via the existing `pow` method arms — add a typing test, no new code expected)
- [ ] 3.4 `zirk-sema` tests (`typing.rs`): `2 ** 3` is `Int`; `2 ** -1` is `Float`; `(1.5) ** 2` is `Float`; `(2.0b) ** 3` is `BinaryFloat64`; `(2.0) ** (3.0b)` is a type error naming the conversion; dynamic exponent stays `Int`

## 4. IR lowering: integer `pow`

- [ ] 4.1 `crates/zirk-ir/src/lower.rs`: add `pow` to `is_scalar_method_call`'s integer `INT1` set (arity 1)
- [ ] 4.2 `lower_int_method_call`: `pow` arm — when the checker typed the call as `Decimal`, lower the receiver with `IntToDecimal` (scale 0), lower the exponent to `i64`, call `zirk_rt_decimal_pow_i`
- [ ] 4.3 `lower_int_method_call`: `pow` arm otherwise — call `zirk_int_checked_pow_ok`; on `false` throw `arithmetic_overflow`; on `true` call `zirk_int_checked_pow_value` and narrow to the receiver width
- [ ] 4.4 `zirk-ir` tests (`lowering.rs`): integer `**` lowers to the checked-pow calls with the overflow branch; negative-literal `**` lowers through `zirk_rt_decimal_pow_i`; `Float`/`BinaryFloat` `**` lower identically to the `pow` method

## 5. Codegen

- [ ] 5.1 Confirm `zirk-codegen-llvm` needs no new path (all helpers already declared/emitted); add an `emission.rs` assertion that `Int ** Int` emits the checked-pow calls and `Float ** Int` the decimal pow call

## 6. End-to-end tests

- [ ] 6.1 CLI fixture: `2 ** 3` prints `8`; `2 ** 10` prints `1024`; `2 ** 3 ** 2` prints `512`; `-2 ** 2` prints `-4`
- [ ] 6.2 CLI fixture: `2 ** -3` prints `0.125` (exact `Float`); `(1.5) ** 2` prints `2.25`
- [ ] 6.3 CLI fixture: `(2.0b) ** 10` prints `1024`; compound `n **= 10` on `mut n = 2` prints `1024`
- [ ] 6.4 CLI fixture (invalid): `Int32` `10 ** 20` raises a catchable `ArithmeticOverflowError`; `(2.0) ** (3.0b)` is a type error
- [ ] 6.5 Wire fixtures into the CLI end-to-end suite with expected stdout / diagnostics

## 7. Documentation

- [ ] 7.1 `docs/ZIRK_LANGUAGE_SPEC.md`: confirm the operator table and numeric-families section present `**` as available; add the `Int ** negativeLiteral -> Float` rule and the `-2 ** 2` associativity note
- [ ] 7.2 `docs/ZIRK_STDLIB_SPEC.md`: note `a ** b` is defined as `a.pow(b)` for the numeric families
- [ ] 7.3 Handbook `02-handbook/05-operators-and-expressions/01-arithmetic.md`: expand the `**` section with the widening rule and the compound form; `11-reference/02-operators-and-precedence.md` already lists it — verify wording
- [ ] 7.4 `docs/handbook/11-reference/12-feature-status.md`: move exponentiation from "intended surface / compiler support may trail" to delivered
- [ ] 7.5 `docs/init/ZIRK_ROADMAP.md` Phase 3b and the handbook roadmap (`13-appendices/08-roadmap.md`): list `**` / `**=` among the delivered Phase 3b operators
- [ ] 7.6 `docs/handbook/13-appendices/07-current-limitations.md`: verify no stale "exponentiation not available" claim remains

## 8. Companion website

- [ ] 8.1 After the zirk-lang changes are committed, run `./scripts/sync-website-content.sh`
- [ ] 8.2 Review `../zirk-lang-site` site-owned status catalog for the exponentiation-operator status change; re-run with `--audit-date YYYY-MM-DD`
- [ ] 8.3 Verify the website presents `**` as available and no longer implies it is deferred

## 9. Close-out

- [ ] 9.1 `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` green
- [ ] 9.2 `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean
- [ ] 9.3 `openspec validate exponentiation-operator` passes; run `/opsx:archive` after review
