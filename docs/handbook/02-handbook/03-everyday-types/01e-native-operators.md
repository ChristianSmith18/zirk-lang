# Native Operators

Native operators are closed language behavior. The compact matrix lives in the
[operator reference](../../11-reference/02-operators-and-precedence.md); this
chapter explains the rules that apply across types.

| Family | Native operations | Important result rule |
| --- | --- | --- |
| Signed integers | arithmetic, order, bitwise, increment | `/` truncates toward zero |
| Unsigned integers | signed set except unary `-` | signed mixing is explicit |
| Float | arithmetic and order | mixed integer arithmetic yields Float |
| Boolean | `!`, `&&`, `||`, equality | no truthiness or ordering |
| Char | equality and deterministic order | no numeric arithmetic |
| String | `+`, `*`, equality, order, `is` | repetition requires non-negative integer |
| Temporal | type-specific composition/arithmetic | Duration and Period are distinct |

Compound assignment requires a reassignable binding because it stores the
operator result:

```zirk
mut laugh = "ja";
laugh *= 3; // "jajaja"

inmut fixed = "ja";
fixed *= 3; // error: binding cannot be reassigned
```

All ordinary numeric overflow, zero division, impossible String allocation,
negative repetition count, invalid shift and unsupported temporal composition
produce controlled diagnostics or errors. `unsafe` does not grant permission
to redefine normal operator semantics.

Operator precedence is syntactic and never changes when a user type implements
a contract. Parenthesize when the intended grouping is not immediately clear.

---

**Previous:** [← Conversions and Context](01d-conversions-and-context.md) · **Next:** [ Signed Integers](02-signed-integers.md)
