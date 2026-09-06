## Context

`**` / `**=` are lexed today but gated to roadmap Phase 3b in
`TokenKind::phase`, so any use raises the phase diagnostic. Every numeric
family already has a working `pow` path:

- exact `Float`: `zirk_rt_decimal_pow_i` (integer exponent, exact repeated
  multiply, reciprocal for a negative exponent) and `zirk_rt_decimal_pow`
  (decimal exponent, f64 precision).
- `BinaryFloatN`: `zirk_float_pow` (`powi`) / `zirk_float_powf`.
- integers: `zirk_int_checked_pow_ok` / `zirk_int_checked_pow_value` (already
  declared as externs; currently only reachable through `checked_pow`, which
  returns a `Result`).

The grammar spec and the handbook operator table already describe `**` as a
right-associative operator that binds tighter than unary `-`. The work is to
make the parser build it, give it result-typing rules, and route it to the
existing runtime helpers.

## Goals / Non-Goals

Goals:
- `**` and `**=` usable across `Int`, `Float`, `BinaryFloat`.
- Reuse the existing `pow` runtime paths; add no runtime symbol.
- Keep `**`-specific typing rules (integer stays integer, a negative literal
  exponent widens to exact `Float`, `Float`/`BinaryFloat` mixing is an error)
  localized.

Non-Goals:
- No new precision guarantee for irrational results — `Float ** Float` and
  `BinaryFloat` keep their documented f64-precision behavior.
- No change to `pow`, `checked_pow`, or any other method.
- No `**` for non-numeric contract types (a contract may define `pow`, but the
  operator surface here is the native numeric families only).

## Decisions

### D1: Desugar in the parser to a `pow` method call

`a ** b` parses to the exact tree `a.pow(b)` would produce
(`Expr::Call { callee: Expr::Field { name: "pow", safe: false }, args: [b] }`),
and `a **= b` to `a = a.pow(b)`. Downstream stages then see an ordinary method
call, so `Float` and `BinaryFloat` need zero new code. Rejected alternative:
a new `BinaryOp::Pow`. It would need a bespoke arm in the checker arithmetic
table, the IR binary-lowering dispatch, and `emit_checked_binary` (which
assumes a native LLVM instruction exists — `**` has none), for no benefit over
the desugaring.

### D2: `**` precedence sits between postfix and unary, right-associative

`parse_unary` gains a `parse_power_tail` step applied to the postfix-complete
operand: if the next token is `**`, consume it and parse the exponent as a
full `parse_unary` (which recurses, giving right-associativity and letting the
exponent be `-1`). A leading unary `-`/`!` wraps the whole power expression,
so `-x ** 2` is `-(x ** 2)`.

The existing `-<integer literal>` fold is suppressed when the next token is
`**` (`self.peek_at(2)`), so `-2 ** 2` is `-(2 ** 2)` = `-4`, consistent with
the general unary rule. A negative integer literal not followed by `**` still
folds, keeping `-2147483648` valid.

### D3: Integer `pow` method surface

The checker's integer value-method surface gains `pow(exponent)`:

- exponent must be an integer (`check_int_argument`).
- if the exponent expression is a negative integer literal (`Expr::Int` with a
  negative value — the parser has already folded `-1`), the result type is
  exact `Float`: `2 ** -1` is `0.5`.
- otherwise the result is the receiver's integer type; overflow is a
  controlled `ArithmeticOverflowError`, like every other integer operation.

Only a **literal** negative exponent widens. `n.pow(k)` with a dynamic `k`
stays integer, and a negative `k` at runtime raises the same controlled error
`zirk_int_checked_pow_ok` already reports (it cannot represent a fraction in
an integer type). This keeps the result type statically decidable.

### D4: IR lowering for integer `pow`

`is_scalar_method_call` and `lower_int_method_call` gain a `pow` arm:

- when the checker typed the call as `Decimal` (negative literal exponent):
  lower the receiver with `IntToDecimal` at scale 0, lower the exponent to
  `i64`, and call `zirk_rt_decimal_pow_i`.
- otherwise: call `zirk_int_checked_pow_ok(base, exp, bits, signed)`; on
  `false` throw `arithmetic_overflow`; on `true` call
  `zirk_int_checked_pow_value(...)` and narrow to the receiver width. This is
  the `checked_*` lowering with the `Result` wrapper replaced by an
  unwrap-or-throw, mirroring `checked_int_arithmetic`.

### D5: Phase gate removal

`TokenKind::phase` stops returning `Phase::THREE_B` for `StarStar` /
`StarStarEq`. The lexer's power-token test is extended to assert no phase is
attributed. `docs/init/ZIRK_ROADMAP.md` Phase 3b and Feature Status are
updated to list `**` as delivered.

## Risks / Trade-offs

- **`-2 ** 2` semantics**: chosen as `-(2 ** 2)` for consistency with the
  general unary rule; a reader expecting `(-2) ** 2` (because `-2` is a Zirk
  literal elsewhere) may be surprised. Mitigated by a handbook note and the
  parser test pinning it.
- **Value-dependent result type** (`2 ** -1` is `Float`, `2 ** 1` is `Int`):
  limited to a *literal* negative exponent so the type stays static. A
  dynamic negative exponent raises at runtime rather than silently widening.
- **Desugaring visibility**: diagnostics on a bad `**` point through a
  synthesized `pow` call. The operator's `op_span` is carried onto the
  synthetic `Field` node so the message still underlines the `**`.

## Migration Plan

None required. `**` was a compile error before this change, so no valid
program depends on its absence. The companion website is re-synced after the
zirk-lang commits land.

## Open Questions

None.
