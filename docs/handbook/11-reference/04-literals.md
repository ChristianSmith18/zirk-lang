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
| Temporal singleton type | `Date(2026, 12, 25)`, `Time(9, 30)`, `DateTime(Date(...), Time(...))` | temporal singleton values use their owning type constructors in type position |
| Regex | `re'[A-Za-z_][A-Za-z0-9_]*'` | compiled `Regex` value; invalid patterns are compile-time errors |

Numeric separators do not affect value. A Float suffix cannot force an out-of-range value, and lossy conversion never truncates silently. There is no `NaN` literal. Date and zone text remain String values until explicitly parsed or passed to a temporal constructor.

## Literal and constant types

Type aliases may preserve a specific value as a singleton alternative:

```zirk
type Status = 200 | 404;
type Active = true | false;
type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);
type OfficeTime = Time(9, 0) | Time(17, 30);
type Meeting = DateTime(Date(2026, 12, 25), Time(9, 30))
             | DateTime(Date(2026, 12, 25), Time(10, 0));
type ConstructedHoliday = Date(2026, 12, 25) | Date(2027, 1, 1);
type ConstructedTime = Time(9, 0) | Time(17, 30);
type ConstructedMeeting = DateTime(Date(2026, 12, 25), Time(9, 30))
                       | DateTime(Date(2026, 12, 25), Time(10, 0));
```

The owning type is part of each value's identity. Singleton alternatives are
written directly as literal values or type names. Temporal singleton values
must use the fixed `Date(year, month, day)`, `Time(hour, minute, second?,
nanosecond?)`, and `DateTime(Date(...), Time(...))` forms, with every component
an integer literal. Bare ISO spellings such as `2026-12-25` and `09:30` are not
accepted in type position. These forms identify values directly; they do not
run constructors. Calls such as `Date.today()`, arithmetic, and other computed
expressions do not form singleton type atoms.

---

**Previous:** [← Built-in Types](03-built-in-types.md) · **Next:** [ Grammar Summary](05-grammar-summary.md)
