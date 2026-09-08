# Literals

| Kind | Examples | Inference and validation |
|---|---|---|
| integer | `0`, `42`, `0xff`, `0b1010`, `1_000` | `Int` when representable unless context or suffix selects a width |
| Decimal (exact) | `1.0`, `6.02e23` | `Decimal` — exact base-ten decimal; the default for fractional literals, so it takes no suffix |
| Binary float | `1.5f`, `1.5f32`, `0.1f128` | `Float*` — IEEE 754, width from the `f` suffix |
| Boolean | `true`, `false` | strict `Boolean` |
| Char | `'a'`, `'π'`, `'👨‍👩‍👧‍👦'` | exactly one Unicode grapheme or compile error |
| String | `"hello"`, `"value={value}"` | `String`; escapes and interpolation are validated |
| Null | `null` | only compatible with nullable context |
| collection | `[1, 2, 3]` and type-specific forms | elements require a compatible inferred type |
| Duration | `250ms`, `250us`, `5s`, `2h`, `1d` | exact signed `Duration`; suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w` |
| Regex | `re'[A-Za-z_][A-Za-z0-9_]*'` | compiled `Regex` value; invalid patterns are compile-time errors |

Numeric separators do not affect value. A Float suffix cannot force an out-of-range value, and lossy conversion never truncates silently. There is no `NaN` literal. Date and zone text remain String values until explicitly parsed or passed to a temporal constructor.

---

**Previous:** [← Built-in Types](03-built-in-types.md) · **Next:** [ Grammar Summary](05-grammar-summary.md)
